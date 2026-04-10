use crate::ffi::AVMediaTypeVideo;
use crate::ffi::CMSampleBufferRef;
use crate::ffi::{
    dispatch_queue_create, CMSampleBufferGetImageBuffer, CMSampleBufferGetPresentationTimeStamp,
    CVImageBufferRef, CVPixelBufferGetBaseAddress, CVPixelBufferGetDataSize,
    CVPixelBufferGetPixelFormatType, CVPixelBufferLockBaseAddress, CVPixelBufferUnlockBaseAddress,
    NSObject,
};
use crate::types::{AVAuthorizationStatus, AVMediaType};
use crate::util::raw_fcc_to_frameformat;
use block2::RcBlock;
use flume::Sender;
use nokhwa_core::{error::NokhwaError, types::FrameFormat};
use objc2::runtime::{AnyClass, AnyObject, AnyProtocol, Bool, ClassBuilder, Sel};
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
        unsafe { mach_timebase_info(&mut info) };
        (info.numer, info.denom)
    });
    let ticks = unsafe { mach_absolute_time() };
    let (numer, denom) = *TIMEBASE;
    ticks.wrapping_mul(u64::from(numer)) / u64::from(denom)
}

static CALLBACK_CLASS: LazyLock<&'static AnyClass> = LazyLock::new(|| {
    let superclass = objc2::class!(NSObject);
    let mut builder = ClassBuilder::new(c"MyCaptureCallback", superclass).unwrap();

    // Ivar to hold a type-erased pointer to the Arc<Sender> for frame data
    builder.add_ivar::<*const c_void>(c"_arcmutptr");

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

        let buffer_as_vec = unsafe {
            std::slice::from_raw_parts(buffer_ptr as *const u8, buffer_length as usize).to_vec()
        };

        let pixel_format = unsafe { CVPixelBufferGetPixelFormatType(image_buffer) };
        let frame_format = raw_fcc_to_frameformat(pixel_format).unwrap_or(FrameFormat::YUYV);

        unsafe { CVPixelBufferUnlockBaseAddress(image_buffer, 0) };

        // Compute sensor capture timestamp from CMSampleBuffer presentation time
        let capture_ts = {
            let pts = unsafe { CMSampleBufferGetPresentationTimeStamp(didOutputSampleBuffer) };
            if pts.timescale > 0 {
                let pts_nanos =
                    (pts.value as u128).saturating_mul(1_000_000_000) / (pts.timescale as u128);
                let mono_now_nanos = u128::from(mach_absolute_time_nanos());
                let wall_now = std::time::SystemTime::now();

                let age = Duration::from_nanos(mono_now_nanos.saturating_sub(pts_nanos) as u64);
                wall_now
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .and_then(|wall_dur| wall_dur.checked_sub(age))
            } else {
                None
            }
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

pub fn request_permission_with_callback(callback: impl Fn(bool) + Send + Sync + 'static) {
    let cls = objc2::class!(AVCaptureDevice);

    let wrapper = move |b: Bool| {
        callback(b.as_bool());
    };

    let objc_fn_pass = RcBlock::new(wrapper);

    unsafe {
        let _: () = objc2::msg_send![cls, requestAccessForMediaType: (AVMediaTypeVideo.clone()), completionHandler: &*objc_fn_pass];
    }
}

pub fn current_authorization_status() -> AVAuthorizationStatus {
    let cls = objc2::class!(AVCaptureDevice);
    unsafe {
        objc2::msg_send![cls, authorizationStatusForMediaType:AVMediaType::Video.into_ns_str()]
    }
}

/// Wraps an Objective-C delegate and GCD dispatch queue for receiving video frames.
///
/// # Thread Safety
/// This type is `!Send + !Sync` because it holds raw ObjC pointers. The delegate
/// and queue are only safe to use from the thread that created them or from the
/// GCD dispatch queue associated with the AVCaptureSession.
pub struct AVCaptureVideoCallback {
    pub(crate) delegate: *mut AnyObject,
    pub(crate) queue: NSObject,
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

        let queue =
            unsafe { dispatch_queue_create(device_spec.as_ptr(), NSObject(std::ptr::null_mut())) };

        Ok(AVCaptureVideoCallback { delegate, queue })
    }

    pub fn inner(&self) -> *mut AnyObject {
        self.delegate
    }

    pub fn queue(&self) -> &NSObject {
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
        use crate::ffi::dispatch_release;
        unsafe {
            dispatch_release(self.queue.clone());
        }
    }
}
