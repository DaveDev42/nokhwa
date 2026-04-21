//! End-to-end smoke test for the GStreamer session-2 streaming path.
//!
//! ```text
//! cargo run --features input-gstreamer --example gstreamer_probe
//! ```
//!
//! Enumerates devices via `query(ApiBackend::GStreamer)`, opens the
//! first one with `GStreamerCaptureDevice::new`, builds the
//! `source ! capsfilter ! videoconvert ! appsink` pipeline, pulls 5
//! frames, and prints a summary. Used for hardware verification in WSL
//! + usbipd setups before the `nokhwa::open()` dispatch integration
//! (session 4) lands.

#[cfg(feature = "input-gstreamer")]
fn main() -> Result<(), nokhwa_core::error::NokhwaError> {
    use nokhwa::backends::capture::GStreamerCaptureDevice;
    use nokhwa::query;
    use nokhwa_core::traits::FrameSource;
    use nokhwa_core::types::{
        color_frame_formats, ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType,
    };

    let cameras = query(ApiBackend::GStreamer)?;
    println!("Detected {} GStreamer source(s):", cameras.len());
    for (i, c) in cameras.iter().enumerate() {
        println!("  [{i}] {} | {}", c.human_name(), c.description());
    }
    if cameras.is_empty() {
        return Err(nokhwa_core::error::NokhwaError::general(
            "no GStreamer sources visible — is a webcam plugged in / attached via usbipd?",
        ));
    }

    // 640x480 NV12 30fps is the session-2 happy-path reference: small
    // enough to fit under WSL + usbip bandwidth budgets, big enough to
    // be meaningful, and exercises `videoconvert` when the device
    // prefers YUY2 natively. Higher resolutions work on direct USB
    // but may hit `usbipd` bandwidth caps in WSL.
    let req = RequestedFormat::with_formats(
        RequestedFormatType::Closest(nokhwa_core::types::CameraFormat::new(
            nokhwa_core::types::Resolution::new(640, 480),
            nokhwa_core::types::FrameFormat::NV12,
            30,
        )),
        color_frame_formats(),
    );
    let mut cam = GStreamerCaptureDevice::new(&CameraIndex::Index(0), req)?;
    println!("Negotiated: {:?}", cam.negotiated_format());

    cam.open()?;
    for i in 0..5 {
        let f = cam.frame()?;
        println!(
            "  frame[{i}]: {} bytes {:?} @ {}x{}",
            f.buffer().len(),
            f.source_frame_format(),
            f.resolution().width(),
            f.resolution().height()
        );
    }
    cam.close()?;
    println!("Done.");
    Ok(())
}

#[cfg(not(feature = "input-gstreamer"))]
fn main() {
    eprintln!(
        "This example requires the `input-gstreamer` feature.\n\
         Try: cargo run --features input-gstreamer --example gstreamer_probe"
    );
}
