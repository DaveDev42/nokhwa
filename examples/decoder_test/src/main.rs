use nokhwa_core::buffer::Buffer;
use nokhwa_core::format_types::Nv12;
use nokhwa_core::frame::{Frame, IntoRgb};
use nokhwa_core::types::{FrameFormat, Resolution};
use std::fs::File;
use std::io::Read;

fn main() {
    let mut nv12 = Vec::new();
    File::open("cchlop.nv12")
        .unwrap()
        .read_to_end(&mut nv12)
        .unwrap();

    let buffer = Buffer::new(Resolution::new(1920, 1080), &nv12, FrameFormat::NV12);
    let frame: Frame<Nv12> = Frame::new(buffer);
    frame
        .into_rgb()
        .materialize()
        .unwrap()
        .save("cchlop_out_nv12.png")
        .unwrap();
}
