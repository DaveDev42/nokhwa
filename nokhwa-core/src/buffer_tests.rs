use super::*;
use crate::types::{FrameFormat, Resolution};

#[test]
fn buffer_new_stores_data() {
    let res = Resolution::new(2, 2);
    let data: Vec<u8> = vec![0; 12]; // 2x2 RGB = 12 bytes
    let buf = Buffer::new(res, &data, FrameFormat::RAWRGB);

    assert_eq!(buf.resolution(), res);
    assert_eq!(buf.source_frame_format(), FrameFormat::RAWRGB);
    assert_eq!(buf.buffer().len(), 12);
    assert_eq!(buf.buffer(), &data[..]);
}

#[test]
fn buffer_bytes_returns_clone() {
    let res = Resolution::new(1, 1);
    let data = vec![255, 0, 128];
    let buf = Buffer::new(res, &data, FrameFormat::RAWRGB);
    let bytes = buf.buffer_bytes();
    assert_eq!(&bytes[..], &data[..]);
}

#[test]
fn buffer_empty_data() {
    let res = Resolution::new(0, 0);
    let data: Vec<u8> = vec![];
    let buf = Buffer::new(res, &data, FrameFormat::MJPEG);
    assert_eq!(buf.buffer().len(), 0);
    assert_eq!(buf.resolution(), Resolution::new(0, 0));
}

#[test]
fn buffer_preserves_frame_format() {
    let res = Resolution::new(1, 1);
    let data = vec![0; 3];
    for fmt in crate::types::frame_formats() {
        let buf = Buffer::new(res, &data, *fmt);
        assert_eq!(buf.source_frame_format(), *fmt);
    }
}
