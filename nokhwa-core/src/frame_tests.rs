use crate::buffer::Buffer;
use crate::format_types::{Gray, Mjpeg, Nv12, RawBgr, RawRgb, Yuyv};
use crate::frame::{Frame, IntoLuma, IntoRgb, IntoRgba};
use crate::types::{FrameFormat, Resolution};

// ---------------------------------------------------------------------------
// Frame construction
// ---------------------------------------------------------------------------

#[test]
fn frame_new_rawrgb() {
    let data = vec![255u8; 2 * 2 * 3]; // 2x2 RGB
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::RAWRGB);
    let frame: Frame<RawRgb> = Frame::new(buf);
    assert_eq!(frame.resolution(), Resolution::new(2, 2));
    assert_eq!(frame.buffer().len(), 12);
}

#[test]
fn frame_new_gray() {
    let data = vec![128u8; 4]; // 2x2 gray
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::GRAY);
    let frame: Frame<Gray> = Frame::new(buf);
    assert_eq!(frame.resolution(), Resolution::new(2, 2));
}

#[test]
fn frame_into_buffer_roundtrip() {
    let data = vec![100u8; 12];
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::RAWRGB);
    let frame: Frame<RawRgb> = Frame::new(buf);
    let recovered = frame.into_buffer();
    assert_eq!(recovered.buffer(), &data[..]);
}

// ---------------------------------------------------------------------------
// RGB conversion
// ---------------------------------------------------------------------------

#[test]
fn rawrgb_into_rgb_materialize() {
    let data = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120];
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::RAWRGB);
    let frame: Frame<RawRgb> = Frame::new(buf);
    let img = frame.into_rgb().materialize().unwrap();
    assert_eq!(img.width(), 2);
    assert_eq!(img.height(), 2);
    // Raw RGB passthrough: first pixel should match
    assert_eq!(img.get_pixel(0, 0).0, [10, 20, 30]);
}

#[test]
fn rawbgr_into_rgb_swaps_channels() {
    // BGR: B=10, G=20, R=30
    let data = vec![10, 20, 30, 10, 20, 30, 10, 20, 30, 10, 20, 30];
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::RAWBGR);
    let frame: Frame<RawBgr> = Frame::new(buf);
    let img = frame.into_rgb().materialize().unwrap();
    // Should be swapped to R=30, G=20, B=10
    assert_eq!(img.get_pixel(0, 0).0, [30, 20, 10]);
}

#[test]
fn rawrgb_into_rgb_write_to() {
    let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::RAWRGB);
    let frame: Frame<RawRgb> = Frame::new(buf);
    let mut dest = vec![0u8; 12];
    frame.into_rgb().write_to(&mut dest).unwrap();
    assert_eq!(dest, data);
}

// ---------------------------------------------------------------------------
// RGBA conversion
// ---------------------------------------------------------------------------

#[test]
fn rawrgb_into_rgba_adds_alpha() {
    let data = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120];
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::RAWRGB);
    let frame: Frame<RawRgb> = Frame::new(buf);
    let img = frame.into_rgba().materialize().unwrap();
    assert_eq!(img.get_pixel(0, 0).0, [10, 20, 30, 255]);
}

#[test]
fn rawbgr_into_rgba_swaps_and_adds_alpha() {
    let data = vec![10, 20, 30, 10, 20, 30, 10, 20, 30, 10, 20, 30];
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::RAWBGR);
    let frame: Frame<RawBgr> = Frame::new(buf);
    let img = frame.into_rgba().materialize().unwrap();
    assert_eq!(img.get_pixel(0, 0).0, [30, 20, 10, 255]);
}

// ---------------------------------------------------------------------------
// Luma conversion
// ---------------------------------------------------------------------------

#[test]
fn gray_into_luma_passthrough() {
    let data = vec![50, 100, 150, 200]; // 2x2 gray
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::GRAY);
    let frame: Frame<Gray> = Frame::new(buf);
    let img = frame.into_luma().materialize().unwrap();
    assert_eq!(img.get_pixel(0, 0).0, [50]);
    assert_eq!(img.get_pixel(1, 1).0, [200]);
}

#[test]
fn rawrgb_into_luma_averages() {
    // RGB (30, 60, 90) -> avg = 60
    let data = vec![30, 60, 90, 30, 60, 90, 30, 60, 90, 30, 60, 90];
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::RAWRGB);
    let frame: Frame<RawRgb> = Frame::new(buf);
    let img = frame.into_luma().materialize().unwrap();
    assert_eq!(img.get_pixel(0, 0).0, [60]);
}

// ---------------------------------------------------------------------------
// YUYV luma extraction (direct Y channel)
// ---------------------------------------------------------------------------

#[test]
fn yuyv_into_luma_extracts_y() {
    // YUYV: [Y0=100, U=128, Y1=200, V=128] for 2 pixels
    // 2x2 image needs 2 YUYV chunks (4 pixels = 2 chunks of 4 bytes)
    let data = vec![100, 128, 200, 128, 50, 128, 150, 128];
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::YUYV);
    let frame: Frame<Yuyv> = Frame::new(buf);
    let img = frame.into_luma().materialize().unwrap();
    assert_eq!(img.get_pixel(0, 0).0, [100]);
    assert_eq!(img.get_pixel(1, 0).0, [200]);
    assert_eq!(img.get_pixel(0, 1).0, [50]);
    assert_eq!(img.get_pixel(1, 1).0, [150]);
}

// ---------------------------------------------------------------------------
// NV12 luma extraction (direct Y plane copy)
// ---------------------------------------------------------------------------

#[test]
fn nv12_into_luma_extracts_y_plane() {
    // 2x2 NV12: Y plane = 4 bytes, UV plane = 2 bytes (1 pair)
    let y_plane = [10u8, 20, 30, 40];
    let uv_plane = [128u8, 128]; // neutral chroma
    let mut data = Vec::new();
    data.extend_from_slice(&y_plane);
    data.extend_from_slice(&uv_plane);

    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::NV12);
    let frame: Frame<Nv12> = Frame::new(buf);
    let img = frame.into_luma().materialize().unwrap();
    assert_eq!(img.get_pixel(0, 0).0, [10]);
    assert_eq!(img.get_pixel(1, 0).0, [20]);
    assert_eq!(img.get_pixel(0, 1).0, [30]);
    assert_eq!(img.get_pixel(1, 1).0, [40]);
}

// ---------------------------------------------------------------------------
// Luma write_to
// ---------------------------------------------------------------------------

#[test]
fn gray_luma_write_to() {
    let data = vec![10, 20, 30, 40];
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::GRAY);
    let frame: Frame<Gray> = Frame::new(buf);
    let mut dest = vec![0u8; 4];
    frame.into_luma().write_to(&mut dest).unwrap();
    assert_eq!(dest, data);
}

#[test]
fn yuyv_luma_write_to() {
    let data = vec![100, 128, 200, 128, 50, 128, 150, 128];
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::YUYV);
    let frame: Frame<Yuyv> = Frame::new(buf);
    let mut dest = vec![0u8; 4];
    frame.into_luma().write_to(&mut dest).unwrap();
    assert_eq!(dest, [100, 200, 50, 150]);
}
