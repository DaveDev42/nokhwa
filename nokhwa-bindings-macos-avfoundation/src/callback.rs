use crate::ffi::CMSampleBufferRef;
use crate::ffi::{
    dispatch_queue_create, dispatch_release, CMSampleBufferGetImageBuffer,
    CMSampleBufferGetPresentationTimeStamp, CVImageBufferRef, CVPixelBufferGetBaseAddress,
    CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRow,
    CVPixelBufferGetBytesPerRowOfPlane, CVPixelBufferGetHeight, CVPixelBufferGetHeightOfPlane,
    CVPixelBufferGetPixelFormatType, CVPixelBufferGetPlaneCount, CVPixelBufferGetWidth,
    CVPixelBufferGetWidthOfPlane, CVPixelBufferIsPlanar, CVPixelBufferLockBaseAddress,
    CVPixelBufferUnlockBaseAddress, DispatchQueue,
};
use crate::types::{AVAuthorizationStatus, AVMediaTypeLocal};
use crate::util::raw_fcc_to_frameformat;
use block2::RcBlock;
use nokhwa_core::{error::NokhwaError, types::FrameFormat};
use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{define_class, msg_send, AnyThread, DefinedClass};
use objc2_av_foundation::{
    AVCaptureConnection, AVCaptureDevice, AVCaptureOutput,
    AVCaptureVideoDataOutputSampleBufferDelegate, AVMediaTypeVideo,
};
use objc2_core_media::CMSampleBuffer;
use std::cell::Cell;
use std::sync::mpsc::Sender;
use std::{
    ffi::{c_void, CStr},
    sync::Arc,
    time::Duration,
};

/// Raw frame data from the capture callback: (pixels, format, optional sensor timestamp).
pub type FrameData = (Vec<u8>, FrameFormat, Option<Duration>);

extern "C" {
    fn mach_absolute_time() -> u64;
}

#[repr(C)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

extern "C" {
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
}

fn mach_absolute_time_nanos() -> u64 {
    static TIMEBASE: std::sync::LazyLock<(u32, u32)> = std::sync::LazyLock::new(|| {
        let mut info = MachTimebaseInfo { numer: 0, denom: 0 };
        unsafe { mach_timebase_info(&raw mut info) };
        (info.numer, info.denom)
    });
    let ticks = unsafe { mach_absolute_time() };
    let (numer, denom) = *TIMEBASE;
    ticks.wrapping_mul(u64::from(numer)) / u64::from(denom)
}

/// Convert a `CMSampleBuffer` presentation timestamp into an absolute
/// wallclock instant.
///
/// `pts.value / pts.timescale` is the buffer's presentation time in
/// `CLOCK_MACH` (the same clock as `mach_absolute_time`). We compute
/// the buffer's *age* relative to `mono_now_nanos` and subtract that
/// from `wall_now`'s `UNIX_EPOCH` offset to recover when the sensor
/// captured it.
///
/// Returns `None` for any of the documented degenerate cases: an
/// uninitialised `CMTime` (`timescale == 0`), a system clock that
/// is before the unix epoch (`duration_since` fails), or an `age`
/// large enough that subtracting it from `wall_now` underflows.
///
/// `pts.value` is `i64` and is treated as non-negative — Apple
/// documents `presentationTimeStamp` as monotonic and non-negative
/// during an active capture session, so we use `saturating_sub` on
/// `mono_now_nanos - pts_nanos` to clamp future-PTS clock skew to a
/// zero-age (instead-of-panic) result.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pts_to_wallclock(
    pts: crate::ffi::CMTime,
    mono_now_nanos: u64,
    wall_now: std::time::SystemTime,
) -> Option<Duration> {
    if pts.timescale <= 0 {
        return None;
    }
    let pts_nanos = (pts.value as u128).saturating_mul(1_000_000_000) / (pts.timescale as u128);
    let mono_now = u128::from(mono_now_nanos);
    let age = Duration::from_nanos(mono_now.saturating_sub(pts_nanos) as u64);
    wall_now
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|wall_dur| wall_dur.checked_sub(age))
}

