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

#![deny(clippy::pedantic)]
#![warn(clippy::all)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]

//! # nokhwa-bindings-uvc
//!
//! Cross-platform UVC (USB Video Class) bindings for
//! [`nokhwa`](https://crates.io/crates/nokhwa). Device enumeration only in
//! this release — streaming, format negotiation, and control surface land
//! in subsequent sessions. See the root project `TODO.md` for the roadmap.
//!
//! This crate is consumed through `nokhwa` with feature `input-uvc`. Do
//! not depend on it directly.

#[cfg(all(
    any(target_os = "linux", target_os = "macos", target_os = "windows"),
    not(feature = "docs-only")
))]
mod internal {
    use nokhwa_core::{
        buffer::Buffer,
        error::NokhwaError,
        traits::{CameraDevice, FrameSource},
        types::{
            ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueSetter,
            FrameFormat, KnownCameraControl, RequestedFormat,
        },
    };
    use rusb::UsbContext;
    use std::{borrow::Cow, time::Duration};

    /// USB Interface Class code for video devices (per USB-IF class codes).
    /// See <https://www.usb.org/defined-class-codes>.
    const USB_CLASS_VIDEO: u8 = 0x0E;
    /// Interface Subclass 0x01 = `VideoControl` — every UVC device exposes at
    /// least one `VideoControl` interface, so matching on it is the canonical
    /// way to discriminate UVC from other USB devices.
    const USB_SUBCLASS_VIDEOCONTROL: u8 = 0x01;
    /// libusb probe timeout for string descriptors. Short-but-nonzero: a
    /// stale descriptor read should not stall enumeration.
    const STRING_DESCRIPTOR_TIMEOUT: Duration = Duration::from_millis(100);

    /// Enumerate all UVC-class USB devices visible to libusb.
    ///
    /// Returns a [`CameraInfo`] per matching device, with:
    /// - `human_name` populated from the USB iProduct string when
    ///   available, else `"UVC <vid>:<pid>"`.
    /// - `description` populated from the USB iManufacturer string.
    /// - `misc` carrying `"<bus>:<address> <vid>:<pid>"` for downstream
    ///   session-2 code to reopen the device without re-scanning.
    /// - `index` as a monotonic `CameraIndex::Index(n)` in enumeration
    ///   order so cross-backend code using `CameraIndex::Index(0)` still
    ///   targets the first device.
    pub fn query() -> Result<Vec<CameraInfo>, NokhwaError> {
        let context = rusb::Context::new()
            .map_err(|why| NokhwaError::general(format!("libusb context init failed: {why}")))?;

        let devices = context.devices().map_err(|why| {
            NokhwaError::general(format!("libusb device enumeration failed: {why}"))
        })?;

        let mut cameras = Vec::new();
        let mut index_counter: u32 = 0;

        for device in devices.iter() {
            let Ok(descriptor) = device.device_descriptor() else {
                continue;
            };

            if !device_is_uvc(&device, &descriptor) {
                continue;
            }

            let vid = descriptor.vendor_id();
            let pid = descriptor.product_id();
            let bus = device.bus_number();
            let addr = device.address();

            // Opening a handle is needed only to read the string
            // descriptors. Failure here is not fatal — the device is
            // still UVC, we just fall back to the vid:pid placeholder
            // name.
            let (product, manufacturer) = match device.open() {
                Ok(handle) => (
                    handle
                        .read_product_string_ascii(&descriptor)
                        .ok()
                        .filter(|s| !s.is_empty())
                        .or_else(|| {
                            handle
                                .read_languages(STRING_DESCRIPTOR_TIMEOUT)
                                .ok()
                                .and_then(|langs| langs.first().copied())
                                .and_then(|lang| {
                                    handle
                                        .read_product_string(
                                            lang,
                                            &descriptor,
                                            STRING_DESCRIPTOR_TIMEOUT,
                                        )
                                        .ok()
                                })
                        }),
                    handle
                        .read_manufacturer_string_ascii(&descriptor)
                        .ok()
                        .filter(|s| !s.is_empty()),
                ),
                Err(_) => (None, None),
            };

            let human_name = product.unwrap_or_else(|| format!("UVC {vid:04x}:{pid:04x}"));
            let description = manufacturer.unwrap_or_default();
            let misc = format!("{bus}:{addr} {vid:04x}:{pid:04x}");

            cameras.push(CameraInfo::new(
                &human_name,
                &description,
                &misc,
                CameraIndex::Index(index_counter),
            ));
            index_counter = index_counter.saturating_add(1);
        }

        Ok(cameras)
    }

