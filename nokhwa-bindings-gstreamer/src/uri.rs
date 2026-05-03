//! URL/URI detection + pipeline construction for the GStreamer
//! backend's session-5 "open by URL" mode.
//!
//! The device-monitor path (sessions 2/3) covers local cameras; this
//! module covers `rtsp://` / `rtmp://` / `http(s)://` / `file://` and
//! similar URIs that don't show up in `DeviceMonitor`. One pipeline
//! shape covers all of them because GStreamer's [`uridecodebin`]
//! element auto-detects the right source + decoder from the scheme:
//!
//! ```text
//! uridecodebin uri=<URL> ! videoconvert ! appsink
//! ```
//!
//! We deliberately skip `capsfilter` here — unlike local cameras we
//! cannot enumerate format capabilities from a URL ahead of time, so
//! the negotiated format is whatever the first sample delivers and
//! `appsink` is configured with a soft `video/x-raw` caps expectation.
//! `videoconvert` normalises the downstream stream into a format we
//! can report.

use crate::format::video_format_to_frame_format;
use gstreamer::prelude::*;
use gstreamer::{Element, Pipeline, State};
use gstreamer_app::AppSink;
use gstreamer_video::VideoFormat;
use nokhwa_core::{
    buffer::Buffer,
    error::NokhwaError,
    types::{CameraFormat, FrameFormat, Resolution},
};
use std::time::Duration;

/// How long `open()` waits for the first sample to appear before
/// deciding the URL won't yield frames. 10s is generous — RTSP setup
/// over a slow network easily takes 2–3s, and file-based URIs should
/// be instant.
const FIRST_SAMPLE_TIMEOUT: Duration = Duration::from_secs(10);

/// Schemes we treat as URLs (dispatched through `uridecodebin`).
/// Anything else is assumed to be a display-name lookup against the
/// live `DeviceMonitor`.
pub(crate) fn looks_like_uri(s: &str) -> bool {
    // Cheap check — no regex. A URI scheme is `alpha (alpha/digit/+/-/.)* ":"`.
    // We only recognise the common multimedia ones to avoid treating a
    // camera whose display name happens to contain `:` as a URL.
    const SCHEMES: &[&str] = &[
        "rtsp://", "rtsps://", "rtmp://", "rtmps://", "http://", "https://", "file://", "srt://",
        "udp://", "tcp://",
    ];
    let lower = s.to_ascii_lowercase();
    SCHEMES.iter().any(|s| lower.starts_with(s))
}

/// URI-mode pipeline handle. Shape mirrors [`crate::pipeline::PipelineHandle`]
/// but the constructor and source element differ: no `Device`, no
/// `capsfilter`, format is learned from the first sample instead of
/// negotiated ahead of time.
pub(crate) struct UriPipelineHandle {
    pipeline: Pipeline,
    appsink: AppSink,
    source: Element,
    format: CameraFormat,
}

impl UriPipelineHandle {
    pub(crate) fn source(&self) -> &Element {
        &self.source
    }