/// Instance variables for `MyCaptureCallback`.
///
/// Holds a type-erased `*const c_void` that is actually an *owned* `Arc<Sender<FrameData>>`
/// strong count, produced via `Arc::into_raw` and released in `dealloc`. Interior
/// mutability via `Cell` is required because `ivars()` returns `&Self::Ivars` (shared
/// reference only) by design.
pub struct CaptureCallbackIvars {
    arc_sender: Cell<*const c_void>,
}

/// Tight-packed bytes-per-pixel for the packed (non-planar) `FrameFormat`s
/// `AVFoundation` can hand us. Used to compute the destination row stride
/// when repacking a buffer that may have hardware row padding.
fn packed_bytes_per_pixel(format: FrameFormat) -> Option<usize> {
    match format {
        FrameFormat::YUYV => Some(2),
        FrameFormat::GRAY => Some(1),
        FrameFormat::RAWRGB | FrameFormat::RAWBGR => Some(3),
        // MJPEG is compressed (no row concept) and NV12 is planar — both
        // are handled outside this helper.
        FrameFormat::MJPEG | FrameFormat::NV12 => None,
    }
}

/// Copy `rows` rows of `dst_stride` useful bytes each out of a source
/// region whose physical row stride is `src_stride` (which may be larger
/// than `dst_stride` due to hardware padding), appending into `out`.
///
/// # Safety
/// `base` must point to at least `rows * src_stride` readable bytes,
/// which holds while the `CVPixelBuffer` base-address lock is held.
unsafe fn copy_rows(
    base: *const u8,
    src_stride: usize,
    dst_stride: usize,
    rows: usize,
    out: &mut Vec<u8>,
) {
    let copy = dst_stride.min(src_stride);
    for row in 0..rows {
        let row_ptr = unsafe { base.add(row * src_stride) };
        let row_slice = unsafe { std::slice::from_raw_parts(row_ptr, copy) };
        out.extend_from_slice(row_slice);
    }
}

