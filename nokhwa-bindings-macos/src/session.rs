use crate::callback::AVCaptureVideoCallback;
use crate::device::{get_raw_device_info, AVCaptureDevice};
use crate::ffi::AVMediaTypeVideo;
use crate::types::AVCaptureDeviceType;
use crate::util::{create_boilerplate_impl, vec_to_ns_arr};
use core_media_sys::{
    kCMPixelFormat_24RGB, kCMPixelFormat_422YpCbCr8_yuvs, kCMPixelFormat_8IndexedGray_WhiteIsZero,
    kCMVideoCodecType_JPEG,
};
use core_video_sys::{
    kCVPixelBufferPixelFormatTypeKey, kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange,
};
use nokhwa_core::{
    error::NokhwaError,
    types::{CameraIndex, CameraInfo, FrameFormat},
};
use objc2::runtime::AnyObject;

create_boilerplate_impl! {
    [pub AVCaptureDeviceDiscoverySession],
    [pub AVCaptureDeviceInput],
    [pub AVCaptureSession]
}

impl AVCaptureDeviceDiscoverySession {
    pub fn new(device_types: Vec<AVCaptureDeviceType>) -> Result<Self, NokhwaError> {
        let device_types = vec_to_ns_arr(device_types);
        let position: isize = 0;

        let media_type_video = unsafe { AVMediaTypeVideo.clone() }.0;

        let discovery_session_cls = objc2::class!(AVCaptureDeviceDiscoverySession);
        let discovery_session: *mut AnyObject = unsafe {
            objc2::msg_send![discovery_session_cls, discoverySessionWithDeviceTypes:device_types, mediaType:media_type_video, position:position]
        };

        Ok(AVCaptureDeviceDiscoverySession {
            inner: discovery_session,
        })
    }

    pub fn default() -> Result<Self, NokhwaError> {
        AVCaptureDeviceDiscoverySession::new(vec![
            AVCaptureDeviceType::UltraWide,
            AVCaptureDeviceType::Telephoto,
            AVCaptureDeviceType::External,
            AVCaptureDeviceType::Dual,
            AVCaptureDeviceType::DualWide,
            AVCaptureDeviceType::Triple,
        ])
    }

    pub fn devices(&self) -> Vec<CameraInfo> {
        let device_ns_array: *mut AnyObject = unsafe { objc2::msg_send![self.inner, devices] };
        let objects_len: usize = unsafe { objc2::msg_send![device_ns_array, count] };
        let mut devices = Vec::with_capacity(objects_len);
        for index in 0..objects_len {
            let device: *mut AnyObject =
                unsafe { objc2::msg_send![device_ns_array, objectAtIndex: index] };
            devices.push(get_raw_device_info(
                CameraIndex::Index(index as u32),
                device,
            ));
        }

        devices
    }
}

impl AVCaptureDeviceInput {
    pub fn new(capture_device: &AVCaptureDevice) -> Result<Self, NokhwaError> {
        let cls = objc2::class!(AVCaptureDeviceInput);
        let err_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let capture_input: *mut AnyObject = unsafe {
            let allocated: *mut AnyObject = objc2::msg_send![cls, alloc];
            objc2::msg_send![allocated, initWithDevice:capture_device.inner(), error:err_ptr]
        };
        if !err_ptr.is_null() {
            return Err(NokhwaError::InitializeError {
                backend: nokhwa_core::types::ApiBackend::AVFoundation,
                error: "Failed to create input".to_string(),
            });
        }

        Ok(AVCaptureDeviceInput {
            inner: capture_input,
        })
    }
}

pub struct AVCaptureVideoDataOutput {
    inner: *mut AnyObject,
}

impl AVCaptureVideoDataOutput {
    pub fn new() -> Self {
        AVCaptureVideoDataOutput::default()
    }

    pub fn add_delegate(&self, delegate: &AVCaptureVideoCallback) -> Result<(), NokhwaError> {
        unsafe {
            let _: () = objc2::msg_send![
                self.inner,
                setSampleBufferDelegate: delegate.delegate,
                queue: delegate.queue().0
            ];
        };
        Ok(())
    }

