//! Integration tests requiring a physical camera device.
//!
//! Only compiled when the `device-test` feature is enabled. CI runners
//! without a camera should leave the feature off — the tests will then
//! not be built at all. Exercises the post-0.13 `nokhwa::open` /
//! `OpenedCamera` API surface.

#![cfg(feature = "device-test")]

use nokhwa::utils::{
    ApiBackend, CameraFormat, CameraIndex, ControlValueDescription, ControlValueSetter,
    FrameFormat, KnownCameraControlFlag, Resolution,
};
use nokhwa::{native_api_backend, open, query, OpenRequest, OpenedCamera};

fn native_backend() -> ApiBackend {
    native_api_backend().expect("no native API backend compiled in for this target")
}

fn open_first() -> OpenedCamera {
    open(CameraIndex::Index(0), OpenRequest::any())
        .expect("open(CameraIndex::Index(0)) failed — is a camera attached?")
}

/// MSMF hotplug plumbing: `take_hotplug_events()` succeeds once and
/// errors on the second call, the returned poller stays quiet under a
/// steady-state device set, and dropping the poller joins the
/// background thread cleanly (the test would hang if the Drop impl
/// regressed). A manual plug/unplug observation is out of scope for
/// the automated suite but the plumbing is fully exercised here.
#[cfg(all(feature = "input-msmf", target_os = "windows"))]
#[test]
fn msmf_hotplug_take_and_steady_state() {
    use nokhwa::backends::hotplug::MediaFoundationHotplugContext;
    use nokhwa_core::traits::HotplugSource;
    use std::time::Duration;

    let mut ctx = MediaFoundationHotplugContext::new();
    let mut poll = ctx
        .take_hotplug_events()
        .expect("first take_hotplug_events must succeed");

    assert!(
        ctx.take_hotplug_events().is_err(),
        "second take_hotplug_events must error per the trait contract"
    );

    // Three poll windows (~1.5s) with no plug/unplug → no events.
    let evt = poll.next_timeout(Duration::from_millis(1500));
    assert!(
        evt.is_none(),
        "expected no hotplug event on steady state, got {evt:?}"
    );

    drop(poll);
}

/// V4L hotplug plumbing: same shape as the MSMF test. Exercises the
/// `take_hotplug_events()` contract, the steady-state silence
/// guarantee on an inotify-driven worker (no spurious wakeups
/// translating into events), and clean Drop-time join. The
/// v4l-loopback CI job auto-validates the actual `Connected` /
/// `Disconnected` emission via `modprobe -r/+r v4l2loopback`.
#[cfg(all(feature = "input-v4l", target_os = "linux"))]
#[test]
fn v4l_hotplug_take_and_steady_state() {
    use nokhwa::backends::hotplug::V4LHotplugContext;
    use nokhwa_core::traits::HotplugSource;
    use std::time::Duration;

    let mut ctx = V4LHotplugContext::new();
    let mut poll = ctx
        .take_hotplug_events()
        .expect("first take_hotplug_events must succeed");

    assert!(
        ctx.take_hotplug_events().is_err(),
        "second take_hotplug_events must error per the trait contract"
    );

    // Three poll windows (~1.5s) with no plug/unplug → no events.
    let evt = poll.next_timeout(Duration::from_millis(1500));
    assert!(
        evt.is_none(),
        "expected no hotplug event on steady state, got {evt:?}"
    );

    drop(poll);
}

/// AVFoundation hotplug plumbing: same shape as the MSMF and V4L
/// tests. Exercises the `take_hotplug_events()` contract and the
/// steady-state silence guarantee on the 500ms polling worker, plus
/// clean Drop-time join. Manual plug/unplug observation against a
/// physical USB camera is out of scope for the automated suite, but
/// the wiring is fully exercised here on the self-hosted
/// `macos-camera` runner.
#[cfg(all(feature = "input-avfoundation", target_os = "macos"))]
#[test]
fn avfoundation_hotplug_take_and_steady_state() {
    use nokhwa::backends::hotplug::AVFoundationHotplugContext;
    use nokhwa_core::traits::HotplugSource;
    use std::time::Duration;

    let mut ctx = AVFoundationHotplugContext::new();
    let mut poll = ctx
        .take_hotplug_events()
        .expect("first take_hotplug_events must succeed");

    assert!(
        ctx.take_hotplug_events().is_err(),
        "second take_hotplug_events must error per the trait contract"
    );

    // Three poll windows (~1.5s) with no plug/unplug → no events.
    let evt = poll.next_timeout(Duration::from_millis(1500));
    assert!(
        evt.is_none(),
        "expected no hotplug event on steady state, got {evt:?}"
    );

    drop(poll);
}

