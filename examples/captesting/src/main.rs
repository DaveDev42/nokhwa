use nokhwa::format_types::Mjpeg;
use nokhwa::frame::{Frame, IntoRgb};
use nokhwa::error::NokhwaError;
use nokhwa::utils::CameraIndex;
use nokhwa::{open, OpenRequest, OpenedCamera};

fn main() -> Result<(), NokhwaError> {
    let opened = open(CameraIndex::Index(0), OpenRequest::any())?;
    let OpenedCamera::Stream(mut camera) = opened else {
        return Err(NokhwaError::general("expected stream-capable camera"));
    };
    println!("{}", camera.negotiated_format());
    camera.open()?;
    let buffer = camera.frame()?;
    camera.close()?;
    let frame: Frame<Mjpeg> = Frame::new(buffer);
    let decoded = frame.into_rgb().materialize()?;
    decoded
        .save_with_format("turtle.jpeg", image::ImageFormat::Jpeg)
        .map_err(|e| NokhwaError::general(e.to_string()))?;
    Ok(())
}
