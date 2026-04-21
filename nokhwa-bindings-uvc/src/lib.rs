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
//! [`nokhwa`](https://crates.io/crates/nokhwa).
//!
//! ## What works
//!
//! - Device enumeration via libusb / `rusb` — finds every UVC webcam
//!   attached to the system.
//! - Format discovery — walks the `VideoStreaming` interface
//!   descriptors and exposes a `Vec<CameraFormat>` covering the
//!   `(resolution, fps, format)` combinations the device advertises.
//!   Covers `MJPEG`, `YUYV` (YUY2 GUID), and `NV12` uncompressed
//!   formats.
//!
//! ## What does not work yet
//!
//! - **Streaming** — opening a stream and reading frames. Blocked on
//!   platform-specific libusb limitations (Windows) and on session 2b
//!   work (Linux/macOS). See the top-level `TODO.md` for the
//!   session-by-session roadmap.
//!
//! ## Platform notes
//!
//! Windows binds UVC webcams to `usbvideo.sys`, so `rusb` cannot claim
//! the `VideoStreaming` interface even with admin rights. Use the
//! `input-msmf` backend for streaming on Windows; the UVC backend is
//! still useful there for device enumeration and format inspection.

mod descriptors;

#[cfg(all(
    any(target_os = "linux", target_os = "macos", target_os = "windows"),
    not(feature = "docs-only")
))]
mod internal {
    use crate::descriptors::parse_video_streaming_formats;
    use nokhwa_core::{
        buffer::Buffer,
        error::NokhwaError,
        traits::{CameraDevice, FrameSource},
        types::{
            ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueSetter,
            FrameFormat, KnownCameraControl, RequestedFormat, RequestedFormatType,
        },
    };
    use rusb::{DeviceHandle, GlobalContext, UsbContext};
    use std::{borrow::Cow, time::Duration};

    /// USB Interface Class code for video devices (per USB-IF class codes).
    /// See <https://www.usb.org/defined-class-codes>.
    const USB_CLASS_VIDEO: u8 = 0x0E;
    /// Interface Subclass 0x01 = `VideoControl` — every UVC device exposes at
    /// least one `VideoControl` interface, so matching on it is the canonical
    /// way to discriminate UVC from other USB devices.
    const USB_SUBCLASS_VIDEOCONTROL: u8 = 0x01;
    /// Interface Subclass 0x02 = `VideoStreaming` — carries the
    /// `VS_FORMAT_*` / `VS_FRAME_*` descriptor chain we parse for
    /// `compatible_formats()`.
    const USB_SUBCLASS_VIDEOSTREAMING: u8 = 0x02;
    /// libusb probe timeout for string descriptors. Short-but-nonzero: a
    /// stale descriptor read should not stall enumeration.
    const STRING_DESCRIPTOR_TIMEOUT: Duration = Duration::from_millis(100);

    /// Error returned by streaming methods (`open`, `frame`, …) so the
    /// caller gets a platform-aware explanation rather than a bare
    /// `UnsupportedOperationError`. On Windows the UVC backend is
    /// *structurally* unable to stream; on other targets streaming is
    /// just not in session 2a.
    fn streaming_unsupported() -> NokhwaError {
        NokhwaError::NotImplementedError(
            if cfg!(target_os = "windows") {
                "UVC streaming via libusb is not supported on Windows: \
                 usbvideo.sys owns the interface and rusb's claim_interface \
                 returns NotSupported. Use the `input-msmf` backend for \
                 streaming; the UVC backend on Windows only exposes query() \
                 and format discovery."
            } else {
                "UVC streaming is not yet implemented (tracked as session 2b \
                 in TODO.md). Enumeration and format discovery work; \
                 open()/frame() do not."
            }
            .to_string(),
        )
    }

    /// Record of one UVC device from a single enumeration pass. Shared
    /// by `query()` and `UVCCaptureDevice::new()` so both see the same
    /// device ordering — `CameraIndex::Index(n)` round-trips deterministically.
    struct UvcEntry {
        device: rusb::Device<GlobalContext>,
        vid: u16,
        pid: u16,
        bus: u8,
        addr: u8,
        product: Option<String>,
        manufacturer: Option<String>,
    }

    impl UvcEntry {
        fn human_name(&self) -> String {
            self.product
                .clone()
                .unwrap_or_else(|| format!("UVC {:04x}:{:04x}", self.vid, self.pid))
        }
        fn description(&self) -> String {
            self.manufacturer.clone().unwrap_or_default()
        }
        fn misc(&self) -> String {
            format!(
                "{}:{} {:04x}:{:04x}",
                self.bus, self.addr, self.vid, self.pid
            )
        }
    }