    pub(crate) fn start(uri: &str) -> Result<Self, NokhwaError> {
        let source = gstreamer::ElementFactory::make("uridecodebin")
            .property("uri", uri)
            .build()
            .map_err(|e| NokhwaError::OpenDeviceError {
                device: uri.to_string(),
                error: format!("uridecodebin factory: {e}"),
            })?;

        let convert = gstreamer::ElementFactory::make("videoconvert")
            .build()
            .map_err(|e| NokhwaError::OpenDeviceError {
                device: uri.to_string(),
                error: format!("videoconvert factory: {e}"),
            })?;

        // Soft caps — let whatever the decoder produces through after
        // videoconvert, filtered to formats we know how to expose.
        let caps = gstreamer::Caps::builder("video/x-raw")
            .field("format", gstreamer::List::new([&"YUY2", &"NV12", &"GRAY8"]))
            .build();
        let appsink = AppSink::builder()
            .caps(&caps)
            .max_buffers(1)
            .drop(true)
            .build();
        let sink_element: Element = appsink.clone().upcast();
        sink_element.set_property("sync", false);

        let pipeline = Pipeline::new();
        pipeline
            .add(&source)
            .map_err(|e| NokhwaError::OpenStreamError {
                message: format!("Pipeline::add(source): {e}"),
                backend: Some(nokhwa_core::types::ApiBackend::GStreamer),
            })?;
        pipeline
            .add(&convert)
            .map_err(|e| NokhwaError::OpenStreamError {
                message: format!("Pipeline::add(convert): {e}"),
                backend: Some(nokhwa_core::types::ApiBackend::GStreamer),
            })?;
        pipeline
            .add(&sink_element)
            .map_err(|e| NokhwaError::OpenStreamError {
                message: format!("Pipeline::add(appsink): {e}"),
                backend: Some(nokhwa_core::types::ApiBackend::GStreamer),
            })?;

        // uridecodebin has dynamic pads — the src pad appears only
        // after the source is probed and the decoder chain is built.
        // Link the static `convert` <-> `appsink` part immediately,
        // but `source -> convert` has to happen from a pad-added
        // signal callback.
        convert
            .link(&sink_element)
            .map_err(|e| NokhwaError::OpenStreamError {
                message: format!("link convert->appsink: {e}"),
                backend: Some(nokhwa_core::types::ApiBackend::GStreamer),
            })?;
        let convert_weak = convert.downgrade();
        source.connect_pad_added(move |_src, new_pad| {
            let Some(convert) = convert_weak.upgrade() else {
                return;
            };
            let Some(sink_pad) = convert.static_pad("sink") else {
                return;
            };
            if sink_pad.is_linked() {
                return;
            }
            // Only link video pads — uridecodebin also produces audio
            // pads for streams that carry audio, which we don't want.
            let is_video = new_pad
                .current_caps()
                .and_then(|caps| {
                    caps.structure(0).map(|s| {
                        let name = s.name();
                        name.starts_with("video/")
                    })
                })
                .unwrap_or(false);
            if !is_video {
                return;
            }
            let _ = new_pad.link(&sink_pad);
        });

        // Kick the pipeline into Playing. `uridecodebin` negotiates
        // asynchronously; we block on the state change and then the
        // first sample.
        let state_change =
            pipeline
                .set_state(State::Playing)
                .map_err(|e| NokhwaError::OpenStreamError {
                    message: format!("set_state(Playing): {e}"),
                    backend: Some(nokhwa_core::types::ApiBackend::GStreamer),
                })?;
        if state_change == gstreamer::StateChangeSuccess::Async {
            let (res, _, _) = pipeline.state(gstreamer::ClockTime::from_seconds(10));
            res.map_err(|e| NokhwaError::OpenStreamError {
                message: format!("async state wait: {e}"),
                backend: Some(nokhwa_core::types::ApiBackend::GStreamer),
            })?;
        }

        // Learn the actual format from the first sample. We can't
        // enumerate `compatible_formats()` from a URL (no probe API
        // short of fully connecting), so the first sample is
        // authoritative.
        let first_sample = appsink
            .try_pull_sample(gstreamer::ClockTime::from_nseconds(
                u64::try_from(FIRST_SAMPLE_TIMEOUT.as_nanos()).unwrap_or(u64::MAX),
            ))
            .ok_or_else(|| NokhwaError::OpenStreamError {
                message: "timed out waiting for the first sample from the URI — \
                          the stream may be unreachable or produce only audio"
                    .to_string(),
                backend: Some(nokhwa_core::types::ApiBackend::GStreamer),
            })?;
        let format = sample_format(&first_sample)?;

        Ok(Self {
            pipeline,
            appsink,
            source,
            format,
        })
    }

    pub(crate) fn pull_frame(&self) -> Result<Buffer, NokhwaError> {
        let sample = self
            .appsink
            .try_pull_sample(gstreamer::ClockTime::from_seconds(1))
            .ok_or_else(|| NokhwaError::ReadFrameError {
                message: "AppSink::try_pull_sample timed out or hit EOS".to_string(),
                format: Some(self.format.format()),
            })?;
        let buffer = sample.buffer().ok_or_else(|| NokhwaError::ReadFrameError {
            message: "Sample carried no GstBuffer".to_string(),
            format: Some(self.format.format()),
        })?;
        let map = buffer
            .map_readable()
            .map_err(|e| NokhwaError::ReadFrameError {
                message: format!("map_readable: {e}"),
                format: Some(self.format.format()),
            })?;
        Ok(Buffer::new(
            self.format.resolution(),
            map.as_slice(),
            self.format.format(),
        ))
    }

    pub(crate) fn format(&self) -> CameraFormat {
        self.format
    }
}

impl Drop for UriPipelineHandle {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(State::Null);
    }
}

/// Extract resolution + FrameFormat from a sample's caps. URL streams
/// might not advertise a stable framerate, so we default to 30fps and
/// document the limitation.
#[allow(clippy::cast_possible_truncation)]
fn sample_format(sample: &gstreamer::Sample) -> Result<CameraFormat, NokhwaError> {
    let caps = sample.caps().ok_or_else(|| NokhwaError::StructureError {
        structure: "sample caps".to_string(),
        error: "first sample had no caps".to_string(),
    })?;
    let structure = caps
        .structure(0)
        .ok_or_else(|| NokhwaError::StructureError {
            structure: "sample caps structure".to_string(),
            error: "caps has no structure".to_string(),
        })?;
    let format_name = structure
        .get::<&str>("format")
        .map_err(|e| NokhwaError::StructureError {
            structure: "format".to_string(),
            error: e.to_string(),
        })?;
    let frame_format = video_format_to_frame_format(VideoFormat::from_string(format_name))
        .ok_or_else(|| NokhwaError::StructureError {
            structure: "format".to_string(),
            error: format!("videoconvert produced unsupported format: {format_name}"),
        })?;
    let width = structure
        .get::<i32>("width")
        .map_err(|e| NokhwaError::StructureError {
            structure: "width".to_string(),
            error: e.to_string(),
        })?;
    let height = structure
        .get::<i32>("height")
        .map_err(|e| NokhwaError::StructureError {
            structure: "height".to_string(),
            error: e.to_string(),
        })?;
    if width <= 0 || height <= 0 {
        return Err(NokhwaError::StructureError {
            structure: "resolution".to_string(),
            error: format!("invalid dimensions {width}x{height}"),
        });
    }
    // Framerate is optional for URL streams — some RTSP sources
    // advertise 0/1 or skip it entirely. 30fps is a sensible default
    // for the `CameraFormat.frame_rate` field, and frame-timing
    // consumers should rely on the capture timestamp rather than the
    // nominal rate anyway.
    let fps = structure
        .get::<gstreamer::Fraction>("framerate")
        .ok()
        .and_then(|frac| {
            let n = frac.numer();
            let d = frac.denom();
            if n <= 0 || d <= 0 {
                None
            } else if n % d == 0 {
                u32::try_from(n / d).ok()
            } else {
                None
            }
        })
        .unwrap_or(30);

    Ok(CameraFormat::new(
        Resolution::new(width as u32, height as u32),
        frame_format,
        fps,
    ))
}

