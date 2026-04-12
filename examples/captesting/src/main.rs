use nokhwa::format_types::Mjpeg;
use nokhwa::frame::IntoRgb;
use nokhwa::utils::{CameraIndex, RequestedFormatType};
use nokhwa::Camera;

fn main() {
    let index = CameraIndex::Index(0);
    let mut camera =
        Camera::open::<Mjpeg>(index, RequestedFormatType::AbsoluteHighestResolution).unwrap();
    println!("{}", camera.camera_format());
    camera.open_stream().unwrap();
    let frame = camera.frame_typed().unwrap();
    camera.stop_stream().unwrap();
    let decoded = frame.into_rgb().materialize().unwrap();
    decoded
        .save_with_format("turtle.jpeg", image::ImageFormat::Jpeg)
        .unwrap();
}