    pub fn set_frame_format(&self, format: FrameFormat) -> Result<(), NokhwaError> {
        let cmpixelfmt = match format {
            FrameFormat::YUYV => kCMPixelFormat_422YpCbCr8_yuvs,
            FrameFormat::MJPEG => kCMVideoCodecType_JPEG,
            FrameFormat::GRAY => kCMPixelFormat_8IndexedGray_WhiteIsZero,
            FrameFormat::NV12 => kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange,
            FrameFormat::RAWRGB => kCMPixelFormat_24RGB,
            FrameFormat::RAWBGR => {
                return Err(NokhwaError::SetPropertyError {
                    property: "setVideoSettings".to_string(),
                    value: "set frame format".to_string(),
                    error: "Unsupported frame format BGR".to_string(),
                });
            }
        };

        // Create NSNumber from the pixel format value
        let ns_number_cls = objc2::class!(NSNumber);
        let obj: *mut AnyObject =
            unsafe { objc2::msg_send![ns_number_cls, numberWithInt: cmpixelfmt as i32] };
        let key = unsafe { kCVPixelBufferPixelFormatTypeKey } as *mut AnyObject;

        // Create NSDictionary with the single key-value pair
        let dict_cls = objc2::class!(NSDictionary);
        let dict: *mut AnyObject =
            unsafe { objc2::msg_send![dict_cls, dictionaryWithObject:obj, forKey:key] };
        let _: () = unsafe { objc2::msg_send![self.inner, setVideoSettings:dict] };
        Ok(())
    }
}

impl Default for AVCaptureVideoDataOutput {
    fn default() -> Self {
        let cls = objc2::class!(AVCaptureVideoDataOutput);
        let inner: *mut AnyObject = unsafe { objc2::msg_send![cls, new] };

        AVCaptureVideoDataOutput { inner }
    }
}

impl AVCaptureSession {
    pub fn new() -> Self {
        AVCaptureSession::default()
    }

    pub fn begin_configuration(&self) {
        unsafe { objc2::msg_send![self.inner, beginConfiguration] }
    }

    pub fn commit_configuration(&self) {
        unsafe { objc2::msg_send![self.inner, commitConfiguration] }
    }

    pub fn can_add_input(&self, input: &AVCaptureDeviceInput) -> bool {
        let result: bool = unsafe { objc2::msg_send![self.inner, canAddInput:input.inner] };
        result
    }

    pub fn add_input(&self, input: &AVCaptureDeviceInput) -> Result<(), NokhwaError> {
        if self.can_add_input(input) {
            let _: () = unsafe { objc2::msg_send![self.inner, addInput:input.inner] };
            return Ok(());
        }
        Err(NokhwaError::SetPropertyError {
            property: "AVCaptureDeviceInput".to_string(),
            value: "add new input".to_string(),
            error: "Rejected".to_string(),
        })
    }

    pub fn remove_input(&self, input: &AVCaptureDeviceInput) {
        unsafe { objc2::msg_send![self.inner, removeInput:input.inner] }
    }

    pub fn can_add_output(&self, output: &AVCaptureVideoDataOutput) -> bool {
        let result: bool = unsafe { objc2::msg_send![self.inner, canAddOutput:output.inner] };
        result
    }

    pub fn add_output(&self, output: &AVCaptureVideoDataOutput) -> Result<(), NokhwaError> {
        if self.can_add_output(output) {
            let _: () = unsafe { objc2::msg_send![self.inner, addOutput:output.inner] };
            return Ok(());
        }
        Err(NokhwaError::SetPropertyError {
            property: "AVCaptureVideoDataOutput".to_string(),
            value: "add new output".to_string(),
            error: "Rejected".to_string(),
        })
    }

    pub fn remove_output(&self, output: &AVCaptureVideoDataOutput) {
        unsafe { objc2::msg_send![self.inner, removeOutput:output.inner] }
    }

    pub fn is_running(&self) -> bool {
        let running: bool = unsafe { objc2::msg_send![self.inner, isRunning] };
        running
    }

    pub fn start(&self) -> Result<(), NokhwaError> {
        let inner = self.inner;
        let start_stream_fn = std::panic::AssertUnwindSafe(move || {
            let _: () = unsafe { objc2::msg_send![inner, startRunning] };
        });

        if std::panic::catch_unwind(start_stream_fn).is_err() {
            return Err(NokhwaError::OpenStreamError(
                "Cannot run AVCaptureSession".to_string(),
            ));
        }
        Ok(())
    }

    pub fn stop(&self) {
        unsafe { objc2::msg_send![self.inner, stopRunning] }
    }

    pub fn is_interrupted(&self) -> bool {
        let interrupted: bool = unsafe { objc2::msg_send![self.inner, isInterrupted] };
        interrupted
    }
}

impl Default for AVCaptureSession {
    fn default() -> Self {
        let cls = objc2::class!(AVCaptureSession);
        let session: *mut AnyObject = {
            let alloc: *mut AnyObject = unsafe { objc2::msg_send![cls, alloc] };
            unsafe { objc2::msg_send![alloc, init] }
        };
        AVCaptureSession { inner: session }
    }
}
