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

//! # nokhwa-bindings-gstreamer
//!
//! Cross-platform `GStreamer` bindings for
//! [`nokhwa`](https://crates.io/crates/nokhwa). Device enumeration only in
//! this release — streaming, format negotiation, and control surface land
//! in subsequent sessions. See the root project `TODO.md` for the roadmap.
//!
//! This crate is consumed through `nokhwa` with feature `input-gstreamer`.
//! Do not depend on it directly.

#[cfg(all(feature = "backend", not(feature = "docs-only")))]
mod internal {
    use gstreamer::prelude::*;
    use gstreamer::{Caps, DeviceMonitor};
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

    /// Enumerate video sources visible to the `GStreamer` device registry.
    ///
    /// Initialises `GStreamer` on first call (idempotent), creates a
    /// [`DeviceMonitor`] filtered to `Video/Source` with `video/x-raw`
    /// caps, starts the monitor, snapshots the device list, and stops
    /// the monitor. Each [`Device`](gstreamer::Device) becomes a
    /// [`CameraInfo`] with:
    /// - `human_name` from `device.display_name()`.
    /// - `description` from `device.device_class()` (e.g. `"Video/Source"`).
    /// - `misc` left empty — session-2 code will populate it with the
    ///   element factory + properties needed to reconstruct a playable
    ///   pipeline.
    /// - `index` as a monotonic `CameraIndex::Index(n)`.
    pub fn query() -> Result<Vec<CameraInfo>, NokhwaError> {
        gstreamer::init()
            .map_err(|e| NokhwaError::general(format!("gstreamer init failed: {e}")))?;

        let monitor = DeviceMonitor::new();
        let caps = Caps::builder("video/x-raw").build();
        // Returning None from add_filter means the filter slot could not
        // be installed. A zero-filter monitor would surface every device
        // on the host, including audio sources — treat it as a fatal
        // enumeration error rather than silently widening the query.
        if monitor
            .add_filter(Some("Video/Source"), Some(&caps))
            .is_none()
        {
            return Err(NokhwaError::StructureError {
                structure: "DeviceMonitor filter Video/Source".to_string(),
                error: "add_filter returned None".to_string(),
            });
        }

        monitor
            .start()
            .map_err(|e| NokhwaError::general(format!("DeviceMonitor::start failed: {e}")))?;

        let devices = monitor.devices();
        // Stop the monitor before returning — leaked monitors hold
        // references to GStreamer plugins that subsequent calls expect
        // to be free.
        monitor.stop();

        let mut cameras = Vec::with_capacity(devices.len());
        for (idx, dev) in devices.into_iter().enumerate() {
            let name = dev.display_name().to_string();
            let class = dev.device_class().to_string();
            cameras.push(CameraInfo::new(
                &name,
                &class,
                "",
                CameraIndex::Index(u32::try_from(idx).unwrap_or(u32::MAX)),
            ));
        }
        Ok(cameras)
    }

    /// Cross-platform `GStreamer` capture device. **Stream support is
    /// unimplemented in this release.** `query()` is fully functional;
    /// `new()` and every `FrameSource` / `CameraDevice` method currently
    /// returns [`NokhwaError::NotImplementedError`] so the backend can
    /// already be compiled, feature-gated, and registered with
    /// `nokhwa_backend!` while the streaming surface is iterated in
    /// follow-up work.
    ///
    /// Track progress against the `GStreamer` backlog item in the project
    /// `TODO.md`.
    pub struct GStreamerCaptureDevice {
        info: CameraInfo,
    }

    impl GStreamerCaptureDevice {
        pub fn new(_index: &CameraIndex, _cam_fmt: RequestedFormat) -> Result<Self, NokhwaError> {
            Err(NokhwaError::NotImplementedError(
                "GStreamer streaming not yet implemented — query() is available; \
                 see TODO.md for the session roadmap"
                    .to_string(),
            ))
        }
    }

    impl CameraDevice for GStreamerCaptureDevice {
        fn backend(&self) -> ApiBackend {
            ApiBackend::GStreamer
        }

        fn info(&self) -> &CameraInfo {
            &self.info
        }