    /// Walks the device's configuration descriptors looking for an
    /// interface descriptor with class = VIDEO and subclass = VIDEOCONTROL.
    /// Devices that expose `VideoStreaming` without `VideoControl` are not
    /// well-formed UVC, so we intentionally skip them.
    fn device_is_uvc<T: rusb::UsbContext>(
        device: &rusb::Device<T>,
        descriptor: &rusb::DeviceDescriptor,
    ) -> bool {
        // Composite devices declare class 0xEF / 0xFF at the device level
        // and push the real class down to the interface descriptor. Check
        // both levels so we catch single-function webcams and composite
        // devices that expose an audio + video interface pair.
        if descriptor.class_code() == USB_CLASS_VIDEO {
            return true;
        }
        for cfg_idx in 0..descriptor.num_configurations() {
            let Ok(config) = device.config_descriptor(cfg_idx) else {
                continue;
            };
            for interface in config.interfaces() {
                for alt in interface.descriptors() {
                    if alt.class_code() == USB_CLASS_VIDEO
                        && alt.sub_class_code() == USB_SUBCLASS_VIDEOCONTROL
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Cross-platform UVC capture device. **Stream support is unimplemented
    /// in this release.** `query()` is fully functional; `new()` and every
    /// `FrameSource` / `CameraDevice` method currently returns
    /// [`NokhwaError::NotImplementedError`] so the backend can already be
    /// compiled, feature-gated, and registered with `nokhwa_backend!` while
    /// the streaming surface is iterated in follow-up work.
    ///
    /// Track progress against the UVC backlog item in the project
    /// `TODO.md`.
    pub struct UVCCaptureDevice {
        info: CameraInfo,
    }

    impl UVCCaptureDevice {
        pub fn new(_index: &CameraIndex, _cam_fmt: RequestedFormat) -> Result<Self, NokhwaError> {
            Err(NokhwaError::NotImplementedError(
                "UVC streaming not yet implemented — query() is available; \
                 see TODO.md for the session roadmap"
                    .to_string(),
            ))
        }
    }

    impl CameraDevice for UVCCaptureDevice {
        fn backend(&self) -> ApiBackend {
            ApiBackend::UniversalVideoClass
        }

        fn info(&self) -> &CameraInfo {
            &self.info
        }

        fn controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }

        fn set_control(
            &mut self,
            _id: KnownCameraControl,
            _value: ControlValueSetter,
        ) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }
    }

    impl FrameSource for UVCCaptureDevice {
        fn negotiated_format(&self) -> CameraFormat {
            CameraFormat::default()
        }

        fn set_format(&mut self, _f: CameraFormat) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }

        fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }

        fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }

        fn open(&mut self) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }

        fn is_open(&self) -> bool {
            false
        }

        fn frame(&mut self) -> Result<Buffer, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }

        fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }

        fn close(&mut self) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }
    }
}

#[cfg(any(
    not(any(target_os = "linux", target_os = "macos", target_os = "windows")),
    feature = "docs-only"
))]
mod internal {
    use nokhwa_core::{
        buffer::Buffer,
        error::NokhwaError,
        traits::{CameraDevice, FrameSource},
        types::{
            ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueSetter,
            FrameFormat, KnownCameraControl, RequestedFormat,
        },
    };
    use std::borrow::Cow;

    /// Stub [`query`] for targets without libusb support.
    pub fn query() -> Result<Vec<CameraInfo>, NokhwaError> {
        Err(NokhwaError::NotImplementedError(
            "UVC (libusb) is not available on this target".to_string(),
        ))
    }

    /// Platform-unsupported stub for [`UVCCaptureDevice`]. Every method
    /// errors with [`NokhwaError::NotImplementedError`].
    pub struct UVCCaptureDevice;

    #[allow(unused_variables)]
    impl UVCCaptureDevice {
        pub fn new(index: &CameraIndex, cam_fmt: RequestedFormat) -> Result<Self, NokhwaError> {
            Err(NokhwaError::NotImplementedError(
                "UVC (libusb) is not available on this target".to_string(),
            ))
        }
    }

    #[allow(unused_variables)]
    impl CameraDevice for UVCCaptureDevice {
        fn backend(&self) -> ApiBackend {
            ApiBackend::UniversalVideoClass
        }

        fn info(&self) -> &CameraInfo {
            unreachable!("UVC stub: UVCCaptureDevice::new always fails on this target")
        }

        fn controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }

        fn set_control(
            &mut self,
            id: KnownCameraControl,
            value: ControlValueSetter,
        ) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }
    }

    #[allow(unused_variables)]
    impl FrameSource for UVCCaptureDevice {
        fn negotiated_format(&self) -> CameraFormat {
            CameraFormat::default()
        }

        fn set_format(&mut self, f: CameraFormat) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }

        fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }

        fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }

        fn open(&mut self) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }

        fn is_open(&self) -> bool {
            false
        }

        fn frame(&mut self) -> Result<Buffer, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }

        fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }

        fn close(&mut self) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::UniversalVideoClass,
            ))
        }
    }
}

pub use internal::*;

#[cfg(all(
    test,
    any(target_os = "linux", target_os = "macos", target_os = "windows"),
    not(feature = "docs-only")
))]
mod tests {
    use super::internal::query;

    /// Smoke test: `query()` must not panic and must return an `Ok` on a
    /// host where libusb can initialise. The test does not require any
    /// UVC device to be present — an empty `Vec` is a valid result. On
    /// CI runners without USB access `libusb_init` may fail; we accept
    /// that case as an `Err(general(...))` rather than a panic.
    #[test]
    fn query_does_not_panic() {
        match query() {
            Ok(cameras) => {
                eprintln!("uvc::query() -> {} camera(s)", cameras.len());
                for cam in &cameras {
                    eprintln!(
                        "  {} | {} | {}",
                        cam.human_name(),
                        cam.description(),
                        cam.misc()
                    );
                }
            }
            Err(e) => eprintln!("uvc::query() errored (accepted): {e}"),
        }
    }
}
