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

#[test]
fn frame_try_new_mismatch_returns_error() {
    let data = vec![128u8; 4]; // 2x2 gray
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::GRAY);
    // Try to wrap a GRAY buffer in a Frame<RawRgb> — should fail
    let result = Frame::<RawRgb>::try_new(buf);
    assert!(result.is_err());
}

#[test]
fn frame_try_new_matching_succeeds() {
    let data = vec![128u8; 4];
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::GRAY);
    let result = Frame::<Gray>::try_new(buf);
    assert!(result.is_ok());
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

// ---------------------------------------------------------------------------
// MJPEG conversion (requires "mjpeg" feature, not WASM)
// ---------------------------------------------------------------------------

/// A valid 2×2 solid-red JPEG (quality 100, generated by ImageMagick).
///
/// After JPEG round-trip through YCbCr, the decoded RGB values will be close
/// to (255, 0, 0) but not exact due to lossy compression.
#[cfg(all(feature = "mjpeg", not(target_arch = "wasm32")))]
const JPEG_RED_2X2: &[u8] = &[
    0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xff, 0xdb, 0x00, 0x43, 0x00, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0xff, 0xdb, 0x00, 0x43, 0x01, 0x01, 0x01,
    0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0xff, 0xc0,
    0x00, 0x11, 0x08, 0x00, 0x02, 0x00, 0x02, 0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11,
    0x01, 0xff, 0xc4, 0x00, 0x14, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x09, 0xff, 0xc4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xc4, 0x00,
    0x15, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x09, 0x0a, 0xff, 0xc4, 0x00, 0x14, 0x11, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xda, 0x00, 0x0c, 0x03, 0x01,
    0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3f, 0x00, 0x17, 0xc5, 0x3a, 0xfe, 0x1f, 0xff, 0xd9,
];

/// Helper: assert all pixel channels are within `tolerance` of expected.
/// The `channels` parameter is used only for diagnostic formatting (pixel index / channel id).
#[cfg(all(feature = "mjpeg", not(target_arch = "wasm32")))]
fn assert_pixels_near(actual: &[u8], expected: &[u8], channels: usize, tolerance: u8) {
    assert_ne!(channels, 0, "channels must be non-zero");
    assert_eq!(
        actual.len(),
        expected.len(),
        "pixel data length mismatch: got {} expected {}",
        actual.len(),
        expected.len()
    );
    for (i, (&a, &e)) in actual.iter().zip(expected.iter()).enumerate() {
        let diff = if a > e { a - e } else { e - a };
        assert!(
            diff <= tolerance,
            "channel {} of pixel {}: got {a}, expected {e} (diff {diff} > tolerance {tolerance})",
            i % channels,
            i / channels,
        );
    }
}

#[cfg(all(feature = "mjpeg", not(target_arch = "wasm32")))]
#[test]
fn mjpeg_into_rgb_produces_correct_output() {
    let buf = Buffer::new(Resolution::new(2, 2), JPEG_RED_2X2, FrameFormat::MJPEG);
    let frame: Frame<Mjpeg> = Frame::new(buf);
    let img = frame.into_rgb().materialize().unwrap();
    assert_eq!(img.width(), 2);
    assert_eq!(img.height(), 2);
    // All 4 pixels should be close to red (255, 0, 0).
    // JPEG YCbCr round-trip introduces small errors.
    let expected = [255, 0, 0].repeat(4);
    assert_pixels_near(img.as_raw(), &expected, 3, 5);
}

#[cfg(all(feature = "mjpeg", not(target_arch = "wasm32")))]
#[test]
fn mjpeg_into_rgba_produces_correct_output() {
    let buf = Buffer::new(Resolution::new(2, 2), JPEG_RED_2X2, FrameFormat::MJPEG);
    let frame: Frame<Mjpeg> = Frame::new(buf);
    let img = frame.into_rgba().materialize().unwrap();
    assert_eq!(img.width(), 2);
    assert_eq!(img.height(), 2);
    // All 4 pixels should be close to red with full alpha.
    // Alpha is always exactly 255 (not lossy); the tolerance applies uniformly to all
    // channels but alpha matches exactly since expected == actual == 255.
    let expected = [255, 0, 0, 255].repeat(4);
    assert_pixels_near(img.as_raw(), &expected, 4, 5);
}

#[cfg(all(feature = "mjpeg", not(target_arch = "wasm32")))]
#[test]
fn mjpeg_into_luma_produces_correct_output() {
    let buf = Buffer::new(Resolution::new(2, 2), JPEG_RED_2X2, FrameFormat::MJPEG);
    let frame: Frame<Mjpeg> = Frame::new(buf);
    let img = frame.into_luma().materialize().unwrap();
    assert_eq!(img.width(), 2);
    assert_eq!(img.height(), 2);
    // Luma = (R+G+B)/3. For near-red (≈255,0,0) that's ≈85.
    let expected = [85u8; 4];
    assert_pixels_near(img.as_raw(), &expected, 1, 5);
}

#[cfg(all(feature = "mjpeg", not(target_arch = "wasm32")))]
#[test]
fn mjpeg_rgb_write_to() {
    let buf = Buffer::new(Resolution::new(2, 2), JPEG_RED_2X2, FrameFormat::MJPEG);
    let frame: Frame<Mjpeg> = Frame::new(buf);
    let mut dest = vec![0u8; 2 * 2 * 3];
    frame.into_rgb().write_to(&mut dest).unwrap();
    // Same tolerance check as materialize — near-red pixels
    let expected = [255, 0, 0].repeat(4);
    assert_pixels_near(&dest, &expected, 3, 5);
}

#[cfg(all(feature = "mjpeg", not(target_arch = "wasm32")))]
#[test]
fn mjpeg_rgba_write_to() {
    let buf = Buffer::new(Resolution::new(2, 2), JPEG_RED_2X2, FrameFormat::MJPEG);
    let frame: Frame<Mjpeg> = Frame::new(buf);
    let mut dest = vec![0u8; 2 * 2 * 4];
    frame.into_rgba().write_to(&mut dest).unwrap();
    let expected = [255, 0, 0, 255].repeat(4);
    assert_pixels_near(&dest, &expected, 4, 5);
}

#[cfg(all(feature = "mjpeg", not(target_arch = "wasm32")))]
#[test]
fn mjpeg_luma_write_to() {
    let buf = Buffer::new(Resolution::new(2, 2), JPEG_RED_2X2, FrameFormat::MJPEG);
    let frame: Frame<Mjpeg> = Frame::new(buf);
    let mut dest = vec![0u8; 2 * 2];
    frame.into_luma().write_to(&mut dest).unwrap();
    let expected = [85u8; 4];
    assert_pixels_near(&dest, &expected, 1, 5);
}

#[cfg(all(feature = "mjpeg", not(target_arch = "wasm32")))]
#[test]
fn mjpeg_malformed_returns_error() {
    // Starts with valid JPEG SOI marker but truncated
    let garbage = &[0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x00, 0x00];
    let buf = Buffer::new(Resolution::new(2, 2), garbage, FrameFormat::MJPEG);
    let frame: Frame<Mjpeg> = Frame::new(buf);
    assert!(frame.into_rgb().materialize().is_err());
}

#[cfg(all(feature = "mjpeg", not(target_arch = "wasm32")))]
#[test]
fn mjpeg_empty_returns_error() {
    let buf = Buffer::new(Resolution::new(2, 2), &[], FrameFormat::MJPEG);
    let frame: Frame<Mjpeg> = Frame::new(buf);
    assert!(frame.into_rgb().materialize().is_err());
}