/// Extract a tight-packed (no row padding) frame from a locked
/// `CVPixelBuffer`, honoring planar layout and per-row stride.
///
/// The downstream SIMD/scalar decoders assume the canonical packed
/// layout for each `FrameFormat` (`width * bpp` bytes per row for packed
/// formats; a `width`-strided Y plane immediately followed by a
/// `width`-strided interleaved `CbCr` plane for NV12). `AVFoundation`,
/// however, returns buffers with hardware row padding (stride can exceed
/// `width * bpp`, common on Apple Silicon) and delivers `420v`/`420f`/
/// `x420` as a *bi-planar* buffer whose planes are not contiguous and
/// each carry their own stride. The old flat
/// `GetBaseAddress`+`GetDataSize` copy mishandled both: it dragged
/// padding bytes into the output for padded packed formats, and for the
/// bi-planar 4:2:0 formats it copied only the Y plane (plus whatever
/// happened to follow it), corrupting every NV12 frame.
///
/// # Safety
/// `image_buffer` must be non-null and its base address must be locked
/// (`CVPixelBufferLockBaseAddress` succeeded) for the duration of the
/// call.
// The CoreVideo getters return `c_ulong` (`u64` on the only targets this
// crate runs on — 64-bit Apple platforms). Frame dimensions and strides
// always fit a `usize` there, so the `as usize` casts cannot truncate.
#[allow(clippy::cast_possible_truncation)]
unsafe fn extract_frame_bytes(
    image_buffer: CVImageBufferRef,
    format: FrameFormat,
) -> Option<Vec<u8>> {
    if unsafe { CVPixelBufferIsPlanar(image_buffer) } {
        // Planar path — only the 4:2:0 bi-planar formats (mapped to
        // FrameFormat::NV12) reach here. Repack into the canonical NV12
        // layout: full-width Y plane, then full-width interleaved CbCr.
        let plane_count = unsafe { CVPixelBufferGetPlaneCount(image_buffer) } as usize;
        if plane_count < 2 {
            return None;
        }
        let mut out = Vec::new();
        for plane in 0..2usize {
            let base = unsafe { CVPixelBufferGetBaseAddressOfPlane(image_buffer, plane as _) };
            if base.is_null() {
                return None;
            }
            let src_stride =
                unsafe { CVPixelBufferGetBytesPerRowOfPlane(image_buffer, plane as _) } as usize;
            let plane_w =
                unsafe { CVPixelBufferGetWidthOfPlane(image_buffer, plane as _) } as usize;
            let plane_h =
                unsafe { CVPixelBufferGetHeightOfPlane(image_buffer, plane as _) } as usize;
            // Y plane: 1 byte per sample column. CbCr plane: 2 bytes per
            // sample column (interleaved Cb,Cr), so the useful width is
            // `plane_w * 2` bytes — GetWidthOfPlane reports the chroma
            // sample count, not the byte count.
            let dst_stride = if plane == 0 { plane_w } else { plane_w * 2 };
            unsafe { copy_rows(base.cast::<u8>(), src_stride, dst_stride, plane_h, &mut out) };
        }
        Some(out)
    } else {
        let bpp = packed_bytes_per_pixel(format)?;
        let width = unsafe { CVPixelBufferGetWidth(image_buffer) } as usize;
        let height = unsafe { CVPixelBufferGetHeight(image_buffer) } as usize;
        let src_stride = unsafe { CVPixelBufferGetBytesPerRow(image_buffer) } as usize;
        let base = unsafe { CVPixelBufferGetBaseAddress(image_buffer) };
        if base.is_null() || width == 0 || height == 0 {
            return None;
        }
        let dst_stride = width * bpp;
        let mut out = Vec::with_capacity(dst_stride * height);
        unsafe { copy_rows(base.cast::<u8>(), src_stride, dst_stride, height, &mut out) };
        Some(out)
    }
}

// SAFETY: `arc_sender` is only written once at init time (before any GCD callbacks
// can fire) and is only read from the serial GCD dispatch queue thereafter.
// `Cell<*const c_void>` is not `Send` by default; we assert it is safe to move
// the whole `MyCaptureCallback` across threads because:
//  - The pointer is read-only after `set_ivars` (init is single-threaded).
//  - The GCD queue ensures serialised access to reads.
//  - `Arc<Sender<FrameData>>` itself is `Send`.
// We deliberately do NOT implement `Sync`: `Cell` is `!Sync`, the serial
// queue gives us single-threaded *access* (which `Send` covers), and no
// code path takes a shared `&CaptureCallbackIvars` across threads.
unsafe impl Send for CaptureCallbackIvars {}

