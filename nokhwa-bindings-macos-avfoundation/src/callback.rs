use crate::ffi::CMSampleBufferRef;
use crate::ffi::{
    dispatch_queue_create, dispatch_release, CMSampleBufferGetImageBuffer,
    CMSampleBufferGetPresentationTimeStamp, CVImageBufferRef, CVPixelBufferGetBaseAddress,
    CVPixelBufferGetDataSize, CVPixelBufferGetPixelFormatType, CVPixelBufferLockBaseAddress,
    CVPixelBufferUnlockBaseAddress, DispatchQueue,
};
use crate::types::{AVAuthorizationStatus, AVMediaTypeLocal};
use crate::util::raw_fcc_to_frameformat;
use block2::RcBlock;
use nokhwa_core::{error::NokhwaError, types::FrameFormat};
use objc2::runtime::{AnyClass, AnyObject, AnyProtocol, Bool, ClassBuilder, Sel};
use objc2_av_foundation::{AVCaptureDevice, AVMediaTypeVideo};
use std::sync::mpsc::Sender;
use std::{
    ffi::{c_void, CStr},
    sync::{Arc, LazyLock},
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
    static TIMEBASE: LazyLock<(u32, u32)> = LazyLock::new(|| {
        let mut info = MachTimebaseInfo { numer: 0, denom: 0 };
        unsafe { mach_timebase_info(&raw mut info) };
        (info.numer, info.denom)
    });
    let ticks = unsafe { mach_absolute_time() };
    let (numer, denom) = *TIMEBASE;
    ticks.wrapping_mul(u64::from(numer)) / u64::from(denom)
}

// Hoist all `extern "C" fn` items to the top of the module scope so they
// are not flagged as items_after_statements inside the LazyLock closure.
// The functions are still only referenced inside the closure.

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

// TODO: Migrate to DeclaredClass + Ivar API when get_ivar/get_mut_ivar are removed in a future objc2 release
#[allow(deprecated)]
extern "C" fn my_callback_get_arcmutptr(this: *mut AnyObject, _: Sel) -> *const c_void {
    unsafe { *(*this).get_ivar("_arcmutptr") }
}

#[allow(deprecated)]
extern "C" fn my_callback_set_arcmutptr(
    this: *mut AnyObject,
    _: Sel,
    new_arcmutptr: *const c_void,
) {
    unsafe {
        *(*this).get_mut_ivar("_arcmutptr") = new_arcmutptr;
    }
}

// Delegate compliance method
// SAFETY: Reads pixel data from CVPixelBuffer while base address lock is held.
// The lock guarantees buffer_ptr is valid and buffer_length bytes are readable.
// cast_possible_truncation, cast_sign_loss: CoreMedia timestamps are i64/i32;
// u128 arithmetic is safe here because the values are always non-negative in
// practice (presentation times from a running capture session). The final
// saturating_sub result is bounded by the session uptime, well within u64::MAX.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
extern "C" fn capture_out_callback(
    this: *mut AnyObject,
    _: Sel,
    _: *mut AnyObject,
    didOutputSampleBuffer: CMSampleBufferRef,
    _: *mut AnyObject,
) {
    let image_buffer: CVImageBufferRef =
        unsafe { CMSampleBufferGetImageBuffer(didOutputSampleBuffer) };

    if image_buffer.is_null() {
        return;
    }

    unsafe {
        CVPixelBufferLockBaseAddress(image_buffer, 0);
    };

    let buffer_length = unsafe { CVPixelBufferGetDataSize(image_buffer) };
    let buffer_ptr = unsafe { CVPixelBufferGetBaseAddress(image_buffer) };

    if buffer_ptr.is_null() || buffer_length == 0 {
        unsafe { CVPixelBufferUnlockBaseAddress(image_buffer, 0) };
        return;
    }

    // CVPixelBufferGetDataSize returns c_ulong (usize on 64-bit Apple platforms).
    // cast_possible_truncation on usize→usize is a no-op; the allow above covers
    // the cross-size target warning.
    let buffer_as_vec = unsafe {
        std::slice::from_raw_parts(buffer_ptr as *const u8, buffer_length as usize).to_vec()
    };

    let pixel_format = unsafe { CVPixelBufferGetPixelFormatType(image_buffer) };
    let frame_format = raw_fcc_to_frameformat(pixel_format).unwrap_or(FrameFormat::YUYV);

    unsafe { CVPixelBufferUnlockBaseAddress(image_buffer, 0) };

    // Compute sensor capture timestamp from CMSampleBuffer presentation time
    let capture_ts = {
        let pts = unsafe { CMSampleBufferGetPresentationTimeStamp(didOutputSampleBuffer) };
        pts_to_wallclock(
            pts,
            mach_absolute_time_nanos(),
            std::time::SystemTime::now(),
        )
    };

    let bufferlck_cv: *const c_void = unsafe { objc2::msg_send![this, bufferPtr] };
    let buffer_sndr = unsafe {
        let ptr = bufferlck_cv.cast::<Sender<FrameData>>();
        Arc::from_raw(ptr)
    };
    let _ = buffer_sndr.send((buffer_as_vec, frame_format, capture_ts));
    std::mem::forget(buffer_sndr);
}

#[allow(non_snake_case)]
extern "C" fn capture_drop_callback(
    _: *mut AnyObject,
    _: Sel,
    _: *mut AnyObject,
    _: *mut AnyObject,
    _: *mut AnyObject,
) {
}

