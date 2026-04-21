//! Integration tests requiring a physical camera device.
//!
//! Only compiled when the `device-test` feature is enabled. CI runners
//! without a camera should leave the feature off — the tests will then
//! not be built at all. Exercises the post-0.13 `nokhwa::open` /
//! `OpenedCamera` API surface.

#![cfg(feature = "device-test")]

use nokhwa::utils::{
    ApiBackend, CameraIndex, ControlValueDescription, ControlValueSetter, KnownCameraControlFlag,
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