define_class!(
    // SAFETY:
    // - `NSObject` has no subclassing requirements that we violate.
    // - We release the owned `Arc<Sender>` strong count by implementing `Drop` for
    //   `MyCaptureCallback` (objc2 runs it from `dealloc`); `#[unsafe(method(dealloc))]`
    //   is not permitted in objc2 0.6.
    // - The `AVCaptureVideoDataOutputSampleBufferDelegate` impl upholds the protocol contract:
    //   the required `captureOutput:didOutputSampleBuffer:fromConnection:` method is
    //   implemented and only reads pixel data while the CVPixelBuffer base-address lock is held.
    #[unsafe(super(objc2::runtime::NSObject))]
    #[name = "MyCaptureCallback"]
    #[ivars = CaptureCallbackIvars]
    struct MyCaptureCallback;

    unsafe impl NSObjectProtocol for MyCaptureCallback {}

    // Delegate compliance method
    // SAFETY: Reads pixel data from CVPixelBuffer while base address lock is held.
    // The lock guarantees buffer_ptr is valid and buffer_length bytes are readable.
    // cast_possible_truncation, cast_sign_loss: CoreMedia timestamps are i64/i32;
    // u128 arithmetic is safe here because the values are always non-negative in
    // practice (presentation times from a running capture session). The final
    // saturating_sub result is bounded by the session uptime, well within u64::MAX.
    unsafe impl AVCaptureVideoDataOutputSampleBufferDelegate for MyCaptureCallback {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        #[allow(non_snake_case)]
        #[unsafe(method(captureOutput:didOutputSampleBuffer:fromConnection:))]
        unsafe fn capture_output_did_output_sample_buffer(
            &self,
            _output: &AVCaptureOutput,
            sample_buffer: &CMSampleBuffer,
            _connection: &AVCaptureConnection,
        ) {
            // Cast the typed CMSampleBuffer ref to the raw CMSampleBufferRef our FFI expects.
            // SAFETY: CMSampleBuffer is a CoreFoundation opaque type; `&CMSampleBuffer` is a
            // non-null reference to the same opaque object that `CMSampleBufferRef` points at.
            // The cast is a pointer identity transformation with no layout change.
            let raw_sb: CMSampleBufferRef = std::ptr::from_ref::<CMSampleBuffer>(sample_buffer)
                .cast_mut()
                .cast::<c_void>();

            let image_buffer: CVImageBufferRef = unsafe { CMSampleBufferGetImageBuffer(raw_sb) };

            if image_buffer.is_null() {
                return;
            }

            let pixel_format = unsafe { CVPixelBufferGetPixelFormatType(image_buffer) };
            let frame_format = raw_fcc_to_frameformat(pixel_format).unwrap_or(FrameFormat::YUYV);

            unsafe {
                CVPixelBufferLockBaseAddress(image_buffer, 0);
            };

            // Repack honoring planar layout + per-row stride. Returns a
            // tight-packed buffer in the canonical layout the decoders
            // expect; `None` if the buffer was empty/malformed or a plane
            // base address was null.
            let extracted = unsafe { extract_frame_bytes(image_buffer, frame_format) };

            unsafe { CVPixelBufferUnlockBaseAddress(image_buffer, 0) };

            let Some(buffer_as_vec) = extracted else {
                return;
            };

            // Compute sensor capture timestamp from CMSampleBuffer presentation time
            let capture_ts = {
                let pts = unsafe { CMSampleBufferGetPresentationTimeStamp(raw_sb) };
                pts_to_wallclock(
                    pts,
                    mach_absolute_time_nanos(),
                    std::time::SystemTime::now(),
                )
            };

            // Borrow the owned Arc<Sender> through the stored raw pointer without
            // touching its strong count: reconstruct, send, then `forget` so the
            // count is left intact (it is owned by the ivar, released in dealloc).
            // `Sender::send` returns `Err` (never panics) when the receiver is gone,
            // so there is no unwind between `from_raw` and `forget`.
            let arc_sender_ptr = self.ivars().arc_sender.get();
            let buffer_sndr = unsafe {
                let ptr = arc_sender_ptr.cast::<Sender<FrameData>>();
                Arc::from_raw(ptr)
            };
            let _ = buffer_sndr.send((buffer_as_vec, frame_format, capture_ts));
            std::mem::forget(buffer_sndr);
        }

        #[allow(non_snake_case)]
        #[unsafe(method(captureOutput:didDropSampleBuffer:fromConnection:))]
        unsafe fn capture_output_did_drop_sample_buffer(
            &self,
            _output: &AVCaptureOutput,
            _sample_buffer: &CMSampleBuffer,
            _connection: &AVCaptureConnection,
        ) {
            // Dropped frames are silently ignored.
        }
    }
);

