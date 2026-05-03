/*
* Copyright 2022 l1npengtul <l1npengtul@protonmail.com> / The Nokhwa Contributors
*
* Licensed under the Apache License, Version 2.0 (the "License");
* you may not use this file except in compliance with the License.
* You may obtain a copy of the License at
*
*     http://www.apache.org/licenses/LICENSE-2.0
*
* Unless required by applicable law or agreed to in writing, software
* distributed under the License is distributed on an "AS IS" BASIS,
* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
* See the License for the specific language governing permissions and
* limitations under the License.
*/

// hello, future peng here
// whatever is written here will induce horrors uncomprehendable.
// save yourselves. write apple code in swift and bind it to rust.

// <some change so we can call this 0.10.4>

#![deny(clippy::pedantic)]
#![warn(clippy::all)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod callback;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod capture;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod device;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod ffi;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod session;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod types;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod util;

mod hotplug;
pub use hotplug::AVFoundationHotplugContext;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use callback::{
    current_authorization_status, request_permission_with_callback, AVCaptureVideoCallback,
};
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use device::{
    get_raw_device_info, query, AVCaptureDeviceFormatWrapper, AVCaptureDeviceWrapper,
    AVFrameRateRangeWrapper,
};
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use ffi::*;

// Re-export typed AVFoundation types for downstream use
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use capture::AVFoundationCaptureDevice;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use objc2::rc::Retained;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use objc2_av_foundation::{AVCaptureDeviceInput, AVCaptureSession, AVCaptureVideoDataOutput};
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use types::{AVAuthorizationStatus, AVCaptureDeviceTypeLocal, AVMediaTypeLocal};
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use util::{CompressionData, DataPipe};

/// Non-Apple stub for `AVFoundationCaptureDevice`.
///
/// Exists so cross-platform documentation builds (`cargo doc
/// --features docs-only,docs-nolink`) and downstream code that merely
/// references the type can compile on Linux / Windows hosts. Fallible
/// methods return [`NokhwaError::NotImplementedError`]; infallible
/// methods panic via `unreachable!()` — they cannot be reached in
/// practice because `AVFoundationCaptureDevice::new` errors off
/// macOS / iOS, so no value of this stub type can exist at runtime via
/// the public constructor path.
///
/// Mirrors the off-Linux / off-Windows stubs used by `V4LCaptureDevice`
/// and `MediaFoundationCaptureDevice`.
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
mod stub {
    use nokhwa_core::buffer::Buffer;
    use nokhwa_core::error::NokhwaError;
    use nokhwa_core::traits::{CameraDevice, FrameSource};
    use nokhwa_core::types::{
        ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueSetter,
        FrameFormat, KnownCameraControl, RequestedFormat,
    };
    use std::borrow::Cow;

    /// See module docs for behavior off macOS / iOS.
    pub struct AVFoundationCaptureDevice;

    /// Shared error for fallible stub methods.
    fn not_on_this_platform() -> NokhwaError {
        NokhwaError::NotImplementedError("AVFoundation only on macOS / iOS".to_string())
    }

    /// Shared panic for infallible stub methods. These methods cannot
    /// return an error and should never be called in practice because
    /// `AVFoundationCaptureDevice::new` errors off macOS / iOS, so no
    /// `AVFoundationCaptureDevice` value can be produced through the
    /// public constructor path.
    #[cold]
    #[inline(never)]
    fn stub_unreachable() -> ! {
        unreachable!("AVFoundation stub: only available on macOS / iOS")
    }

    #[allow(unused_variables)]
    impl AVFoundationCaptureDevice {
        /// Creates a new capture device using the `AVFoundation` backend.
        /// # Errors
        /// Always returns [`NokhwaError::NotImplementedError`] off
        /// macOS / iOS.
        pub fn new(index: &CameraIndex, req_fmt: RequestedFormat) -> Result<Self, NokhwaError> {
            Err(not_on_this_platform())
        }

        /// Look up a single control by its [`KnownCameraControl`] identifier.
        /// # Errors
        /// Always returns [`NokhwaError::NotImplementedError`] off
        /// macOS / iOS.
        pub fn camera_control(
            &self,
            control: KnownCameraControl,
        ) -> Result<CameraControl, NokhwaError> {
            Err(not_on_this_platform())
        }
    }

    #[allow(unused_variables)]
    impl CameraDevice for AVFoundationCaptureDevice {
        fn backend(&self) -> ApiBackend {
            ApiBackend::AVFoundation
        }

        fn info(&self) -> &CameraInfo {
            stub_unreachable()
        }

        fn controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
            Err(not_on_this_platform())
        }

