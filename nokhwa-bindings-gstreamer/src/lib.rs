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
mod controls;
#[cfg(all(feature = "backend", not(feature = "docs-only")))]
mod format;
#[cfg(all(feature = "backend", not(feature = "docs-only")))]
mod pipeline;

#[cfg(all(feature = "backend", not(feature = "docs-only")))]
mod internal {
    use crate::controls::{
        build_extra_controls, control_handle, list_controls, set_live_property, unsupported,
        v4l2_cid_value, GstControlHandle,
    };
    use crate::pipeline::{
        compatible_formats as caps_for_device, compatible_fourcc as fourcc_for_device, find_device,
        resolve_format, PipelineHandle,
    };
    use gstreamer::prelude::*;
    use gstreamer::{Caps, Device, DeviceMonitor};
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
    use std::collections::BTreeMap;

    /// Enumerate video sources visible to the `GStreamer` device registry.
    ///
    /// Initialises `GStreamer` on first call (idempotent), creates a
    /// [`DeviceMonitor`] filtered to `Video/Source` with `video/x-raw`
    /// caps, starts the monitor, snapshots the device list, and stops
    /// the monitor. Each [`Device`](gstreamer::Device) becomes a
    /// [`CameraInfo`] with:
    /// - `human_name` from `device.display_name()`.
    /// - `description` from `device.device_class()` (e.g. `"Video/Source"`).
    /// - `misc` holds the display name as a stable re-identification
    ///   key for `GStreamerCaptureDevice::new()` to rediscover the
    ///   underlying [`Device`] across successive `DeviceMonitor`
    ///   invocations. Two cameras that share a display name fall back
    ///   to positional index.
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
                &name,
                CameraIndex::Index(u32::try_from(idx).unwrap_or(u32::MAX)),
            ));
        }
        Ok(cameras)
    }

    /// Cross-platform `GStreamer` capture device.
    ///
    /// Session 2 (this release) implements streaming via a
    /// `source ! capsfilter ! videoconvert ! appsink` pipeline. The
    /// source element is the one `Device::create_element()` hands us —
    /// `v4l2src` on Linux, `mfvideosrc` on Windows, `avfvideosrc` on
    /// macOS — so format enumeration and actual negotiation happen
    /// against the real device caps rather than a hardcoded element
    /// name.
    ///
    /// Controls (session 3) are Linux-only: `v4l2src` exposes four
    /// `controllable` GObject properties (brightness / contrast / hue
    /// / saturation) that work at any pipeline state, plus the
    /// write-only `extra-controls` structure for the rest of the V4L2
    /// CID namespace (exposure / zoom / focus / pan / tilt etc).
    /// Windows `mfvideosrc` / `ksvideosrc` and macOS `avfvideosrc`
    /// expose no camera-control properties — on those platforms
    /// `controls()` returns an empty list and `set_control()` errors.
    /// Users who need full control support on Windows / macOS should
    /// use the native `input-msmf` / `input-avfoundation` backends.
    ///
    /// Not yet implemented: `nokhwa::open()` dispatch integration
    /// (session 4).
    pub struct GStreamerCaptureDevice {
        info: CameraInfo,
        device: Device,
        formats: Vec<CameraFormat>,
        negotiated: CameraFormat,
        pipeline: Option<PipelineHandle>,
        /// V4L2 CIDs to apply via `extra-controls` on the next
        /// pipeline open. Keyed by the CID name (e.g. `"zoom_absolute"`).
        /// Accumulates across `set_control` calls until the next
        /// `open()` flushes it into the source element.
        pending_extra_controls: BTreeMap<String, i64>,
    }

    impl GStreamerCaptureDevice {
        pub fn new(index: &CameraIndex, cam_fmt: RequestedFormat) -> Result<Self, NokhwaError> {
            gstreamer::init()
                .map_err(|e| NokhwaError::general(format!("gstreamer init failed: {e}")))?;

            let (display_name, positional) = match index {
                CameraIndex::Index(i) => (String::new(), *i),
                CameraIndex::String(s) => (s.clone(), 0),
            };
            let device = find_device(&display_name, positional)?;

            let formats = caps_for_device(&device);
            let negotiated = resolve_format(&formats, &cam_fmt)?;

            let name = device.display_name().to_string();
            let class = device.device_class().to_string();
            let info = CameraInfo::new(&name, &class, &name, index.clone());

            Ok(Self {
                info,
                device,
                formats,
                negotiated,
                pipeline: None,
                pending_extra_controls: BTreeMap::new(),
            })
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
            // Without an open pipeline we have no source element to
            // introspect. Errors rather than returning `Ok(vec![])` so
            // the distinction between "no controls at all" and "ask
            // me again after `open()`" stays visible.
            let Some(pipeline) = &self.pipeline else {
                return Err(NokhwaError::ReadFrameError {
                    message: "GStreamer controls() requires an open pipeline; call open() first"
                        .to_string(),
                    format: None,
                });
            };
            Ok(list_controls(pipeline.source()))
        }

        fn set_control(
            &mut self,
            id: KnownCameraControl,
            value: ControlValueSetter,
        ) -> Result<(), NokhwaError> {
            let handle = control_handle(id).ok_or_else(|| NokhwaError::SetPropertyError {
                property: id.to_string(),
                value: value.to_string(),
                error: "KnownCameraControl::Other is not mapped by the GStreamer backend"
                    .to_string(),
            })?;
            match handle {
                GstControlHandle::Property(name) => {
                    // Live path — the four `controllable` v4l2src
                    // properties can be set at any pipeline state, but
                    // we still need the source element to exist.
                    let Some(pipeline) = &self.pipeline else {
                        return Err(NokhwaError::SetPropertyError {
                            property: name.to_string(),
                            value: value.to_string(),
                            error: "pipeline not open; open() before set_control for live controls"
                                .to_string(),
                        });
                    };
                    set_live_property(pipeline.source(), name, &value)
                }
                GstControlHandle::V4l2Cid(cid) => {
                    // Stage it in `pending_extra_controls` — it takes
                    // effect on the next `open()` via v4l2src's
                    // `extra-controls` property. If the pipeline is
                    // already open we tear it down and restart so the
                    // change lands immediately; matches what the MSMF
                    // backend does for non-live property writes.
                    let int_value = v4l2_cid_value(cid, &value)?;
                    self.pending_extra_controls
                        .insert(cid.to_string(), int_value);
                    if self.pipeline.is_some() {
                        self.pipeline = None;
                        self.pipeline = Some(PipelineHandle::start(
                            &self.device,
                            self.negotiated,
                            build_extra_controls(&self.pending_extra_controls),
                        )?);
                    }
                    Ok(())
                }
            }
        }
    }

    // `unsupported` is a sentinel used by sibling modules on non-Linux
    // paths. Keep the re-export explicit so clippy's `dead_code` pass
    // doesn't trip on the import.
    #[allow(dead_code)]
    fn _touch_unsupported() -> NokhwaError {
        unsupported()
    }

    impl FrameSource for GStreamerCaptureDevice {
        fn negotiated_format(&self) -> CameraFormat {
            self.negotiated
        }

        fn set_format(&mut self, f: CameraFormat) -> Result<(), NokhwaError> {
            if !self.formats.contains(&f) {
                return Err(NokhwaError::SetPropertyError {
                    property: "CameraFormat".to_string(),
                    value: format!("{f:?}"),
                    error: "not in the device's compatible format list".to_string(),
                });
            }
            // Tear down the live pipeline before swapping formats;
            // rebuilding with the new caps is the cleanest way to
            // flush any in-flight buffers negotiated against the old
            // format.
            let was_open = self.pipeline.is_some();
            self.pipeline = None;
            self.negotiated = f;
            if was_open {
                self.pipeline = Some(PipelineHandle::start(
                    &self.device,
                    self.negotiated,
                    build_extra_controls(&self.pending_extra_controls),
                )?);
            }
            Ok(())
        }

        fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
            Ok(self.formats.clone())
        }

        fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
            Ok(fourcc_for_device(&self.formats))
        }

        fn open(&mut self) -> Result<(), NokhwaError> {
            if self.pipeline.is_none() {
                self.pipeline = Some(PipelineHandle::start(
                    &self.device,
                    self.negotiated,
                    build_extra_controls(&self.pending_extra_controls),
                )?);
            }
            Ok(())
        }

        fn is_open(&self) -> bool {
            self.pipeline.is_some()
        }

        fn frame(&mut self) -> Result<Buffer, NokhwaError> {
            match &self.pipeline {
                Some(p) => p.pull_frame(),
                None => Err(NokhwaError::ReadFrameError {
                    message: "pipeline not open — call open() first".to_string(),
                    format: Some(self.negotiated.format()),
                }),
            }
        }

        fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
            // Cow::Owned wrap of the pulled frame's bytes. A borrowed
            // variant isn't safe here because the AppSink sample's
            // memory mapping is scoped to the pull call.
            let buf = self.frame()?;
            Ok(Cow::Owned(buf.buffer().to_vec()))
        }

        fn close(&mut self) -> Result<(), NokhwaError> {
            self.pipeline = None;
            Ok(())
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