#[test]
fn query_reports_at_least_one_device() {
    let devices = query(native_backend()).expect("query() returned an error");
    assert!(
        !devices.is_empty(),
        "no cameras found — these tests require a physical camera"
    );
}

#[test]
fn open_stream_and_capture_frames() {
    match open_first() {
        OpenedCamera::Stream(mut cam) => {
            cam.open().expect("StreamCamera::open");
            let res = cam.negotiated_format().resolution();
            for i in 0..5 {
                let buf = cam.frame().expect("StreamCamera::frame");
                assert!(!buf.buffer().is_empty(), "frame {i} empty");
                assert_eq!(buf.resolution(), res);
            }
            cam.close().expect("StreamCamera::close");
        }
        OpenedCamera::Hybrid(mut cam) => {
            cam.open().expect("HybridCamera::open");
            let res = cam.negotiated_format().resolution();
            for i in 0..5 {
                let buf = cam.frame().expect("HybridCamera::frame");
                assert!(!buf.buffer().is_empty(), "frame {i} empty");
                assert_eq!(buf.resolution(), res);
            }
            cam.close().expect("HybridCamera::close");
        }
        OpenedCamera::Shutter(_) => {
            panic!("expected a stream-capable camera, got Shutter-only")
        }
    }
}

#[test]
fn enumerate_controls_and_formats() {
    match open_first() {
        // Stream needs `&mut` for `compatible_formats()`; the other
        // two wrappers expose `controls()` via `&self`.
        OpenedCamera::Stream(mut cam) => {
            cam.controls().expect("StreamCamera::controls");
            cam.compatible_formats()
                .expect("StreamCamera::compatible_formats");
        }
        OpenedCamera::Hybrid(cam) => {
            cam.controls().expect("HybridCamera::controls");
        }
        OpenedCamera::Shutter(cam) => {
            cam.controls().expect("ShutterCamera::controls");
        }
    }
}

#[test]
fn control_set_get_round_trip() {
    macro_rules! round_trip {
        ($cam:expr) => {{
            let cam = $cam;
            let controls = cam.controls().expect("controls()");

            // Prefer a Manual-mode IntegerRange control with headroom. Automatic-
            // flagged controls are skipped because set_control on MSMF preserves
            // the current flag: writing a value while the driver still owns the
            // control may round-trip intermittently.
            let candidate = controls.iter().find_map(|c| {
                let disqualified = c.flag().iter().any(|f| {
                    matches!(
                        f,
                        KnownCameraControlFlag::ReadOnly
                            | KnownCameraControlFlag::WriteOnly
                            | KnownCameraControlFlag::Disabled
                            | KnownCameraControlFlag::Automatic
                    )
                });
                if disqualified {
                    return None;
                }
                match c.description() {
                    ControlValueDescription::IntegerRange {
                        min,
                        max,
                        value,
                        step,
                        ..
                    } if *max > *min && *step > 0 => Some((c.control(), *min, *max, *step, *value)),
                    _ => None,
                }
            });

            let Some((id, min, max, step, current)) = candidate else {
                eprintln!(
                    "control_set_get_round_trip: no writable IntegerRange control in Manual mode \
                     is exposed by this device; skipping."
                );
                return;
            };

            let target = if current + step <= max {
                current + step
            } else if current - step >= min {
                current - step
            } else {
                eprintln!(
                    "control_set_get_round_trip: {id:?} has no headroom \
                     (min={min} max={max} step={step} value={current}); skipping."
                );
                return;
            };

            eprintln!(
                "control_set_get_round_trip: using {id:?} \
                 (min={min} max={max} step={step} current={current} target={target})"
            );

            cam.set_control(id, ControlValueSetter::Integer(target))
                .unwrap_or_else(|e| panic!("set_control({id:?}, {target}): {e}"));

            let after = cam.controls().expect("controls() after set");
            let updated = after
                .iter()
                .find(|c| c.control() == id)
                .expect("control disappeared after set");
            match updated.description() {
                ControlValueDescription::IntegerRange { value, .. } => assert_eq!(
                    *value, target,
                    "{id:?} did not round-trip: wanted {target}, got {value}"
                ),
                d => panic!("{id:?} changed description variant: {d:?}"),
            }

            let _ = cam.set_control(id, ControlValueSetter::Integer(current));
        }};
    }

    match open_first() {
        OpenedCamera::Stream(mut cam) => round_trip!(&mut cam),
        OpenedCamera::Hybrid(mut cam) => round_trip!(&mut cam),
        OpenedCamera::Shutter(mut cam) => round_trip!(&mut cam),
    }
}

