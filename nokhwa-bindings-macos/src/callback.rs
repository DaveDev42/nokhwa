use crate::ffi::AVMediaTypeVideo;
use crate::ffi::{
    dispatch_queue_create, CMSampleBufferGetImageBuffer, CVImageBufferRef,
    CVPixelBufferGetBaseAddress, CVPixelBufferGetDataSize, CVPixelBufferGetPixelFormatType,
    CVPixelBufferLockBaseAddress, CVPixelBufferUnlockBaseAddress, NSObject,
};
use crate::types::{AVAuthorizationStatus, AVMediaType};
use crate::util::raw_fcc_to_frameformat;
use block2::RcBlock;
use core_media_sys::CMSampleBufferRef;
use flume::Sender;
use nokhwa_core::{error::NokhwaError, types::FrameFormat};
use objc2::runtime::{AnyClass, AnyObject, AnyProtocol, Bool, ClassBuilder, Sel};
use std::{
    ffi::{c_void, CStr},
    sync::{Arc, LazyLock},
};

static CALLBACK_CLASS: LazyLock<&'static AnyClass> = LazyLock::new(|| {
    let superclass = objc2::class!(NSObject);
    let mut builder = ClassBuilder::new(c"MyCaptureCallback", superclass).unwrap();

    // frame stack
    // oooh scary provenance-breaking BULLSHIT AAAAAA I LOVE TYPE ERASURE
    builder.add_ivar::<*const c_void>(c"_arcmutptr"); // ArkMutex, the not-arknights totally not gacha totally not ripoff new vidya game from l-pleasestop-npengtul

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
            std::slice::from_raw_parts_mut(buffer_ptr as *mut u8, buffer_length as usize).to_vec()
        };

        let pixel_format = unsafe { CVPixelBufferGetPixelFormatType(image_buffer) };
        let frame_format = raw_fcc_to_frameformat(pixel_format).unwrap_or(FrameFormat::YUYV);

        unsafe { CVPixelBufferUnlockBaseAddress(image_buffer, 0) };

        let bufferlck_cv: *const c_void = unsafe { objc2::msg_send![this, bufferPtr] };
        let buffer_sndr = unsafe {
            let ptr = bufferlck_cv.cast::<Sender<(Vec<u8>, FrameFormat)>>();
            Arc::from_raw(ptr)
        };
        let _ = buffer_sndr.send((buffer_as_vec, frame_format));
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
    let status: AVAuthorizationStatus = unsafe {
        objc2::msg_send![cls, authorizationStatusForMediaType:AVMediaType::Video.into_ns_str()]
    };
    status
}

pub struct AVCaptureVideoCallback {
    pub(crate) delegate: *mut AnyObject,
    pub(crate) queue: NSObject,
}

impl AVCaptureVideoCallback {
    pub fn new(
        device_spec: &CStr,
        buffer: &Arc<Sender<(Vec<u8>, FrameFormat)>>,
    ) -> Result<Self, NokhwaError> {
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
