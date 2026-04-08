use super::*;

pub(crate) static CALLBACK_CLASS: Lazy<&'static Class> = Lazy::new(|| {
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