/// Opening a non-existent index must surface a `NokhwaError` rather
/// than panicking or returning a bogus camera. Index 999 is well past
/// any realistic device count.
#[test]
fn open_invalid_index_errors() {
    let res = open(CameraIndex::Index(999), OpenRequest::any());
    assert!(
        res.is_err(),
        "open(CameraIndex::Index(999)) unexpectedly succeeded"
    );
}

/// `compatible_formats()` must enumerate at least one entry on a real
/// device. An empty list would mean negotiation has nothing to work
/// against.
#[test]
fn compatible_formats_nonempty() {
    let OpenedCamera::Stream(mut cam) = open_first() else {
        eprintln!("compatible_formats_nonempty: backend is not Stream-capable; skipping.");
        return;
    };
    let formats = cam
        .compatible_formats()
        .expect("StreamCamera::compatible_formats");
    assert!(!formats.is_empty(), "compatible_formats() returned empty");
}

/// Requesting a format the device cannot serve (1×1 @ 1 fps) must
/// either error or fail to round-trip. Backends differ — V4L2 may
/// silently snap to the nearest valid format, MSMF tends to error —
/// so this test accepts either outcome and just rejects the
/// "succeeded *and* round-tripped the bogus value" combination.
#[test]
fn set_format_invalid_does_not_round_trip() {
    let OpenedCamera::Stream(mut cam) = open_first() else {
        eprintln!(
            "set_format_invalid_does_not_round_trip: backend is not Stream-capable; skipping."
        );
        return;
    };
    let bogus = CameraFormat::new(Resolution::new(1, 1), FrameFormat::MJPEG, 1);
    match cam.set_format(bogus) {
        Err(_) => {} // expected on most backends
        Ok(()) => {
            let got = cam.negotiated_format();
            assert_ne!(
                got, bogus,
                "set_format accepted a 1x1@1 MJPEG format and round-tripped it; \
                 driver should have either errored or snapped to a real format"
            );
        }
    }
}

/// Multiple consecutive `frame()` calls must report a stable
/// resolution and source format. A regression here would mean the
/// stream is silently re-negotiating mid-stream, which would break
/// downstream `Buffer::typed::<F>()` consumers.
#[test]
fn frame_metadata_is_stable() {
    let OpenedCamera::Stream(mut cam) = open_first() else {
        eprintln!("frame_metadata_is_stable: backend is not Stream-capable; skipping.");
        return;
    };
    cam.open().expect("StreamCamera::open");
    let first = cam.frame().expect("first frame");
    let res = first.resolution();
    let fmt = first.source_frame_format();
    for i in 1..4 {
        let buf = cam.frame().expect("frame()");
        assert_eq!(buf.resolution(), res, "resolution drifted at frame {i}");
        assert_eq!(
            buf.source_frame_format(),
            fmt,
            "source format drifted at frame {i}"
        );
    }
    cam.close().expect("StreamCamera::close");
}

/// `compatible_fourcc()` must enumerate at least one entry, and every
/// entry must be a `FrameFormat` that also appears in
/// `compatible_formats()`. This is the invariant that the MSMF
/// truncation bug in #194 violated — `compatible_fourcc` returned at
/// most 2 entries while `compatible_formats` could expose all 4
/// (MJPEG / YUYV / NV12 / GRAY), so a UI that branched on
/// `compatible_fourcc` saw a strict subset of what the device
/// actually supported.
#[test]
fn compatible_fourcc_is_subset_of_compatible_formats() {
    let OpenedCamera::Stream(mut cam) = open_first() else {
        eprintln!(
            "compatible_fourcc_is_subset_of_compatible_formats: backend is not Stream-capable; skipping."
        );
        return;
    };
    let fourccs = cam
        .compatible_fourcc()
        .expect("StreamCamera::compatible_fourcc");
    assert!(!fourccs.is_empty(), "compatible_fourcc() returned empty");

    let formats = cam
        .compatible_formats()
        .expect("StreamCamera::compatible_formats");
    let formats_fourccs: std::collections::HashSet<FrameFormat> =
        formats.iter().map(|f| f.format()).collect();
    for ff in &fourccs {
        assert!(
            formats_fourccs.contains(ff),
            "compatible_fourcc returned {ff:?} which is not in compatible_formats() = {formats_fourccs:?}"
        );
    }
}