        fn controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }

        fn set_control(
            &mut self,
            _id: KnownCameraControl,
            _value: ControlValueSetter,
        ) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }
    }

    impl FrameSource for GStreamerCaptureDevice {
        fn negotiated_format(&self) -> CameraFormat {
            CameraFormat::default()
        }

        fn set_format(&mut self, _f: CameraFormat) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }

        fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }

        fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }

        fn open(&mut self) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }

        fn is_open(&self) -> bool {
            false
        }

        fn frame(&mut self) -> Result<Buffer, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }

        fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }

        fn close(&mut self) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }
    }
}

#[cfg(any(not(feature = "backend"), feature = "docs-only"))]
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

    /// Stub [`query`] for builds without the `backend` feature. The
    /// real implementation requires `gstreamer-rs` (and a system
    /// `GStreamer` install); consumers enable it via the top-level
    /// `input-gstreamer` feature on the `nokhwa` crate.
    pub fn query() -> Result<Vec<CameraInfo>, NokhwaError> {
        Err(NokhwaError::NotImplementedError(
            "GStreamer backend not compiled in (enable feature `input-gstreamer` on the `nokhwa` \
             crate)"
                .to_string(),
        ))
    }

    /// Stub [`GStreamerCaptureDevice`] for builds without the `backend`
    /// feature. Every method errors with
    /// [`NokhwaError::NotImplementedError`].
    pub struct GStreamerCaptureDevice;

    #[allow(unused_variables)]
    impl GStreamerCaptureDevice {
        pub fn new(index: &CameraIndex, cam_fmt: RequestedFormat) -> Result<Self, NokhwaError> {
            Err(NokhwaError::NotImplementedError(
                "GStreamer backend not compiled in".to_string(),
            ))
        }
    }

    #[allow(unused_variables)]
    impl CameraDevice for GStreamerCaptureDevice {
        fn backend(&self) -> ApiBackend {
            ApiBackend::GStreamer
        }

        fn info(&self) -> &CameraInfo {
            unreachable!("GStreamer stub: GStreamerCaptureDevice::new always fails")
        }

        fn controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }

        fn set_control(
            &mut self,
            id: KnownCameraControl,
            value: ControlValueSetter,
        ) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }
    }

    #[allow(unused_variables)]
    impl FrameSource for GStreamerCaptureDevice {
        fn negotiated_format(&self) -> CameraFormat {
            CameraFormat::default()
        }

        fn set_format(&mut self, f: CameraFormat) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }

        fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }

        fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }

        fn open(&mut self) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }

        fn is_open(&self) -> bool {
            false
        }

        fn frame(&mut self) -> Result<Buffer, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }

        fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }

        fn close(&mut self) -> Result<(), NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::GStreamer,
            ))
        }
    }
}

pub use internal::*;

#[cfg(all(test, not(feature = "backend")))]
mod stub_tests {
    use super::internal::query;

    /// Verifies the stub path compiles and returns a `NotImplementedError`
    /// without panicking.
    #[test]
    fn stub_query_errors_cleanly() {
        let err = query().expect_err("stub query() must error");
        let msg = format!("{err}");
        assert!(
            msg.contains("GStreamer"),
            "unexpected stub error text: {msg}"
        );
    }
}

#[cfg(all(test, feature = "backend", not(feature = "docs-only")))]
mod backend_tests {
    use super::internal::query;

    /// Smoke test with the real GStreamer backend. Must not panic.
    /// Accepts both `Ok(vec)` and `Err(_)` because CI runners may
    /// have GStreamer installed with zero video-source plugins
    /// registered, in which case the monitor returns an empty list,
    /// while a sandboxed runner without `/dev/video*` access may
    /// surface `gstreamer::init()` errors.
    #[test]
    fn query_does_not_panic() {
        match query() {
            Ok(cameras) => {
                eprintln!("gstreamer::query() -> {} source(s)", cameras.len());
                for cam in &cameras {
                    eprintln!("  {} | {}", cam.human_name(), cam.description());
                }
            }
            Err(e) => eprintln!("gstreamer::query() errored (accepted): {e}"),
        }
    }
}