        fn set_control(
            &mut self,
            id: KnownCameraControl,
            value: ControlValueSetter,
        ) -> Result<(), NokhwaError> {
            Err(not_on_this_platform())
        }
    }

    #[allow(unused_variables)]
    impl FrameSource for AVFoundationCaptureDevice {
        fn negotiated_format(&self) -> CameraFormat {
            stub_unreachable()
        }

        fn set_format(&mut self, f: CameraFormat) -> Result<(), NokhwaError> {
            Err(not_on_this_platform())
        }

        fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
            Err(not_on_this_platform())
        }

        fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
            Err(not_on_this_platform())
        }

        fn open(&mut self) -> Result<(), NokhwaError> {
            Err(not_on_this_platform())
        }

        fn is_open(&self) -> bool {
            false
        }

        fn frame(&mut self) -> Result<Buffer, NokhwaError> {
            Err(not_on_this_platform())
        }

        fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
            Err(not_on_this_platform())
        }

        fn close(&mut self) -> Result<(), NokhwaError> {
            Err(not_on_this_platform())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{not_on_this_platform, AVFoundationCaptureDevice};
        use nokhwa_core::error::NokhwaError;
        use nokhwa_core::format_types::Mjpeg;
        use nokhwa_core::traits::{CameraDevice, FrameSource};
        use nokhwa_core::types::{
            ApiBackend, CameraFormat, CameraIndex, ControlValueSetter, FrameFormat,
            KnownCameraControl, RequestedFormat, RequestedFormatType, Resolution,
        };

        // Pin the contract that the off-Apple stub never hands out a live
        // device. Mirrors the V4L / MSMF stub coverage. Without these
        // tests a future refactor could regress one of the methods to
        // `panic!()` / `todo!()` and the cross-platform docs build would
        // still pass.

        fn assert_not_implemented(err: &NokhwaError) {
            assert!(
                matches!(err, NokhwaError::NotImplementedError(_)),
                "expected NotImplementedError, got {err:?}",
            );
        }

        #[test]
        fn shared_error_helper_is_not_implemented() {
            assert_not_implemented(&not_on_this_platform());
        }

        #[test]
        fn new_errors_off_apple() {
            // `AVFoundationCaptureDevice` does not implement `Debug`.
            match AVFoundationCaptureDevice::new(
                &CameraIndex::Index(0),
                RequestedFormat::new::<Mjpeg>(RequestedFormatType::AbsoluteHighestFrameRate),
            ) {
                Err(err) => assert_not_implemented(&err),
                Ok(_) => panic!("stub `new` must always error off macOS / iOS"),
            }
        }

        #[test]
        fn camera_control_errors_off_apple() {
            let dev = AVFoundationCaptureDevice;
            match dev.camera_control(KnownCameraControl::Brightness) {
                Err(err) => assert_not_implemented(&err),
                Ok(_) => panic!("stub `camera_control` must always error off macOS / iOS"),
            }
        }

        #[test]
        fn backend_reports_avfoundation() {
            let dev = AVFoundationCaptureDevice;
            assert_eq!(dev.backend(), ApiBackend::AVFoundation);
        }

        #[test]
        fn camera_device_fallible_methods_return_not_implemented() {
            let mut dev = AVFoundationCaptureDevice;
            assert_not_implemented(&dev.controls().expect_err("stub controls() must error"));
            let err = dev
                .set_control(
                    KnownCameraControl::Brightness,
                    ControlValueSetter::Integer(0),
                )
                .expect_err("stub set_control() must error");
            assert_not_implemented(&err);
        }

        #[test]
        fn frame_source_fallible_methods_return_not_implemented() {
            let mut dev = AVFoundationCaptureDevice;
            assert_not_implemented(
                &dev.set_format(CameraFormat::new(
                    Resolution::new(640, 480),
                    FrameFormat::MJPEG,
                    30,
                ))
                .expect_err("stub set_format() must error"),
            );
            assert_not_implemented(
                &dev.compatible_formats()
                    .expect_err("stub compatible_formats() must error"),
            );
            assert_not_implemented(
                &dev.compatible_fourcc()
                    .expect_err("stub compatible_fourcc() must error"),
            );
            assert_not_implemented(&dev.open().expect_err("stub open() must error"));
            assert_not_implemented(&dev.frame().expect_err("stub frame() must error"));
            assert_not_implemented(&dev.frame_raw().expect_err("stub frame_raw() must error"));
            assert_not_implemented(&dev.close().expect_err("stub close() must error"));
        }

        #[test]
        fn is_open_reports_false() {
            let dev = AVFoundationCaptureDevice;
            assert!(!dev.is_open());
        }

        #[test]
        #[should_panic(expected = "AVFoundation stub: only available on macOS / iOS")]
        fn info_panics_via_stub_unreachable() {
            let dev = AVFoundationCaptureDevice;
            let _info = dev.info();
        }

        #[test]
        #[should_panic(expected = "AVFoundation stub: only available on macOS / iOS")]
        fn negotiated_format_panics_via_stub_unreachable() {
            let dev = AVFoundationCaptureDevice;
            let _fmt = dev.negotiated_format();
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub use stub::AVFoundationCaptureDevice;