static CALLBACK_CLASS: LazyLock<&'static AnyClass> = LazyLock::new(|| {
    let superclass = objc2::class!(NSObject);
    let mut builder = ClassBuilder::new(c"MyCaptureCallback", superclass).unwrap();

    // Ivar to hold a type-erased pointer to the Arc<Sender> for frame data
    builder.add_ivar::<*const c_void>(c"_arcmutptr");

    unsafe {
        builder.add_method(
            objc2::sel!(bufferPtr),
            my_callback_get_arcmutptr as extern "C" fn(*mut AnyObject, Sel) -> *const c_void,
        );
        builder.add_method(
            objc2::sel!(setBufferPtr:),
            my_callback_set_arcmutptr as extern "C" fn(*mut AnyObject, Sel, *const c_void),
        );
        builder.add_method(
            objc2::sel!(captureOutput:didOutputSampleBuffer:fromConnection:),
            capture_out_callback
                as extern "C" fn(
                    *mut AnyObject,
                    Sel,
                    *mut AnyObject,
                    CMSampleBufferRef,
                    *mut AnyObject,
                ),
        );
        builder.add_method(
            objc2::sel!(captureOutput:didDropSampleBuffer:fromConnection:),
            capture_drop_callback
                as extern "C" fn(
                    *mut AnyObject,
                    Sel,
                    *mut AnyObject,
                    *mut AnyObject,
                    *mut AnyObject,
                ),
        );

        builder.add_protocol(
            AnyProtocol::get(c"AVCaptureVideoDataOutputSampleBufferDelegate").unwrap(),
        );
    }

    builder.register()
});

/// Requests camera access permission from the user.
///
/// # Panics
///
/// Panics if the `AVMediaTypeVideo` constant is unavailable on the current
/// platform, which should not happen on any supported Apple platform.
pub fn request_permission_with_callback(callback: impl Fn(bool) + Send + Sync + 'static) {
    let media_type = unsafe { AVMediaTypeVideo.unwrap() };

    let wrapper = move |b: Bool| {
        callback(b.as_bool());
    };

    let objc_fn_pass = RcBlock::new(wrapper);

    unsafe {
        AVCaptureDevice::requestAccessForMediaType_completionHandler(media_type, &objc_fn_pass);
    }
}

#[must_use]
pub fn current_authorization_status() -> AVAuthorizationStatus {
    let media_type = AVMediaTypeLocal::Video.to_av_media_type();
    let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type) };
    // Map from objc2_av_foundation::AVAuthorizationStatus(NSInteger) to our local enum.
    // Values match Apple's AVAuthorizationStatus enum:
    // https://developer.apple.com/documentation/avfoundation/avauthorizationstatus
    match status.0 {
        1 => AVAuthorizationStatus::Restricted,
        2 => AVAuthorizationStatus::Denied,
        3 => AVAuthorizationStatus::Authorized,
        _ => AVAuthorizationStatus::NotDetermined,
    }
}

/// Wraps an Objective-C delegate and GCD dispatch queue for receiving video frames.
///
/// # Thread Safety
/// This type holds raw `ObjC` pointers (`*mut AnyObject` delegate and `DispatchQueue`).
/// It is `!Send` by default due to the raw pointers, but the containing
/// `AVFoundationCaptureDevice` implements `Send` because GCD dispatch queues are
/// thread-safe and the delegate is managed by the session's dispatch queue.
pub struct AVCaptureVideoCallback {
    pub(crate) delegate: *mut AnyObject,
    pub(crate) queue: DispatchQueue,
}

impl AVCaptureVideoCallback {
    pub fn new(device_spec: &CStr, buffer: &Arc<Sender<FrameData>>) -> Result<Self, NokhwaError> {
        let cls = &CALLBACK_CLASS as &AnyClass;
        let delegate: *mut AnyObject = unsafe { objc2::msg_send![cls, alloc] };
        let delegate: *mut AnyObject = unsafe { objc2::msg_send![delegate, init] };
        let buffer_as_ptr = {
            let arc_raw = Arc::as_ptr(buffer);
            arc_raw.cast::<c_void>()
        };
        unsafe {
            let _: () = objc2::msg_send![delegate, setBufferPtr: buffer_as_ptr];
        }

        let queue = unsafe { dispatch_queue_create(device_spec.as_ptr(), std::ptr::null()) };

        Ok(AVCaptureVideoCallback { delegate, queue })
    }

    #[must_use]
    pub fn inner(&self) -> *mut AnyObject {
        self.delegate
    }

    #[must_use]
    pub fn queue(&self) -> &DispatchQueue {
        &self.queue
    }
}

impl Drop for AVCaptureVideoCallback {
    fn drop(&mut self) {
        if !self.delegate.is_null() {
            unsafe {
                let _: () = objc2::msg_send![self.delegate, release];
            }
        }
        if !self.queue.0.is_null() {
            unsafe {
                dispatch_release(DispatchQueue(self.queue.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::pts_to_wallclock;
    use crate::ffi::CMTime;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn cmtime(value: i64, timescale: i32) -> CMTime {
        CMTime {
            value,
            timescale,
            flags: 0,
            epoch: 0,
        }
    }

    /// `timescale == 0` is the documented "uninitialised CMTime"
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

    /// Happy path: a 1-second-old PTS (mono_now - pts_nanos = 1s)
    /// pins back to wall_now - 1s.
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

    /// Future PTS (pts_nanos > mono_now_nanos) — clock skew or a
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

    /// `age > wall_now` (e.g. mocked wall_now of 1 ns post-epoch
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
}