impl Drop for MyCaptureCallback {
    /// Release the owned `Arc<Sender<FrameData>>` strong count that was handed to
    /// the ivar via `Arc::into_raw` in `new`. `objc2` runs this from the
    /// Objective-C `dealloc`, after the last (GCD or otherwise) reference to the delegate is
    /// gone — so reconstructing and dropping the `Arc` here balances the
    /// `into_raw` exactly once and frees the `Sender` only when nothing can still
    /// touch it.
    fn drop(&mut self) {
        let ptr = self.ivars().arc_sender.get().cast::<Sender<FrameData>>();
        if !ptr.is_null() {
            unsafe { drop(Arc::from_raw(ptr)) };
        }
    }
}

impl MyCaptureCallback {
    /// Allocate and initialize a new `MyCaptureCallback` with the given sender pointer.
    ///
    /// The `arc_sender_ptr` must be a raw pointer obtained from `Arc::into_raw` on an
    /// `Arc<Sender<FrameData>>` — i.e. it carries an owned strong count. That count is
    /// released in `dealloc`, so the `Sender` is kept alive for exactly as long as this
    /// delegate object (and any GCD callback still referencing it) lives.
    fn new_with_ptr(arc_sender_ptr: *const c_void) -> Retained<Self> {
        let this = Self::alloc().set_ivars(CaptureCallbackIvars {
            arc_sender: Cell::new(arc_sender_ptr),
        });
        unsafe { msg_send![super(this), init] }
    }
}

/// Requests camera access permission from the user.
///
/// # Panics
///
/// Panics if the `AVMediaTypeVideo` constant is unavailable on the current
/// platform, which should not happen on any supported Apple platform.
pub fn request_permission_with_callback(callback: impl Fn(bool) + Send + Sync + 'static) {
    use objc2::runtime::Bool;
    use objc2_av_foundation::AVCaptureDevice as AvCapDev;
    let media_type = unsafe { AVMediaTypeVideo.unwrap() };

    let wrapper = move |b: Bool| {
        callback(b.as_bool());
    };

    let objc_fn_pass = RcBlock::new(wrapper);

    unsafe {
        AvCapDev::requestAccessForMediaType_completionHandler(media_type, &objc_fn_pass);
    }
}

#[must_use]
pub fn current_authorization_status() -> AVAuthorizationStatus {
    let media_type = AVMediaTypeLocal::Video.to_av_media_type();
    let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type) };
    decode_authorization_status(status.0)
}

/// Decode the raw `NSInteger` returned by
/// `AVCaptureDevice::authorizationStatusForMediaType` into our local
/// [`AVAuthorizationStatus`] enum. Split out from
/// [`current_authorization_status`] so the four documented branches
/// plus the "anything else" fallback can be pinned without an
/// `AVCaptureDevice` round-trip.
///
/// Constants match Apple's `AVAuthorizationStatus`:
/// <https://developer.apple.com/documentation/avfoundation/avauthorizationstatus>.
/// Unknown values (negative ints, future Apple additions) collapse to
/// `NotDetermined` — the conservative default that prompts the user
/// rather than assuming access.
#[must_use]
fn decode_authorization_status(raw: isize) -> AVAuthorizationStatus {
    match raw {
        1 => AVAuthorizationStatus::Restricted,
        2 => AVAuthorizationStatus::Denied,
        3 => AVAuthorizationStatus::Authorized,
        _ => AVAuthorizationStatus::NotDetermined,
    }
}

/// Wraps an Objective-C delegate and GCD dispatch queue for receiving video frames.
///
/// # Thread Safety
/// The `delegate` field is a `Retained<MyCaptureCallback>` managed by ARC. The
/// `MyCaptureCallback` object is only created on a single thread and thereafter
/// all accesses to its ivars occur on the serial GCD dispatch queue.
pub struct AVCaptureVideoCallback {
    delegate: Retained<MyCaptureCallback>,
    queue: DispatchQueue,
}

