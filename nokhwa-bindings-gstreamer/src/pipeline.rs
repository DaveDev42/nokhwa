//! GStreamer pipeline lifecycle for session-2 streaming.
//!
//! Pipeline shape:
//!
//! ```text
//! source (device.create_element) ! capsfilter ! videoconvert ! appsink
//! ```
//!
//! `capsfilter` locks the negotiated `video/x-raw` format/resolution/
//! framerate so downstream negotiation doesn't drift. `videoconvert`
//! is a safety net for sources that won't hand us exactly the format
//! we asked for (rare, but the cost is cheap). `appsink` is the
//! egress — we pull samples synchronously via
//! [`AppSink::pull_sample`].
//!
//! AppSink is configured for low-latency "grab the latest frame"
//! semantics: `max-buffers=1`, `drop=true`, `sync=false`. That matches
//! the other backends (MSMF / V4L / AVF) — nokhwa's `FrameSource`
//! contract is "give me the freshest frame," not "give me every frame
//! in order."

use crate::format::frame_format_to_video_format;
use gstreamer::prelude::*;
use gstreamer::{Caps, Device, Element, Fraction, Pipeline, State};
use gstreamer_app::AppSink;
use gstreamer_video::VideoFormat;
use nokhwa_core::{
    buffer::Buffer,
    error::NokhwaError,
    types::{CameraFormat, FrameFormat},
};
use std::time::Duration;

/// How long [`PipelineHandle::pull_frame`] blocks waiting for the next
/// sample before returning [`NokhwaError::ReadFrameError`]. Matches
/// V4L's `read_timeout` (1 second) — long enough for a slow source to
/// warm up, short enough to avoid wedging a misbehaving pipeline.
const PULL_TIMEOUT: Duration = Duration::from_secs(1);

/// Owning handle to a live GStreamer pipeline.
///
/// Drops `set_state(Null)` automatically so a forgotten `close()` call
/// doesn't leak a playing pipeline across subsequent device opens.
pub(crate) struct PipelineHandle {
    pipeline: Pipeline,
    appsink: AppSink,
    source: Element,
    format: CameraFormat,
}

impl PipelineHandle {
    /// Access the source element for control introspection + writes.
    /// On Linux this is `v4l2src`; on Windows `ksvideosrc` /
    /// `mfvideosrc`; on macOS `avfvideosrc`.
    pub(crate) fn source(&self) -> &Element {
        &self.source
    }
}

impl PipelineHandle {
    /// Build + start a pipeline for `device` negotiated to `format`.
    ///
    /// Synchronously waits for the `Playing` state change so that the
    /// very first `pull_frame` call sees a live buffer queue rather
    /// than racing a half-initialised pipeline.
    pub(crate) fn start(
        device: &Device,
        format: CameraFormat,
        extra_controls: Option<gstreamer::Structure>,
    ) -> Result<Self, NokhwaError> {
        let video_format = frame_format_to_video_format(format.format()).ok_or_else(|| {
            NokhwaError::SetPropertyError {
                property: "FrameFormat".to_string(),
                value: format!("{:?}", format.format()),
                error: "not supported by GStreamer session-2 pipeline".to_string(),
            }
        })?;

        let caps_value = caps_for(format, video_format);

        let source = device
            .create_element(None)
            .map_err(|e| NokhwaError::OpenDeviceError {
                device: device.display_name().to_string(),
                error: format!("Device::create_element failed: {e}"),
            })?;

        // Apply extra-controls before state leaves NULL — v4l2src reads
        // this property during the transition to READY and dispatches
        // the corresponding V4L2 VIDIOC_S_CTRL ioctls. Best-effort;
        // non-v4l2 source elements simply ignore the property.
        if let Some(structure) = extra_controls {
            // `find_property` keeps this safe on source elements that
            // don't know what `extra-controls` is (everything other
            // than v4l2src): skip silently instead of asserting.
            if source.find_property("extra-controls").is_some() {
                source.set_property("extra-controls", &structure);
            }
        }

        let capsfilter = gstreamer::ElementFactory::make("capsfilter")
            .property("caps", caps_value.clone())
            .build()
            .map_err(|e| element_err("capsfilter", &e.to_string()))?;

        let convert = gstreamer::ElementFactory::make("videoconvert")
            .build()
            .map_err(|e| element_err("videoconvert", &e.to_string()))?;

        let appsink = AppSink::builder()
            .caps(&caps_value)
            .max_buffers(1)
            .drop(true)
            .build();
        // `sync` is a property on `BaseSink`, the parent of `AppSink`.
        // Setting it to `false` means the sink hands frames up
        // immediately on arrival instead of waiting for the pipeline
        // clock — correct semantics for "grab latest frame."
        let sink_element: Element = appsink.clone().upcast();
        sink_element.set_property("sync", false);

        let pipeline = Pipeline::new();
        pipeline
            .add(&source)
            .map_err(|err| element_err("Pipeline::add(source)", &err.to_string()))?;
        pipeline
            .add(&capsfilter)
            .map_err(|err| element_err("Pipeline::add(capsfilter)", &err.to_string()))?;
        pipeline
            .add(&convert)
            .map_err(|err| element_err("Pipeline::add(convert)", &err.to_string()))?;
        pipeline
            .add(&sink_element)
            .map_err(|err| element_err("Pipeline::add(appsink)", &err.to_string()))?;
        source
            .link(&capsfilter)
            .map_err(|err| element_err("link source->capsfilter", &err.to_string()))?;
        capsfilter
            .link(&convert)
            .map_err(|err| element_err("link capsfilter->convert", &err.to_string()))?;
        convert
            .link(&sink_element)
            .map_err(|err| element_err("link convert->appsink", &err.to_string()))?;

        let state_change =
            pipeline
                .set_state(State::Playing)
                .map_err(|e| NokhwaError::OpenStreamError {
                    message: format!("set_state(Playing): {e}"),
                    backend: Some(nokhwa_core::types::ApiBackend::GStreamer),
                })?;
        if state_change == gstreamer::StateChangeSuccess::Async {
            let (res, _, _) = pipeline.state(gstreamer::ClockTime::from_seconds(5));
            res.map_err(|e| NokhwaError::OpenStreamError {
                message: format!("async state wait: {e}"),
                backend: Some(nokhwa_core::types::ApiBackend::GStreamer),
            })?;
        }

        Ok(Self {
            pipeline,
            appsink,
            source,
            format,
        })
    }

