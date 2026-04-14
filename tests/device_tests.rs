//! Integration tests requiring a physical camera device.
//!
//! Only compiled when the `device-test` feature is enabled. CI runners
//! without a camera should leave the feature off — the tests will then
//! not be built at all. Exercises the post-0.13 `nokhwa::open` /
//! `OpenedCamera` API surface.

#![cfg(feature = "device-test")]

use nokhwa::utils::{ApiBackend, CameraIndex};
use nokhwa::{native_api_backend, open, query, OpenRequest, OpenedCamera};

fn native_backend() -> ApiBackend {
    native_api_backend().expect("no native API backend compiled in for this target")
}

fn open_first() -> OpenedCamera {
    open(CameraIndex::Index(0), OpenRequest::any())
        .expect("open(CameraIndex::Index(0)) failed — is a camera attached?")
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