/// Round-trip an arbitrary entry from `compatible_formats()` through
/// `set_format()` and confirm `negotiated_format()` reports the same
/// values. Catches drift between `compatible_formats` (what the
/// backend says it supports) and `set_format` (what the backend
/// actually accepts) — these can diverge if a backend's
/// `compatible_formats` returns synthesised entries the driver can't
/// honour at `set_format` time.
#[test]
fn set_format_from_compatible_round_trip() {
    let OpenedCamera::Stream(mut cam) = open_first() else {
        eprintln!(
            "set_format_from_compatible_round_trip: backend is not Stream-capable; skipping."
        );
        return;
    };
    let formats = cam
        .compatible_formats()
        .expect("StreamCamera::compatible_formats");
    let Some(target) = formats.into_iter().next() else {
        eprintln!("set_format_from_compatible_round_trip: no compatible formats; skipping.");
        return;
    };
    cam.set_format(target).unwrap_or_else(|e| {
        panic!("set_format({target:?}) returned error on a value from compatible_formats(): {e}")
    });
    let got = cam.negotiated_format();
    assert_eq!(
        got, target,
        "negotiated_format mismatched after set_format({target:?}); got {got:?}"
    );
}

/// `info()` and `backend()` must reflect the device that was opened —
/// `info().index()` matches the index passed to `open()`, and
/// `backend()` matches the platform's `native_api_backend()`. Catches
/// a regression where a wrapper's `info()` returns stale or
/// constructor-default data instead of pass-through to the backend.
#[test]
fn opened_camera_info_and_backend_reflect_request() {
    let cam = open_first();
    let (backend, info) = match &cam {
        OpenedCamera::Stream(c) => (c.backend(), c.info()),
        OpenedCamera::Shutter(c) => (c.backend(), c.info()),
        OpenedCamera::Hybrid(c) => (c.backend(), c.info()),
    };
    assert_eq!(
        backend,
        native_backend(),
        "OpenedCamera::backend() must match native_api_backend() for an index-opened device"
    );
    assert_eq!(
        info.index(),
        &CameraIndex::Index(0),
        "OpenedCamera::info().index() must echo the index passed to open()"
    );
    assert!(
        !info.human_name().is_empty(),
        "OpenedCamera::info().human_name() must be non-empty for a real device"
    );
}

/// The dual-form `CameraIndex` contract — a numeric `String` is a
/// valid index, by parsing — must hold through the public `open()`
/// dispatcher, not just at the `as_index()` unit-test layer.
/// `open(CameraIndex::String("0"))` must reach the same native
/// backend as `open(CameraIndex::Index(0))`. Regression here would
/// silently route numeric-string callers to GStreamer's URL path
/// (which expects `rtsp://`/`http://`/`file://`) and produce a
/// backend mismatch on the resulting `OpenedCamera`.
#[test]
fn open_numeric_string_routes_to_native_backend() {
    let cam = open(CameraIndex::String("0".to_string()), OpenRequest::any())
        .expect("open(CameraIndex::String(\"0\")) must dispatch to the native backend");
    let backend = match &cam {
        OpenedCamera::Stream(c) => c.backend(),
        OpenedCamera::Shutter(c) => c.backend(),
        OpenedCamera::Hybrid(c) => c.backend(),
    };
    assert_eq!(
        backend,
        native_backend(),
        "open(String(\"0\")) reached the wrong backend; numeric-string dispatch is broken"
    );
}

/// `frame()` must return non-empty bytes. `frame_metadata_is_stable`
/// already pins resolution + source format across frames; this is the
/// matching pin for the actual payload. A regression that returns
/// `Buffer { buffer: Cow::Borrowed(&[]), .. }` would slip past every
/// existing test because they only inspect metadata.
#[test]
fn frame_buffer_is_non_empty() {
    let OpenedCamera::Stream(mut cam) = open_first() else {
        eprintln!("frame_buffer_is_non_empty: backend is not Stream-capable; skipping.");
        return;
    };
    cam.open().expect("StreamCamera::open");
    let buf = cam.frame().expect("StreamCamera::frame");
    assert!(
        !buf.buffer().is_empty(),
        "frame() returned a Buffer with zero payload bytes — backend is producing empty frames"
    );
    cam.close().expect("StreamCamera::close");
}

/// `is_open()` must reflect the open/close lifecycle: false before
/// `open()`, true after `open()`, false again after `close()`. A
/// regression where `is_open()` is hardcoded `true` (or never updated
/// on `close()`) would silently break callers that branch on this
/// flag for re-init logic.
#[test]
fn stream_camera_is_open_lifecycle() {
    let OpenedCamera::Stream(mut cam) = open_first() else {
        eprintln!("stream_camera_is_open_lifecycle: backend is not Stream-capable; skipping.");
        return;
    };
    assert!(
        !cam.is_open(),
        "is_open() must be false before StreamCamera::open()"
    );
    cam.open().expect("StreamCamera::open");
    assert!(
        cam.is_open(),
        "is_open() must be true after StreamCamera::open()"
    );
    cam.close().expect("StreamCamera::close");
    assert!(
        !cam.is_open(),
        "is_open() must be false after StreamCamera::close()"
    );
}