impl AVCaptureVideoCallback {
    pub fn new(device_spec: &CStr, buffer: &Arc<Sender<FrameData>>) -> Result<Self, NokhwaError> {
        // Hand the delegate its *own* strong reference (via `Arc::into_raw`),
        // not a borrowed `Arc::as_ptr`. The sample-buffer callback runs on a GCD
        // queue that can still have a block in flight after the caller stops the
        // session and drops its `Arc`; an owned count guarantees the `Sender`
        // outlives the delegate object (and thus any queued callback), closing a
        // teardown use-after-free. The count is released in `dealloc`.
        let arc_sender_ptr = Arc::into_raw(Arc::clone(buffer)).cast::<c_void>();
        let delegate = MyCaptureCallback::new_with_ptr(arc_sender_ptr);
        let queue = unsafe { dispatch_queue_create(device_spec.as_ptr(), std::ptr::null()) };
        Ok(AVCaptureVideoCallback { delegate, queue })
    }

    /// Returns a raw `*mut AnyObject` pointer to the delegate object.
    ///
    /// Used by `session.rs` to pass the delegate to
    /// `setSampleBufferDelegate:queue:` via `msg_send!`. The pointer is
    /// valid for as long as `self` is alive (the `Retained` keeps it alive).
    #[must_use]
    pub fn inner(&self) -> *mut objc2::runtime::AnyObject {
        Retained::as_ptr(&self.delegate).cast_mut().cast()
    }

    #[must_use]
    pub fn queue(&self) -> &DispatchQueue {
        &self.queue
    }
}

