#[cfg(feature = "input-gstreamer")]
fn main() -> Result<(), nokhwa_core::error::NokhwaError> {
    use nokhwa::backends::capture::GStreamerCaptureDevice;
    use nokhwa_core::traits::FrameSource;
    use nokhwa_core::types::{
        color_frame_formats, CameraIndex, RequestedFormat, RequestedFormatType,
    };

    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "file:///tmp/test.mp4".to_string());
    // RequestedFormat is ignored in URL mode (format is learned from
    // the stream) but the API still requires one.
    let req = RequestedFormat::with_formats(
        RequestedFormatType::AbsoluteHighestResolution,
        color_frame_formats(),
    );
    let mut cam = GStreamerCaptureDevice::new(&CameraIndex::String(url.clone()), req)?;
    println!("URL: {url}");
    cam.open()?;
    println!("Negotiated after open: {:?}", cam.negotiated_format());
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
    eprintln!("Requires `input-gstreamer`.");
}
