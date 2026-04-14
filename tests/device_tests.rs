//! Integration tests requiring a physical camera device.
//!
//! Only compiled when the `device-test` feature is enabled. CI runners
//! without a camera should leave the feature off — the tests will then
//! not be built at all. Exercises the post-0.13 `nokhwa::open` /
//! `OpenedCamera` API surface.

#![cfg(feature = "device-test")]

use nokhwa::{
    native_api_backend, open, query, CameraRunner, OpenRequest, OpenedCamera, RunnerConfig,
};
use nokhwa_core::error::NokhwaError;
use nokhwa_core::types::CameraIndex;
use std::time::Duration;

fn native_backend() -> nokhwa::utils::ApiBackend {
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
fn open_stream_and_capture_frames() -> Result<(), NokhwaError> {
    match open_first() {
        OpenedCamera::Stream(mut cam) => {
            cam.open()?;
            let res = cam.negotiated_format().resolution();
            for i in 0..5 {
                let buf = cam.frame()?;
                assert!(!buf.buffer().is_empty(), "frame {i} empty");
                assert_eq!(buf.resolution(), res);
            }
            cam.close()
        }
        OpenedCamera::Hybrid(mut cam) => {
            cam.open()?;
            let res = cam.negotiated_format().resolution();
            for i in 0..5 {
                let buf = cam.frame()?;
                assert!(!buf.buffer().is_empty(), "frame {i} empty");
                assert_eq!(buf.resolution(), res);
            }
            cam.close()
        }
        OpenedCamera::Shutter(_) => {
            panic!("expected a stream-capable camera, got Shutter-only")
        }
    }
}

#[test]
fn enumerate_controls_and_formats() -> Result<(), NokhwaError> {
    match open_first() {
        OpenedCamera::Stream(mut cam) => {
            let _ = cam.controls()?;
            let _ = cam.compatible_formats()?;
        }
        OpenedCamera::Hybrid(cam) => {
            let _ = cam.controls()?;
        }
        OpenedCamera::Shutter(cam) => {
            let _ = cam.controls()?;
        }
    }
    Ok(())
}

#[cfg(feature = "runner")]
#[test]
fn runner_produces_frames() -> Result<(), NokhwaError> {
    let opened = open(CameraIndex::Index(0), OpenRequest::any())?;
    let runner = CameraRunner::spawn(opened, RunnerConfig::default())?;
    let frames = runner
        .frames()
        .ok_or_else(|| NokhwaError::general("runner has no frames channel"))?;
    for i in 0..3 {
        let buf = frames
            .recv_timeout(Duration::from_secs(5))
            .map_err(|e| NokhwaError::general(format!("recv frame {i}: {e}")))?;
        assert!(!buf.buffer().is_empty(), "runner frame {i} empty");
    }
    runner.stop()
}
