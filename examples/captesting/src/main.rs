use nokhwa::error::NokhwaError;
use nokhwa::format_types::{Mjpeg, Nv12, RawBgr, RawRgb, Yuyv};
use nokhwa::frame::{Frame, IntoRgb, RgbConversion};
use nokhwa::utils::{CameraIndex, FrameFormat};
use nokhwa::{open, Buffer, OpenRequest, OpenedCamera};

fn main() -> Result<(), NokhwaError> {
    let opened = open(CameraIndex::Index(0), OpenRequest::any())?;
    let OpenedCamera::Stream(mut camera) = opened else {
        return Err(NokhwaError::general("expected stream-capable camera"));
    };
    println!("{}", camera.negotiated_format());
    camera.open()?;
    let buffer = camera.frame()?;
    camera.close()?;
    let decoded = decode_to_rgb(buffer)?.materialize()?;
    decoded
        .save_with_format("turtle.jpeg", image::ImageFormat::Jpeg)
        .map_err(|e| NokhwaError::general(e.to_string()))?;
    Ok(())
}

/// Wrap a `Buffer` in the typed `Frame<F>` matching its own fourcc and
/// start a lazy RGB conversion. Dispatching on the buffer's format avoids
/// the panic that `Frame::<Mjpeg>::new` raises when the backend negotiates
/// a non-MJPEG format (most Linux webcams pick YUYV).
fn decode_to_rgb(buf: Buffer) -> Result<RgbConversion, NokhwaError> {
    match buf.source_frame_format() {
        FrameFormat::MJPEG => Ok(Frame::<Mjpeg>::try_new(buf)?.into_rgb()),
        FrameFormat::YUYV => Ok(Frame::<Yuyv>::try_new(buf)?.into_rgb()),
        FrameFormat::NV12 => Ok(Frame::<Nv12>::try_new(buf)?.into_rgb()),
        FrameFormat::RAWRGB => Ok(Frame::<RawRgb>::try_new(buf)?.into_rgb()),
        FrameFormat::RAWBGR => Ok(Frame::<RawBgr>::try_new(buf)?.into_rgb()),
        FrameFormat::GRAY => Err(NokhwaError::general(
            "captesting does not support GRAY/Luma cameras",
        )),
    }
}