impl Drop for AVCaptureVideoCallback {
    fn drop(&mut self) {
        // `delegate` (a `Retained<MyCaptureCallback>`) is dropped automatically,
        // which sends an ObjC `release` message and deallocates the object when
        // the retain count reaches zero. No manual `release` is needed.
        if !self.queue.0.is_null() {
            unsafe {
                dispatch_release(DispatchQueue(self.queue.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_authorization_status, pts_to_wallclock};
    use crate::ffi::CMTime;
    use crate::types::AVAuthorizationStatus;
    use std::time::{Duration, UNIX_EPOCH};

    fn cmtime(value: i64, timescale: i32) -> CMTime {
        CMTime {
            value,
            timescale,
            flags: 0,
            epoch: 0,
        }
    }

    /// `timescale == 0` is the documented "uninitialised `CMTime`"
    /// sentinel and must short-circuit to `None` before the
    /// division — the previous inline code did this too, but with
    /// a `>` rather than `<=` check, so let's pin both forms.
    #[test]
    fn pts_to_wallclock_zero_timescale_returns_none() {
        let pts = cmtime(1_000_000_000, 0);
        let wall = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        assert_eq!(pts_to_wallclock(pts, 5_000_000_000, wall), None);
    }

    /// Negative timescale is an invalid Apple value (Apple
    /// documents `timescale > 0` for valid presentation times); we
    /// reject it the same as zero.
    #[test]
    fn pts_to_wallclock_negative_timescale_returns_none() {
        let pts = cmtime(1_000_000_000, -1);
        let wall = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        assert_eq!(pts_to_wallclock(pts, 5_000_000_000, wall), None);
    }

    /// Happy path: a 1-second-old PTS (`mono_now` - `pts_nanos` = 1s)
    /// pins back to `wall_now` - 1s.
    #[test]
    fn pts_to_wallclock_1s_old_pts_subtracts_1s_from_wall_now() {
        // pts_nanos = 4_000_000_000 / 1 = 4 s expressed as nanos
        let pts = cmtime(4_000_000_000, 1_000_000_000);
        let mono_now_nanos: u64 = 5_000_000_000; // 5 s on the mach clock
        let wall = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let observed = pts_to_wallclock(pts, mono_now_nanos, wall)
            .expect("happy-path conversion must succeed");
        let expected = Duration::from_secs(1_700_000_000 - 1);
        assert_eq!(observed, expected);
    }

    /// Future PTS (`pts_nanos` > `mono_now_nanos`) — clock skew or a
    /// buggy emulator. The `saturating_sub` clamps the age to 0 so
    /// the returned wallclock equals `wall_now`'s offset; pin that
    /// the function returns `Some(_)` rather than panicking.
    #[test]
    fn pts_to_wallclock_future_pts_clamps_age_to_zero() {
        let pts = cmtime(10_000_000_000, 1_000_000_000); // 10 s
        let mono_now_nanos: u64 = 5_000_000_000; // 5 s — pts is in the future
        let wall = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let observed =
            pts_to_wallclock(pts, mono_now_nanos, wall).expect("future-pts must clamp, not panic");
        assert_eq!(observed, Duration::from_secs(1_700_000_000));
    }

    /// `wall_now` before `UNIX_EPOCH` (impossible on real hosts but
    /// possible in a synthetic mock) → `duration_since` returns
    /// `Err` → helper returns `None` rather than wrapping.
    #[test]
    fn pts_to_wallclock_wall_before_unix_epoch_returns_none() {
        let pts = cmtime(1_000_000_000, 1_000_000_000);
        let wall = UNIX_EPOCH - Duration::from_secs(1);
        assert_eq!(pts_to_wallclock(pts, 5_000_000_000, wall), None);
    }

    /// `age > wall_now` (e.g. mocked `wall_now` of 1 ns post-epoch
    /// with a 5-second-old buffer) → `checked_sub` returns `None`
    /// instead of underflowing.
    #[test]
    fn pts_to_wallclock_age_exceeds_wall_now_returns_none() {
        // pts_nanos = 0 (timescale=1, value=0), mono_now=5s, age=5s
        let pts = cmtime(0, 1);
        let mono_now_nanos: u64 = 5_000_000_000;
        let wall = UNIX_EPOCH + Duration::from_nanos(1);
        assert_eq!(pts_to_wallclock(pts, mono_now_nanos, wall), None);
    }

    /// Apple's `AVAuthorizationStatus` constant `0` → `NotDetermined`.
    /// This is also the conservative fallback for unknown values, so
    /// pin the explicit `0` branch separately from the default arm.
    #[test]
    fn decode_authorization_status_zero_is_not_determined() {
        assert_eq!(
            decode_authorization_status(0),
            AVAuthorizationStatus::NotDetermined
        );
    }

    /// Apple's `AVAuthorizationStatus` constant `1` → `Restricted`
    /// (parental controls / MDM block).
    #[test]
    fn decode_authorization_status_one_is_restricted() {
        assert_eq!(
            decode_authorization_status(1),
            AVAuthorizationStatus::Restricted
        );
    }

    /// Apple's `AVAuthorizationStatus` constant `2` → `Denied` (user
    /// declined or revoked in System Settings).
    #[test]
    fn decode_authorization_status_two_is_denied() {
        assert_eq!(
            decode_authorization_status(2),
            AVAuthorizationStatus::Denied
        );
    }

    /// Apple's `AVAuthorizationStatus` constant `3` → `Authorized`.
    #[test]
    fn decode_authorization_status_three_is_authorized() {
        assert_eq!(
            decode_authorization_status(3),
            AVAuthorizationStatus::Authorized
        );
    }

    /// Anything Apple did not document — negative ints, `isize::MAX`,
    /// future Apple additions — must collapse to `NotDetermined`. The
    /// fallback prompts the user instead of silently treating the
    /// unknown state as `Authorized`, which would be a security
    /// regression.
    #[test]
    fn decode_authorization_status_unknown_values_fall_back_to_not_determined() {
        for raw in [-1, 4, 100, isize::MIN, isize::MAX] {
            assert_eq!(
                decode_authorization_status(raw),
                AVAuthorizationStatus::NotDetermined,
                "raw={raw} must fall back to NotDetermined"
            );
        }
    }
}
