use crate::ffi::AVMediaTypeVideo;
use crate::ffi::{
    dispatch_queue_create, CMSampleBufferGetImageBuffer, CVImageBufferRef,
    CVPixelBufferGetBaseAddress, CVPixelBufferGetDataSize, CVPixelBufferGetPixelFormatType,
    CVPixelBufferLockBaseAddress, CVPixelBufferUnlockBaseAddress, NSObject,
};
use crate::types::{AVAuthorizationStatus, AVMediaType};
use crate::util::raw_fcc_to_frameformat;
use block::ConcreteBlock;
use core_media_sys::CMSampleBufferRef;
use flume::Sender;
use nokhwa_core::{error::NokhwaError, types::FrameFormat};
use objc::{
    declare::ClassDecl,
    runtime::{Class, Object, Protocol, Sel, BOOL, YES},
};
use once_cell::sync::Lazy;
use std::{
    ffi::{c_void, CStr},
    sync::Arc,
};

static CALLBACK_CLASS: Lazy<&'static Class> = Lazy::new(|| {
    {
        let mut decl = ClassDecl::new("MyCaptureCallback", class!(NSObject)).unwrap();

        // frame stack
        // oooh scary provenannce-breaking BULLSHIT AAAAAA I LOVE TYPE ERASURE
        decl.add_ivar::<*const c_void>("_arcmutptr"); // ArkMutex, the not-arknights totally not gacha totally not ripoff new vidya game from l-pleasestop-npengtul

        extern "C" fn my_callback_get_arcmutptr(this: &Object, _: Sel) -> *const c_void {
            unsafe { *this.get_ivar("_arcmutptr") }
        }
        extern "C" fn my_callback_set_arcmutptr(
            this: &mut Object,
            _: Sel,
            new_arcmutptr: *const c_void,
        ) {
            unsafe {
                this.set_ivar("_arcmutptr", new_arcmutptr);
            }
        }

        // Delegate compliance method
        // SAFETY: This assumes that the buffer byte size is a u8. Any other size will cause unsafety.
        #[allow(non_snake_case)]
        #[allow(non_upper_case_globals)]
        extern "C" fn capture_out_callback(
            this: &mut Object,
            _: Sel,
            _: *mut Object,
            didOutputSampleBuffer: CMSampleBufferRef,
            _: *mut Object,
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
                std::slice::from_raw_parts_mut(buffer_ptr as *mut u8, buffer_length as usize)
                    .to_vec()
            };

            let pixel_format = unsafe { CVPixelBufferGetPixelFormatType(image_buffer) };
            let frame_format = raw_fcc_to_frameformat(pixel_format).unwrap_or(FrameFormat::YUYV);

            unsafe { CVPixelBufferUnlockBaseAddress(image_buffer, 0) };

            let bufferlck_cv: *const c_void = unsafe { msg_send![this, bufferPtr] };
            let buffer_sndr = unsafe {
                let ptr = bufferlck_cv.cast::<Sender<(Vec<u8>, FrameFormat)>>();
                Arc::from_raw(ptr)
            };
            let _ = buffer_sndr.send((buffer_as_vec, frame_format));
            std::mem::forget(buffer_sndr);
        }

        #[allow(non_snake_case)]
        extern "C" fn capture_drop_callback(
            _: &mut Object,
            _: Sel,
            _: *mut Object,
            _: *mut Object,
            _: *mut Object,
        ) {
        }

        unsafe {
            decl.add_method(
                sel!(bufferPtr),
                my_callback_get_arcmutptr as extern "C" fn(&Object, Sel) -> *const c_void,
            );
            decl.add_method(
                sel!(SetBufferPtr:),
                my_callback_set_arcmutptr as extern "C" fn(&mut Object, Sel, *const c_void),
            );
            decl.add_method(
                sel!(captureOutput:didOutputSampleBuffer:fromConnection:),
                capture_out_callback
                    as extern "C" fn(&mut Object, Sel, *mut Object, CMSampleBufferRef, *mut Object),
            );
            decl.add_method(
                sel!(captureOutput:didDropSampleBuffer:fromConnection:),
                capture_drop_callback
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object, *mut Object),
            );

            decl.add_protocol(
                Protocol::get("AVCaptureVideoDataOutputSampleBufferDelegate").unwrap(),
            );
        }

        decl.register()
    }
});

pub fn request_permission_with_callback(callback: impl Fn(bool) + Send + Sync + 'static) {
    let cls = class!(AVCaptureDevice);

    let wrapper = move |bool: BOOL| {
        callback(bool == YES);
    };

    let objc_fn_block: ConcreteBlock<(BOOL,), (), _> = ConcreteBlock::new(wrapper);
    let objc_fn_pass = objc_fn_block.copy();

    unsafe {
        let _: () = msg_send![cls, requestAccessForMediaType:(AVMediaTypeVideo.clone()) completionHandler:objc_fn_pass];
    }
}

pub fn current_authorization_status() -> AVAuthorizationStatus {
    let cls = class!(AVCaptureDevice);
    let status: AVAuthorizationStatus =
        unsafe { msg_send![cls, authorizationStatusForMediaType:AVMediaType::Video.into_ns_str()] };
    status
}

pub struct AVCaptureVideoCallback {
    pub(crate) delegate: *mut Object,
    pub(crate) queue: NSObject,
}

impl AVCaptureVideoCallback {
    pub fn new(
        device_spec: &CStr,
        buffer: &Arc<Sender<(Vec<u8>, FrameFormat)>>,
    ) -> Result<Self, NokhwaError> {
        let cls = &CALLBACK_CLASS as &Class;
        let delegate: *mut Object = unsafe { msg_send![cls, alloc] };
        let delegate: *mut Object = unsafe { msg_send![delegate, init] };
        let buffer_as_ptr = {
            let arc_raw = Arc::as_ptr(buffer);
            arc_raw.cast::<c_void>()
        };
        unsafe {
            let _: () = msg_send![delegate, SetBufferPtr: buffer_as_ptr];
        }

        let queue =
            unsafe { dispatch_queue_create(device_spec.as_ptr(), NSObject(std::ptr::null_mut())) };

        Ok(AVCaptureVideoCallback { delegate, queue })
    }

    pub fn data_len(&self) -> usize {
        unsafe { msg_send![self.delegate, dataLength] }
    }

    pub fn inner(&self) -> *mut Object {
        self.delegate
    }

    pub fn queue(&self) -> &NSObject {
        &self.queue
    }
}