    fn enumerate_uvc() -> Result<Vec<UvcEntry>, NokhwaError> {
        let context = GlobalContext::default();
        let devices = context.devices().map_err(|why| {
            NokhwaError::general(format!("libusb device enumeration failed: {why}"))
        })?;

        let mut out = Vec::new();
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
            let (product, manufacturer) = read_product_manufacturer(&device, &descriptor);
            out.push(UvcEntry {
                device,
                vid,
                pid,
                bus,
                addr,
                product,
                manufacturer,
            });
        }
        Ok(out)
    }

    /// Opens a transient handle on the device just long enough to read
    /// its iProduct / iManufacturer strings. Any failure is non-fatal —
    /// we fall back to the vid:pid placeholder name elsewhere.
    fn read_product_manufacturer<T: UsbContext>(
        device: &rusb::Device<T>,
        descriptor: &rusb::DeviceDescriptor,
    ) -> (Option<String>, Option<String>) {
        match device.open() {
            Ok(handle) => (
                read_product(&handle, descriptor),
                handle
                    .read_manufacturer_string_ascii(descriptor)
                    .ok()
                    .filter(|s| !s.is_empty()),
            ),
            Err(_) => (None, None),
        }
    }

    fn read_product<T: UsbContext>(
        handle: &DeviceHandle<T>,
        descriptor: &rusb::DeviceDescriptor,
    ) -> Option<String> {
        handle
            .read_product_string_ascii(descriptor)
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                handle
                    .read_languages(STRING_DESCRIPTOR_TIMEOUT)
                    .ok()
                    .and_then(|langs| langs.first().copied())
                    .and_then(|lang| {
                        handle
                            .read_product_string(lang, descriptor, STRING_DESCRIPTOR_TIMEOUT)
                            .ok()
                    })
            })
    }

    /// Enumerate all UVC-class USB devices visible to libusb.
    ///
    /// Returns a [`CameraInfo`] per matching device, with:
    /// - `human_name` populated from the USB iProduct string when
    ///   available, else `"UVC <vid>:<pid>"`.
    /// - `description` populated from the USB iManufacturer string.
    /// - `misc` carrying `"<bus>:<address> <vid>:<pid>"` which
    ///   `UVCCaptureDevice::new(CameraIndex::String(...))` accepts as
    ///   a re-open key.
    /// - `index` as a monotonic `CameraIndex::Index(n)` in enumeration
    ///   order so cross-backend code using `CameraIndex::Index(0)` still
    ///   targets the first device.
    pub fn query() -> Result<Vec<CameraInfo>, NokhwaError> {
        let entries = enumerate_uvc()?;
        let mut out = Vec::with_capacity(entries.len());
        for (i, entry) in entries.iter().enumerate() {
            out.push(CameraInfo::new(
                &entry.human_name(),
                &entry.description(),
                &entry.misc(),
                CameraIndex::Index(u32::try_from(i).unwrap_or(u32::MAX)),
            ));
        }
        Ok(out)
    }

    /// Walks the device's configuration descriptors looking for an
    /// interface descriptor with class = VIDEO and subclass = VIDEOCONTROL.
    /// Devices that expose `VideoStreaming` without `VideoControl` are not
    /// well-formed UVC, so we intentionally skip them.
    fn device_is_uvc<T: UsbContext>(
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

    /// Find the `VideoStreaming` interface (alt setting 0 carries the
    /// class-specific descriptor chain) and return its interface number
    /// plus the parsed `Vec<CameraFormat>`. If no VS interface is
    /// present the returned format list is empty and the interface
    /// number is zero — callers should treat empty as "unsupported
    /// device shape".
    fn read_streaming_formats<T: UsbContext>(
        device: &rusb::Device<T>,
        descriptor: &rusb::DeviceDescriptor,
    ) -> (u8, Vec<CameraFormat>) {
        for cfg_idx in 0..descriptor.num_configurations() {
            let Ok(config) = device.config_descriptor(cfg_idx) else {
                continue;
            };
            for interface in config.interfaces() {
                for alt in interface.descriptors() {
                    if alt.class_code() == USB_CLASS_VIDEO
                        && alt.sub_class_code() == USB_SUBCLASS_VIDEOSTREAMING
                        && alt.setting_number() == 0
                    {
                        let formats = parse_video_streaming_formats(alt.extra());
                        return (alt.interface_number(), formats);
                    }
                }
            }
        }
        (0, Vec::new())
    }

    /// Cross-platform UVC capture device. Session 2a surface:
    ///
    /// - `new()` opens the libusb device, parses the `VideoStreaming`
    ///   descriptor chain, and picks an initial format.
    /// - `compatible_formats()`, `compatible_fourcc()`,
    ///   `negotiated_format()`, `set_format()` work end-to-end.
    /// - `open()`, `frame()`, `frame_raw()`, and the control surface
    ///   all error with a platform-aware diagnostic (see
    ///   [`streaming_unsupported`]).
    pub struct UVCCaptureDevice {
        info: CameraInfo,
        /// Held open for the lifetime of the device so the kernel does
        /// not rip the handle away between `new()` and a future
        /// session-2b `open()`. Unused fields are name-prefixed with
        /// `_` to suppress clippy's `dead_code` while the streaming
        /// surface is still being built out.
        _handle: DeviceHandle<GlobalContext>,
        /// `VideoStreaming` interface number. Session 2b will
        /// `claim_interface` on this to start isochronous transfers.
        _iface: u8,
        formats: Vec<CameraFormat>,
        current_format: CameraFormat,
    }

    impl UVCCaptureDevice {
        #[allow(clippy::needless_pass_by_value)]
        pub fn new(index: &CameraIndex, cam_fmt: RequestedFormat) -> Result<Self, NokhwaError> {
            let entries = enumerate_uvc()?;
            let entry = match index {
                CameraIndex::Index(n) => {
                    entries
                        .get(*n as usize)
                        .ok_or_else(|| NokhwaError::OpenDeviceError {
                            device: format!("UVC index {n}"),
                            error: format!("no UVC device at index {n} (found {})", entries.len()),
                        })?
                }
                CameraIndex::String(s) => {
                    entries.iter().find(|e| e.misc() == *s).ok_or_else(|| {
                        NokhwaError::OpenDeviceError {
                            device: s.clone(),
                            error: "no UVC device matches the provided `bus:addr vid:pid` key"
                                .to_string(),
                        }
                    })?
                }
            };

            let descriptor =
                entry
                    .device
                    .device_descriptor()
                    .map_err(|why| NokhwaError::OpenDeviceError {
                        device: entry.misc(),
                        error: format!("device_descriptor: {why}"),
                    })?;
            let handle = entry
                .device
                .open()
                .map_err(|why| NokhwaError::OpenDeviceError {
                    device: entry.misc(),
                    error: format!("libusb open: {why}"),
                })?;
            let (iface, formats) = read_streaming_formats(&entry.device, &descriptor);
            if formats.is_empty() {
                return Err(NokhwaError::OpenDeviceError {
                    device: entry.misc(),
                    error: "no VS_FORMAT_MJPEG / VS_FORMAT_UNCOMPRESSED descriptors found"
                        .to_string(),
                });
            }
            let current_format = pick_format(&formats, &cam_fmt)?;
            let info = CameraInfo::new(
                &entry.human_name(),
                &entry.description(),
                &entry.misc(),
                index.clone(),
            );
            Ok(Self {
                info,
                _handle: handle,
                _iface: iface,
                formats,
                current_format,
            })
        }
    }

    /// Pick a concrete [`CameraFormat`] from the device's advertised
    /// list according to the caller's [`RequestedFormat`]. Session 2a
    /// only implements the `Exact` branch precisely; every other
    /// variant falls back to "biggest pixel count, prefer MJPEG, then
    /// highest fps" because that matches the default `OpenRequest::any()`
    /// behaviour of the native backends.
    fn pick_format(
        available: &[CameraFormat],
        requested: &RequestedFormat,
    ) -> Result<CameraFormat, NokhwaError> {
        match requested.requested_format_type() {
            RequestedFormatType::Exact(target) => available
                .iter()
                .copied()
                .find(|g| *g == target)
                .ok_or_else(|| NokhwaError::GetPropertyError {
                    property: "CameraFormat".to_string(),
                    error: format!(
                        "requested {target:?} not in {} available formats",
                        available.len()
                    ),
                }),
            _ => available
                .iter()
                .copied()
                .max_by(|a, b| {
                    let area_a = u64::from(a.width()) * u64::from(a.height());
                    let area_b = u64::from(b.width()) * u64::from(b.height());
                    area_a
                        .cmp(&area_b)
                        .then_with(|| {
                            (a.format() == FrameFormat::MJPEG)
                                .cmp(&(b.format() == FrameFormat::MJPEG))
                        })
                        .then_with(|| a.frame_rate().cmp(&b.frame_rate()))
                })
                .ok_or_else(|| NokhwaError::general("no formats available")),
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
            self.current_format
        }

        fn set_format(&mut self, f: CameraFormat) -> Result<(), NokhwaError> {
            if self.formats.contains(&f) {
                self.current_format = f;
                Ok(())
            } else {
                Err(NokhwaError::SetPropertyError {
                    property: "CameraFormat".to_string(),
                    value: format!("{f:?}"),
                    error: "not advertised by the device (see compatible_formats())".to_string(),
                })
            }
        }

        fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
            Ok(self.formats.clone())
        }

        fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
            let mut ff: Vec<FrameFormat> = Vec::new();
            for f in &self.formats {
                if !ff.contains(&f.format()) {
                    ff.push(f.format());
                }
            }
            Ok(ff)
        }

        fn open(&mut self) -> Result<(), NokhwaError> {
            Err(streaming_unsupported())
        }

        fn is_open(&self) -> bool {
            false
        }

        fn frame(&mut self) -> Result<Buffer, NokhwaError> {
            Err(streaming_unsupported())
        }

        fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
            Err(streaming_unsupported())
        }

        fn close(&mut self) -> Result<(), NokhwaError> {
            // No stream was ever started in session 2a, so close is
            // a no-op rather than an error; callers that wrap
            // open/close pairs can still call this cleanly.
            Ok(())
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
