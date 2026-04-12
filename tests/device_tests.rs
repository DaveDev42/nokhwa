//! Integration tests requiring a physical camera device.
//! Only compiled when `device-test` feature is enabled.
//! Run with: cargo test --features device-test,input-avfoundation,runner

#![cfg(feature = "device-test")]

use nokhwa::utils::*;
use nokhwa::{native_api_backend, query, Buffer, Camera};
use nokhwa_core::format_types::Mjpeg;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Returns the first camera index, or None if no camera is available.
fn require_camera() -> Option<CameraIndex> {
    match query(native_api_backend().unwrap()) {
        Ok(cameras) if !cameras.is_empty() => Some(cameras[0].index().clone()),
        _ => None,
    }
}

/// Intentionally hard-fails if no camera is found — verifying device discovery
/// is the most basic requirement for the self-hosted runner.
#[test]
fn query_devices_returns_cameras() {
    let backend = native_api_backend().expect("no native backend");
    let cameras = query(backend).expect("query failed");
    assert!(!cameras.is_empty(), "expected at least one camera");
    for cam in &cameras {
        println!(
            "Found camera: {} (index: {})",
            cam.human_name(),
            cam.index()
        );
    }
}

#[test]
fn open_camera_and_capture_frame() {
    let Some(idx) = require_camera() else {
        eprintln!("SKIP: no camera device found");
        return;
    };
    let mut camera = Camera::open::<Mjpeg>(idx, RequestedFormatType::AbsoluteHighestFrameRate)
        .expect("failed to open camera");

    camera.open_stream().expect("failed to open stream");
    assert!(camera.is_stream_open());

    let frame: Buffer = camera.frame().expect("failed to capture frame");
    assert!(!frame.buffer().is_empty(), "frame buffer is empty");
    assert!(frame.resolution().width() > 0);
    assert!(frame.resolution().height() > 0);

    camera.stop_stream().expect("failed to stop stream");
}

#[test]
fn query_compatible_formats() {
    let Some(idx) = require_camera() else {
        eprintln!("SKIP: no camera device found");
        return;
    };
    let mut camera = Camera::open::<Mjpeg>(idx, RequestedFormatType::AbsoluteHighestFrameRate)
        .expect("failed to open camera");

    let formats = camera
        .compatible_camera_formats()
        .expect("failed to get formats");
    assert!(
        !formats.is_empty(),
        "expected at least one compatible format"
    );

    let fourccs = camera.compatible_fourcc().expect("failed to get fourccs");
    assert!(!fourccs.is_empty(), "expected at least one fourcc");
}

#[cfg(feature = "runner")]
#[test]
fn callback_camera_receives_frames() {
    let Some(idx) = require_camera() else {
        eprintln!("SKIP: no camera device found");
        return;
    };
    let received = Arc::new(AtomicBool::new(false));
    let received_clone = received.clone();

    let format = RequestedFormat::new::<Mjpeg>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut camera = nokhwa::CallbackCamera::new(idx, format, move |_buffer| {
        received_clone.store(true, Ordering::SeqCst);
    })
    .expect("failed to create callback camera");

    camera.open_stream().expect("failed to open stream");
    // Wait up to 5 seconds for a frame
    for _ in 0..50 {
        if received.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    camera.stop_stream().expect("failed to stop stream");

    assert!(
        received.load(Ordering::SeqCst),
        "callback never received a frame within 5 seconds"
    );
}