/// `frame_raw()` is the zero-copy sibling of `frame()`. The raw byte
/// slice must be non-empty for the same reason `frame()`'s `Buffer`
/// payload must be — a regression that returns `Cow::Borrowed(&[])`
/// would slip past `frame_buffer_is_non_empty` (which goes through
/// `frame()`) and silently break low-level consumers that read
/// `frame_raw()` directly to avoid the `Buffer` copy.
#[test]
fn frame_raw_is_non_empty() {
    let OpenedCamera::Stream(mut cam) = open_first() else {
        eprintln!("frame_raw_is_non_empty: backend is not Stream-capable; skipping.");
        return;
    };
    cam.open().expect("StreamCamera::open");
    let raw = cam.frame_raw().expect("StreamCamera::frame_raw");
    assert!(
        !raw.is_empty(),
        "frame_raw() returned a zero-length slice — backend is producing empty frames"
    );
    cam.close().expect("StreamCamera::close");
}

/// Indices returned by `query()` must be openable by `open()`. The
/// dual-form `CameraIndex` contract has the parsed-from-string path
/// (covered by `open_numeric_string_routes_to_native_backend`); this
/// is the matching pin for the typed `Index` path: `query()` is the
/// canonical enumerator, and a `CameraInfo` it returns must round-trip
/// through `open()` without surprise. A regression — `query()`
/// reporting an index `open()` rejects — would break every consumer
/// that picks devices by enumeration.
#[test]
fn query_results_are_openable() {
    let devices = query(native_backend()).expect("query() returned an error");
    let Some(first) = devices.first() else {
        eprintln!("query_results_are_openable: query returned empty; skipping.");
        return;
    };
    let CameraIndex::Index(idx) = first.index() else {
        eprintln!(
            "query_results_are_openable: native query returned a non-Index variant ({:?}); skipping.",
            first.index()
        );
        return;
    };
    let cam = open(CameraIndex::Index(*idx), OpenRequest::any())
        .unwrap_or_else(|e| panic!("open(query[0].index = {idx}) failed: {e}"));
    let backend = match &cam {
        OpenedCamera::Stream(c) => c.backend(),
        OpenedCamera::Shutter(c) => c.backend(),
        OpenedCamera::Hybrid(c) => c.backend(),
    };
    assert_eq!(
        backend,
        native_backend(),
        "open() of a query-reported index reached the wrong backend"
    );
}

/// Every entry in `compatible_formats()` must have a non-zero
/// resolution. A 0×0 entry would silently feed into `set_format()` /
/// `RequestedFormat::Exact` and either error or produce a degenerate
/// stream, depending on the backend. The closest-match negotiation
/// path also assumes positive resolutions for distance computation.
#[test]
fn compatible_formats_have_nonzero_resolutions() {
    let OpenedCamera::Stream(mut cam) = open_first() else {
        eprintln!(
            "compatible_formats_have_nonzero_resolutions: backend is not Stream-capable; skipping."
        );
        return;
    };
    let formats = cam
        .compatible_formats()
        .expect("StreamCamera::compatible_formats");
    for f in &formats {
        assert!(
            f.resolution().width() > 0,
            "compatible_formats() returned a 0-width entry: {f:?}"
        );
        assert!(
            f.resolution().height() > 0,
            "compatible_formats() returned a 0-height entry: {f:?}"
        );
    }
}

// The file is already gated by `device-test` at the top, so this
// submodule's effective gate is `device-test AND runner` — i.e. it
// compiles only when both features are enabled.
#[cfg(feature = "runner")]
mod runner_tests {
    use super::{open, CameraIndex, OpenRequest};
    use nokhwa::{CameraRunner, RunnerConfig};
    use std::time::Duration;

    #[test]
    fn runner_produces_frames() {
        let opened = open(CameraIndex::Index(0), OpenRequest::any())
            .expect("open(CameraIndex::Index(0)) failed");
        let runner =
            CameraRunner::spawn(opened, RunnerConfig::default()).expect("CameraRunner::spawn");
        let frames = runner.frames().expect("runner has no frames channel");
        for i in 0..3 {
            let buf = frames
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or_else(|e| panic!("recv frame {i}: {e}"));
            assert!(!buf.buffer().is_empty(), "runner frame {i} empty");
        }
        runner.stop().expect("runner.stop()");
    }
}