    /// Pull the next ready sample and copy it into a nokhwa
    /// [`Buffer`]. Blocks up to [`PULL_TIMEOUT`]; translates timeout
    /// and EOS into [`NokhwaError::ReadFrameError`].
    pub(crate) fn pull_frame(&self) -> Result<Buffer, NokhwaError> {
        let sample = self
            .appsink
            .try_pull_sample(gstreamer::ClockTime::from_nseconds(
                u64::try_from(PULL_TIMEOUT.as_nanos()).unwrap_or(u64::MAX),
            ))
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
}

impl Drop for PipelineHandle {
    fn drop(&mut self) {
        // Best-effort — if Null transition fails we've already lost
        // control of the pipeline, and bubbling the error up past Drop
        // isn't possible anyway.
        let _ = self.pipeline.set_state(State::Null);
    }
}

/// `Caps` for the negotiated format. Both the source-side capsfilter
/// and the appsink use the same caps so `videoconvert` sees matching
/// sink/src pads and is a no-op on the happy path.
fn caps_for(fmt: CameraFormat, video_format: VideoFormat) -> Caps {
    #[allow(clippy::cast_possible_wrap)]
    Caps::builder("video/x-raw")
        .field("format", video_format.to_str().as_str())
        .field("width", fmt.width() as i32)
        .field("height", fmt.height() as i32)
        .field("framerate", Fraction::new(fmt.frame_rate() as i32, 1))
        .build()
}

fn element_err(what: &str, why: &str) -> NokhwaError {
    NokhwaError::OpenStreamError {
        message: format!("{what}: {why}"),
        backend: Some(nokhwa_core::types::ApiBackend::GStreamer),
    }
}

/// Walk the live monitor a second time to find the device the caller
/// enumerated via [`crate::query`]. We pick by `display_name` because
/// the original `query` only stored that in `CameraInfo.human_name`;
/// falling back to positional index lets the common "first camera"
/// path work even when two devices share a display name.
pub(crate) fn find_device(
    display_name: &str,
    positional_index: u32,
) -> Result<Device, NokhwaError> {
    use gstreamer::DeviceMonitor;

    gstreamer::init().map_err(|e| NokhwaError::general(format!("gstreamer init failed: {e}")))?;

    let monitor = DeviceMonitor::new();
    let caps = Caps::builder("video/x-raw").build();
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
        .map_err(|e| NokhwaError::general(format!("DeviceMonitor::start: {e}")))?;
    let devices = monitor.devices();
    monitor.stop();

    if !display_name.is_empty() {
        if let Some(d) = devices
            .iter()
            .find(|d| d.display_name().as_str() == display_name)
        {
            return Ok(d.clone());
        }
    }
    devices
        .into_iter()
        .nth(positional_index as usize)
        .ok_or_else(|| NokhwaError::OpenDeviceError {
            device: format!("index={positional_index} name={display_name}"),
            error: "no matching device".to_string(),
        })
}

/// Pull the full capability set of a device as a flat
/// `Vec<CameraFormat>`.
pub(crate) fn compatible_formats(device: &Device) -> Vec<CameraFormat> {
    match device.caps() {
        Some(caps) => crate::format::caps_to_camera_formats(&caps),
        None => Vec::new(),
    }
}

/// Pick the best-matching format for `req` from `candidates`. Panics
/// with an error if nothing matches — same contract as MSMF's
/// `set_format`.
pub(crate) fn resolve_format(
    candidates: &[CameraFormat],
    req: &nokhwa_core::types::RequestedFormat,
) -> Result<CameraFormat, NokhwaError> {
    if candidates.is_empty() {
        return Err(NokhwaError::OpenDeviceError {
            device: "GStreamer device".to_string(),
            error: "no compatible formats".to_string(),
        });
    }
    req.fulfill(candidates)
        .ok_or_else(|| NokhwaError::OpenDeviceError {
            device: "GStreamer device".to_string(),
            error: format!("no format in the device's caps satisfied the request: {candidates:?}"),
        })
}

/// Distinct `FrameFormat`s across a candidate list, sorted in
/// `FrameFormat`'s `Ord` order. Mirrors the V4L / AVFoundation / MSMF
/// shape (`collect → sort → dedup`) so callers see a stable
/// cross-backend ordering regardless of how the underlying API
/// enumerated its caps.
pub(crate) fn compatible_fourcc(candidates: &[CameraFormat]) -> Vec<FrameFormat> {
    let mut out: Vec<FrameFormat> = candidates.iter().map(CameraFormat::format).collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nokhwa_core::types::{RequestedFormat, RequestedFormatType, Resolution};

    fn fmt(w: u32, h: u32, ff: FrameFormat, fps: u32) -> CameraFormat {
        CameraFormat::new(Resolution::new(w, h), ff, fps)
    }

    #[test]
    fn compatible_fourcc_empty_returns_empty() {
        assert_eq!(compatible_fourcc(&[]), Vec::<FrameFormat>::new());
    }

    #[test]
    fn compatible_fourcc_dedupes_and_sorts() {
        // Three duplicates of YUYV mixed with one NV12 and one MJPEG —
        // result must be one of each, sorted in `FrameFormat::Ord` order.
        let candidates = [
            fmt(640, 480, FrameFormat::YUYV, 30),
            fmt(1280, 720, FrameFormat::NV12, 30),
            fmt(640, 480, FrameFormat::YUYV, 60),
            fmt(1920, 1080, FrameFormat::MJPEG, 30),
            fmt(1280, 720, FrameFormat::YUYV, 30),
        ];
        let out = compatible_fourcc(&candidates);
        let mut expected = vec![FrameFormat::YUYV, FrameFormat::NV12, FrameFormat::MJPEG];
        expected.sort();
        assert_eq!(out, expected);
    }

    #[test]
    fn compatible_fourcc_singleton_returns_singleton() {
        let candidates = [fmt(640, 480, FrameFormat::GRAY, 30)];
        assert_eq!(compatible_fourcc(&candidates), vec![FrameFormat::GRAY]);
    }

    #[test]
    fn compatible_fourcc_ordering_matches_frame_format_ord() {
        // Construct candidates in reverse `FrameFormat::Ord` order — the
        // sort must place them in the canonical cross-backend ordering.
        let candidates = [
            fmt(640, 480, FrameFormat::RAWBGR, 30),
            fmt(640, 480, FrameFormat::RAWRGB, 30),
            fmt(640, 480, FrameFormat::GRAY, 30),
            fmt(640, 480, FrameFormat::NV12, 30),
            fmt(640, 480, FrameFormat::MJPEG, 30),
            fmt(640, 480, FrameFormat::YUYV, 30),
        ];
        let mut got = compatible_fourcc(&candidates);
        let mut expected = got.clone();
        expected.sort();
        assert_eq!(got, expected);
        // Idempotent: running it again is identical.
        got = compatible_fourcc(&got.iter().map(|f| fmt(1, 1, *f, 30)).collect::<Vec<_>>());
        assert_eq!(got, expected);
    }

    #[test]
    fn resolve_format_empty_candidates_errors_with_open_device() {
        // Pin the error string verbatim. Previously this used
        // `.contains("no compatible formats")` which would still pass
        // if a future refactor expanded the message to include extra
        // context (e.g. `"no compatible formats; got 0 candidates"`)
        // — that drift would silently invalidate downstream tests
        // that quote the canonical wording, and would diverge the
        // GStreamer message from the V4L / MSMF / AVFoundation paths
        // that callers may switch on by string content.
        let req =
            RequestedFormat::with_formats(RequestedFormatType::AbsoluteHighestResolution, &[]);
        let err = resolve_format(&[], &req).unwrap_err();
        match err {
            NokhwaError::OpenDeviceError { device, error } => {
                assert_eq!(device, "GStreamer device");
                assert_eq!(error, "no compatible formats");
            }
            other => panic!("expected OpenDeviceError, got {other:?}"),
        }
    }

    #[test]
    fn resolve_format_no_matching_format_errors() {
        // Candidates only carry MJPEG, but the request only accepts YUYV
        // — `fulfill` returns None and `resolve_format` must surface an
        // OpenDeviceError that includes the candidate list verbatim.
        //
        // The previous version checked
        // `error.contains("no format in the device's caps satisfied the
        // request")` and `error.contains("MJPEG")`. Both could pass
        // even after meaningful regressions:
        //   - rewording "satisfied the request" → "matched the request"
        //     (or any subtler tweak) would only break consumers who
        //     quote the canonical phrase, and `contains("MJPEG")`
        //     still trivially holds because every candidate prints its
        //     `FrameFormat` Debug;
        //   - swapping the `{candidates:?}` interpolation for a
        //     summary like `"{} candidates"`.len()` (a refactor that
        //     would superficially "shorten the message") would still
        //     drop the user's diagnostic — but `contains("MJPEG")`
        //     would fire only if the count format was lucky.
        // Pin the exact prefix `"no format in the device's caps
        // satisfied the request: "` and verify the suffix is the
        // `Debug` form of the `candidates` slice — that's the
        // documented contract: `format!("…: {candidates:?}")`.
        let candidates = [
            fmt(640, 480, FrameFormat::MJPEG, 30),
            fmt(1280, 720, FrameFormat::MJPEG, 30),
        ];
        let req = RequestedFormat::with_formats(
            RequestedFormatType::AbsoluteHighestResolution,
            &[FrameFormat::YUYV],
        );
        let err = resolve_format(&candidates, &req).unwrap_err();
        match err {
            NokhwaError::OpenDeviceError { device, error } => {
                assert_eq!(device, "GStreamer device");
                let expected =
                    format!("no format in the device's caps satisfied the request: {candidates:?}");
                assert_eq!(error, expected);
            }
            other => panic!("expected OpenDeviceError, got {other:?}"),
        }
    }

    #[test]
    fn resolve_format_picks_highest_resolution() {
        let candidates = [
            fmt(640, 480, FrameFormat::YUYV, 30),
            fmt(1920, 1080, FrameFormat::YUYV, 30),
            fmt(1280, 720, FrameFormat::YUYV, 30),
        ];
        let req = RequestedFormat::with_formats(
            RequestedFormatType::AbsoluteHighestResolution,
            &[FrameFormat::YUYV],
        );
        let chosen = resolve_format(&candidates, &req).unwrap();
        assert_eq!(chosen.resolution(), Resolution::new(1920, 1080));
        assert_eq!(chosen.format(), FrameFormat::YUYV);
    }

    #[test]
    fn resolve_format_picks_highest_framerate_at_max_resolution() {
        // Two entries share the max resolution; `fulfill` must tie-break
        // by framerate (highest wins).
        let candidates = [
            fmt(1920, 1080, FrameFormat::YUYV, 30),
            fmt(1920, 1080, FrameFormat::YUYV, 60),
            fmt(1280, 720, FrameFormat::YUYV, 120),
        ];
        let req = RequestedFormat::with_formats(
            RequestedFormatType::AbsoluteHighestResolution,
            &[FrameFormat::YUYV],
        );
        let chosen = resolve_format(&candidates, &req).unwrap();
        assert_eq!(chosen.resolution(), Resolution::new(1920, 1080));
        assert_eq!(chosen.frame_rate(), 60);
    }

    #[test]
    fn resolve_format_exact_returns_exact_match() {
        let target = fmt(1280, 720, FrameFormat::NV12, 30);
        let candidates = [
            fmt(640, 480, FrameFormat::YUYV, 30),
            target,
            fmt(1920, 1080, FrameFormat::MJPEG, 60),
        ];
        let req = RequestedFormat::with_formats(
            RequestedFormatType::Exact(target),
            &[FrameFormat::NV12, FrameFormat::YUYV, FrameFormat::MJPEG],
        );
        let chosen = resolve_format(&candidates, &req).unwrap();
        assert_eq!(chosen, target);
    }

    #[test]
    fn resolve_format_filters_by_wanted_decoder_list() {
        // Both formats are at 1920x1080 and the request asks for the
        // absolute-highest resolution, but the wanted-decoder list only
        // permits MJPEG — we must not pick the (otherwise tied) YUYV
        // entry.
        let candidates = [
            fmt(1920, 1080, FrameFormat::YUYV, 60),
            fmt(1920, 1080, FrameFormat::MJPEG, 30),
        ];
        let req = RequestedFormat::with_formats(
            RequestedFormatType::AbsoluteHighestResolution,
            &[FrameFormat::MJPEG],
        );
        let chosen = resolve_format(&candidates, &req).unwrap();
        assert_eq!(chosen.format(), FrameFormat::MJPEG);
    }
}