/// `compatible_fourcc()` for URL-mode sources. `uridecodebin`
/// negotiates exactly one format on the appsink pad — the network
/// peer / file picks the encoding, we don't get to enumerate
/// alternatives — so the compatible-fourcc list is the singleton
/// `[fmt.format()]` (the format we already have). Returning an empty
/// list here would silently break callers that compare it against
/// `compatible_formats()` for the cross-backend invariant
/// (`compatible_fourcc ⊇ compatible_formats.iter().map(|f| f.format())`)
/// pinned in `tests/device_tests.rs`.
#[must_use]
pub(crate) fn compatible_fourcc_from_negotiated(fmt: CameraFormat) -> Vec<FrameFormat> {
    vec![fmt.format()]
}

#[cfg(test)]
mod tests {
    use super::{compatible_fourcc_from_negotiated, looks_like_uri};
    use nokhwa_core::types::{CameraFormat, FrameFormat, Resolution};

    #[test]
    fn looks_like_uri_detects_all_known_schemes() {
        for s in [
            "rtsp://example.com/stream",
            "rtsps://example.com/stream",
            "rtmp://example.com/live",
            "rtmps://example.com/live",
            "http://example.com/video.mp4",
            "https://example.com/video.mp4",
            "file:///tmp/video.mp4",
            "srt://example.com:8080",
            "udp://239.0.0.1:5004",
            "tcp://example.com:1234",
        ] {
            assert!(looks_like_uri(s), "expected URL: {s}");
        }
    }

    #[test]
    fn looks_like_uri_rejects_non_uri_inputs() {
        for s in [
            "",
            "/dev/video0",
            "video0",
            "Logitech BRIO",
            "ftp://example.com/file",
            "ws://example.com",
            "rtsp",
            "http:/foo",
        ] {
            assert!(!looks_like_uri(s), "expected non-URL: {s}");
        }
    }

    #[test]
    fn looks_like_uri_is_case_insensitive() {
        for s in [
            "RTSP://example.com",
            "Http://example.com",
            "FILE:///tmp/x",
            "RtMpS://example.com",
        ] {
            assert!(looks_like_uri(s), "expected URL (mixed case): {s}");
        }
    }

    // The session.rs `looks_like_uri_scheme` function has a doc note saying
    // "Kept in sync with the scheme list in
    // `nokhwa-bindings-gstreamer::uri::looks_like_uri`." This test pins the
    // scheme list shape (count + canonical lower-case forms) so a divergence
    // between the two implementations would be visible at the binding-crate
    // level. The mirror test in `nokhwa::session` covers the other side.
    #[test]
    fn scheme_list_shape_is_stable() {
        // Each of these MUST be detected. The set is intentionally narrow:
        // we don't want to treat a display name with `:` as a URL.
        let mirror = [
            "rtsp://", "rtsps://", "rtmp://", "rtmps://", "http://", "https://", "file://",
            "srt://", "udp://", "tcp://",
        ];
        for prefix in mirror {
            assert!(looks_like_uri(prefix), "{prefix} should be a URL prefix");
        }
        // And these intentionally are NOT in the list. If anyone adds them,
        // they have to update both implementations and this test.
        for unsupported in ["ftp://", "ws://", "wss://", "data:", "mms://"] {
            assert!(
                !looks_like_uri(unsupported),
                "{unsupported} unexpectedly recognised"
            );
        }
    }

    #[test]
    fn compatible_fourcc_from_negotiated_returns_singleton() {
        for f in [
            FrameFormat::MJPEG,
            FrameFormat::YUYV,
            FrameFormat::NV12,
            FrameFormat::GRAY,
            FrameFormat::RAWRGB,
            FrameFormat::RAWBGR,
        ] {
            let cf = CameraFormat::new(Resolution::new(640, 480), f, 30);
            let v = compatible_fourcc_from_negotiated(cf);
            assert_eq!(v, vec![f], "wrong singleton for {f:?}");
        }
    }
}
