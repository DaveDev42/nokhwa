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

#[test]
fn buffer_with_timestamp_some() {
    let res = Resolution::new(2, 2);
    let data = vec![0; 12];
    let ts = std::time::Duration::from_millis(12345);
    let buf = Buffer::with_timestamp(
        res,
        &data,
        FrameFormat::RAWRGB,
        Some((ts, TimestampKind::Capture)),
    );

    assert_eq!(buf.resolution(), res);
    assert_eq!(buf.source_frame_format(), FrameFormat::RAWRGB);
    assert_eq!(buf.buffer().len(), 12);
    assert_eq!(buf.capture_timestamp(), Some(ts));
    assert_eq!(
        buf.capture_timestamp_with_kind(),
        Some((ts, TimestampKind::Capture))
    );
}

#[test]
fn buffer_with_timestamp_none() {
    let res = Resolution::new(1, 1);
    let data = vec![0; 3];
    let buf = Buffer::with_timestamp(res, &data, FrameFormat::RAWRGB, None);
    assert!(buf.capture_timestamp().is_none());
    assert!(buf.capture_timestamp_with_kind().is_none());
}

#[test]
fn buffer_new_has_no_timestamp() {
    let res = Resolution::new(1, 1);
    let data = vec![0; 3];
    let buf = Buffer::new(res, &data, FrameFormat::RAWRGB);
    assert!(buf.capture_timestamp().is_none());
    assert!(buf.capture_timestamp_with_kind().is_none());
}

#[test]
fn buffer_with_timestamp_zero_duration() {
    let res = Resolution::new(1, 1);
    let data = vec![0; 3];
    let ts = std::time::Duration::ZERO;
    let buf = Buffer::with_timestamp(
        res,
        &data,
        FrameFormat::RAWRGB,
        Some((ts, TimestampKind::MonotonicClock)),
    );
    assert_eq!(buf.capture_timestamp(), Some(std::time::Duration::ZERO));
    assert_eq!(
        buf.capture_timestamp_with_kind(),
        Some((std::time::Duration::ZERO, TimestampKind::MonotonicClock))
    );
}

#[test]
fn buffer_large_data() {
    const FULL_HD_RGB_SIZE: usize = 1920 * 1080 * 3;
    let res = Resolution::new(1920, 1080);
    let data = vec![128u8; FULL_HD_RGB_SIZE];
    let buf = Buffer::new(res, &data, FrameFormat::RAWRGB);
    assert_eq!(buf.buffer().len(), FULL_HD_RGB_SIZE);
    assert_eq!(buf.resolution().width(), 1920);
    assert_eq!(buf.resolution().height(), 1080);
}

#[test]
fn buffer_timestamp_kind_variants() {
    let res = Resolution::new(1, 1);
    let data = vec![0; 3];
    let ts = std::time::Duration::from_secs(1);

    for kind in [
        TimestampKind::Capture,
        TimestampKind::Presentation,
        TimestampKind::MonotonicClock,
        TimestampKind::WallClock,
        TimestampKind::Unknown,
    ] {
        let buf = Buffer::with_timestamp(res, &data, FrameFormat::RAWRGB, Some((ts, kind)));
        let (returned_ts, returned_kind) = buf.capture_timestamp_with_kind().unwrap();
        assert_eq!(returned_ts, ts);
        assert_eq!(returned_kind, kind);
    }
}

// ===== Zero-copy from_vec constructor tests =====

#[test]
fn buffer_from_vec_stores_data() {
    let res = Resolution::new(2, 2);
    let data: Vec<u8> = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120];
    let expected = data.clone();
    let buf = Buffer::from_vec(res, data, FrameFormat::RAWRGB);

    assert_eq!(buf.resolution(), res);
    assert_eq!(buf.source_frame_format(), FrameFormat::RAWRGB);
    assert_eq!(buf.buffer().len(), 12);
    assert_eq!(buf.buffer(), &expected[..]);
}

#[test]
fn buffer_from_vec_has_no_timestamp() {
    let res = Resolution::new(1, 1);
    let buf = Buffer::from_vec(res, vec![0; 3], FrameFormat::RAWRGB);
    assert!(buf.capture_timestamp().is_none());
}

#[test]
fn buffer_from_vec_with_timestamp_some() {
    let res = Resolution::new(2, 2);
    let data = vec![0; 12];
    let ts = std::time::Duration::from_millis(12345);
    let buf = Buffer::from_vec_with_timestamp(
        res,
        data,
        FrameFormat::RAWRGB,
        Some((ts, TimestampKind::Capture)),
    );

    assert_eq!(buf.resolution(), res);
    assert_eq!(buf.source_frame_format(), FrameFormat::RAWRGB);
    assert_eq!(buf.buffer().len(), 12);
    assert_eq!(buf.capture_timestamp(), Some(ts));
    assert_eq!(
        buf.capture_timestamp_with_kind(),
        Some((ts, TimestampKind::Capture))
    );
}

#[test]
fn buffer_from_vec_with_timestamp_none() {
    let res = Resolution::new(1, 1);
    let buf = Buffer::from_vec_with_timestamp(res, vec![0; 3], FrameFormat::RAWRGB, None);
    assert!(buf.capture_timestamp().is_none());
    assert!(buf.capture_timestamp_with_kind().is_none());
}

#[test]
fn buffer_from_vec_empty_data() {
    let res = Resolution::new(0, 0);
    let buf = Buffer::from_vec(res, vec![], FrameFormat::MJPEG);
    assert_eq!(buf.buffer().len(), 0);
    assert_eq!(buf.resolution(), Resolution::new(0, 0));
}

#[test]
fn buffer_from_vec_empty_data_nonzero_resolution() {
    let res = Resolution::new(2, 2);
    let buf = Buffer::from_vec(res, vec![], FrameFormat::RAWRGB);
    assert_eq!(buf.buffer().len(), 0);
    assert_eq!(buf.resolution(), Resolution::new(2, 2));
}

#[test]
fn buffer_from_vec_equivalent_to_new() {
    let res = Resolution::new(2, 2);
    let data: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let buf_copy = Buffer::new(res, &data, FrameFormat::RAWRGB);
    let buf_zero = Buffer::from_vec(res, data, FrameFormat::RAWRGB);

    assert_eq!(buf_copy.buffer(), buf_zero.buffer());
    assert_eq!(buf_copy.resolution(), buf_zero.resolution());
    assert_eq!(
        buf_copy.source_frame_format(),
        buf_zero.source_frame_format()
    );
    assert_eq!(buf_copy.capture_timestamp(), buf_zero.capture_timestamp());
}

// ===== Format conversion correctness tests =====

#[test]
fn decode_rawrgb_to_rgb_identity() {
    // A 2x2 image with known pixel values: red, green, blue, white
    let data: Vec<u8> = vec![
        255, 0, 0, // red
        0, 255, 0, // green
        0, 0, 255, // blue
        255, 255, 255, // white
    ];
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::RAWRGB);
    let img = buf
        .decode_image::<crate::pixel_format::RgbFormat>()
        .expect("RAWRGB -> RgbFormat should succeed");
    assert_eq!(img.width(), 2);
    assert_eq!(img.height(), 2);
    // RAWRGB -> RGB is identity; output should match input exactly
    assert_eq!(img.into_raw(), data);
}

#[test]
fn decode_rawrgb_to_luma_averages_channels() {
    // Single white pixel: avg(255,255,255) = 255
    let white = vec![255u8, 255, 255];
    let buf = Buffer::new(Resolution::new(1, 1), &white, FrameFormat::RAWRGB);
    let img = buf
        .decode_image::<crate::pixel_format::LumaFormat>()
        .expect("RAWRGB -> LumaFormat should succeed");
    assert_eq!(img.into_raw(), vec![255u8]);

    // Single pixel (30,60,90): avg = (30+60+90)/3 = 60
    let pixel = vec![30u8, 60, 90];
    let buf = Buffer::new(Resolution::new(1, 1), &pixel, FrameFormat::RAWRGB);
    let img = buf
        .decode_image::<crate::pixel_format::LumaFormat>()
        .expect("RAWRGB -> LumaFormat should succeed");
    assert_eq!(img.into_raw(), vec![60u8]);
}

#[test]
fn decode_gray_to_rgb_triplicates() {
    // Gray pixel value 128 should become (128, 128, 128) in RGB
    let data = vec![128u8, 64];
    let buf = Buffer::new(Resolution::new(2, 1), &data, FrameFormat::GRAY);
    let img = buf
        .decode_image::<crate::pixel_format::RgbFormat>()
        .expect("GRAY -> RgbFormat should succeed");
    assert_eq!(img.into_raw(), vec![128, 128, 128, 64, 64, 64]);
}

#[test]
fn decode_gray_to_luma_identity() {
    let data = vec![0u8, 128, 255, 42];
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::GRAY);
    let img = buf
        .decode_image::<crate::pixel_format::LumaFormat>()
        .expect("GRAY -> LumaFormat should succeed");
    assert_eq!(img.into_raw(), data);
}

#[test]
fn decode_gray_to_rgba_adds_alpha() {
    let data = vec![100u8];
    let buf = Buffer::new(Resolution::new(1, 1), &data, FrameFormat::GRAY);
    let img = buf
        .decode_image::<crate::pixel_format::RgbAFormat>()
        .expect("GRAY -> RgbAFormat should succeed");
    assert_eq!(img.into_raw(), vec![100, 100, 100, 255]);
}

#[test]
fn decode_gray_to_luma_a_adds_alpha() {
    let data = vec![200u8, 50];
    let buf = Buffer::new(Resolution::new(2, 1), &data, FrameFormat::GRAY);
    let img = buf
        .decode_image::<crate::pixel_format::LumaAFormat>()
        .expect("GRAY -> LumaAFormat should succeed");
    assert_eq!(img.into_raw(), vec![200, 255, 50, 255]);
}

#[test]
fn decode_rawbgr_to_rgb_swaps_channels() {
    // BGR (10, 20, 30) should become RGB (30, 20, 10)
    let bgr = vec![10u8, 20, 30, 40, 50, 60];
    let buf = Buffer::new(Resolution::new(2, 1), &bgr, FrameFormat::RAWBGR);
    let img = buf
        .decode_image::<crate::pixel_format::RgbFormat>()
        .expect("RAWBGR -> RgbFormat should succeed");
    assert_eq!(img.into_raw(), vec![30, 20, 10, 60, 50, 40]);
}

#[test]
fn decode_rawbgr_to_rgba_swaps_and_adds_alpha() {
    let bgr = vec![10u8, 20, 30];
    let buf = Buffer::new(Resolution::new(1, 1), &bgr, FrameFormat::RAWBGR);
    let img = buf
        .decode_image::<crate::pixel_format::RgbAFormat>()
        .expect("RAWBGR -> RgbAFormat should succeed");
    assert_eq!(img.into_raw(), vec![30, 20, 10, 255]);
}

#[test]
fn decode_rawrgb_to_rgba_adds_alpha() {
    let rgb = vec![11u8, 22, 33];
    let buf = Buffer::new(Resolution::new(1, 1), &rgb, FrameFormat::RAWRGB);
    let img = buf
        .decode_image::<crate::pixel_format::RgbAFormat>()
        .expect("RAWRGB -> RgbAFormat should succeed");
    assert_eq!(img.into_raw(), vec![11, 22, 33, 255]);
}

#[test]
fn decode_yuyv_to_rgb_known_values() {
    // YUYV: 2 pixels packed as [Y0, U, Y1, V].
    // Use Y=128, U=128, V=128 which should produce neutral gray in RGB.
    // YUV (128,128,128) -> R = clamp(128 + 1.370705*(128-128)) = 128
    //                       G = clamp(128 - 0.698001*(128-128) - 0.337633*(128-128)) = 128
    //                       B = clamp(128 + 1.732446*(128-128)) = 128
    let yuyv = vec![128u8, 128, 128, 128]; // 2 pixels
    let buf = Buffer::new(Resolution::new(2, 1), &yuyv, FrameFormat::YUYV);
    let img = buf
        .decode_image::<crate::pixel_format::RgbFormat>()
        .expect("YUYV -> RgbFormat should succeed");
    let raw = img.into_raw();
    assert_eq!(raw.len(), 6); // 2 pixels * 3 channels
                              // Both pixels should be approximately (128, 128, 128)
    for px in raw.chunks_exact(3) {
        for &channel in px {
            assert!(
                (120..=136).contains(&channel),
                "Expected ~128 but got {channel}"
            );
        }
    }
}

#[test]
fn decode_nv12_to_rgb_known_values() {
    // NV12: Y plane followed by interleaved UV plane.
    // For a 2x2 image: 4 Y bytes + 2 UV bytes = 6 bytes total.
    // Y=128, U=128, V=128 -> neutral gray
    let nv12 = vec![
        128, 128, 128, 128, // Y plane (2x2)
        128, 128, // UV plane (1 pair for 2x2 block)
    ];
    let buf = Buffer::new(Resolution::new(2, 2), &nv12, FrameFormat::NV12);
    let img = buf
        .decode_image::<crate::pixel_format::RgbFormat>()
        .expect("NV12 -> RgbFormat should succeed");
    let raw = img.into_raw();
    assert_eq!(raw.len(), 12); // 4 pixels * 3 channels
    for px in raw.chunks_exact(3) {
        for &channel in px {
            assert!(
                (120..=136).contains(&channel),
                "Expected ~128 but got {channel}"
            );
        }
    }
}

#[test]
fn decode_yuyv_to_luma_correctness() {
    // YUYV [Y0=100, U=128, Y1=200, V=128] -> 2 pixels
    // U=V=128 (neutral chroma), so RGB ≈ (Y, Y, Y) and luma ≈ Y
    let yuyv = vec![100u8, 128, 200, 128];
    let buf = Buffer::new(Resolution::new(2, 1), &yuyv, FrameFormat::YUYV);
    let img = buf
        .decode_image::<crate::pixel_format::LumaFormat>()
        .expect("YUYV -> LumaFormat should succeed");
    assert_eq!(img.width(), 2);
    assert_eq!(img.height(), 1);
    let raw = img.into_raw();
    assert_eq!(raw.len(), 2);
    // With neutral chroma, luma should be close to Y values.
    // The YUV->RGB->avg pipeline introduces rounding, so allow ±20 tolerance.
    assert!(
        (80..=120).contains(&raw[0]),
        "First pixel luma expected ~100, got {}",
        raw[0]
    );
    assert!(
        (180..=220).contains(&raw[1]),
        "Second pixel luma expected ~200, got {}",
        raw[1]
    );
}

#[test]
fn decode_nv12_to_luma_correctness() {
    // NV12: Y plane [50, 100, 150, 200], UV plane [128, 128] (neutral chroma)
    // With neutral chroma, luma should be close to original Y values
    let nv12 = vec![50, 100, 150, 200, 128, 128];
    let buf = Buffer::new(Resolution::new(2, 2), &nv12, FrameFormat::NV12);
    let img = buf
        .decode_image::<crate::pixel_format::LumaFormat>()
        .expect("NV12 -> LumaFormat should succeed");
    assert_eq!(img.width(), 2);
    assert_eq!(img.height(), 2);
    let raw = img.into_raw();
    assert_eq!(raw.len(), 4);
    let expected_y = [50u8, 100, 150, 200];
    // The NV12->RGB->avg pipeline introduces rounding, so allow ±20 tolerance.
    for (i, (&actual, &expected)) in raw.iter().zip(expected_y.iter()).enumerate() {
        assert!(
            (actual as i16 - expected as i16).unsigned_abs() <= 20,
            "Pixel {i}: luma expected ~{expected}, got {actual}"
        );
    }
}

// ===== write_output_buffer tests =====

#[test]
fn decode_rawrgb_to_buffer_identity() {
    let data = vec![10u8, 20, 30, 40, 50, 60];
    let buf = Buffer::new(Resolution::new(2, 1), &data, FrameFormat::RAWRGB);
    let mut dest = vec![0u8; 6];
    buf.decode_image_to_buffer::<crate::pixel_format::RgbFormat>(&mut dest)
        .expect("write_output_buffer RAWRGB -> RGB should succeed");
    assert_eq!(dest, data);
}

#[test]
fn decode_gray_to_rgb_buffer() {
    let data = vec![128u8];
    let buf = Buffer::new(Resolution::new(1, 1), &data, FrameFormat::GRAY);
    let mut dest = vec![0u8; 3];
    buf.decode_image_to_buffer::<crate::pixel_format::RgbFormat>(&mut dest)
        .expect("write_output_buffer GRAY -> RGB should succeed");
    assert_eq!(dest, vec![128, 128, 128]);
}

#[test]
fn decode_gray_to_luma_buffer() {
    let data = vec![42u8, 99];
    let buf = Buffer::new(Resolution::new(2, 1), &data, FrameFormat::GRAY);
    let mut dest = vec![0u8; 2];
    buf.decode_image_to_buffer::<crate::pixel_format::LumaFormat>(&mut dest)
        .expect("write_output_buffer GRAY -> Luma should succeed");
    assert_eq!(dest, vec![42, 99]);
}

#[test]
fn decode_yuyv_to_rgb_buffer() {
    // YUYV neutral gray: Y=128, U=128, V=128 -> RGB ~(128, 128, 128) per pixel
    let yuyv = vec![128u8, 128, 128, 128]; // 2 pixels
    let buf = Buffer::new(Resolution::new(2, 1), &yuyv, FrameFormat::YUYV);
    let mut dest = vec![0u8; 6]; // 2 pixels * 3 channels
    buf.decode_image_to_buffer::<crate::pixel_format::RgbFormat>(&mut dest)
        .expect("write_output_buffer YUYV -> RGB should succeed");
    for (i, &channel) in dest.iter().enumerate() {
        assert!(
            (120..=136).contains(&channel),
            "Channel {i}: expected ~128 but got {channel}"
        );
    }
}

// ===== Robustness tests: malformed data =====

#[test]
fn decode_mjpeg_empty_data_returns_error() {
    let buf = Buffer::new(Resolution::new(1, 1), &[], FrameFormat::MJPEG);
    let result = buf.decode_image::<crate::pixel_format::RgbFormat>();
    assert!(result.is_err(), "Empty MJPEG data should return error");
}

#[test]
fn decode_mjpeg_truncated_data_returns_error() {
    // Random bytes that aren't valid JPEG
    let garbage = vec![0xFFu8, 0xD8, 0xFF, 0xE0, 0x00]; // truncated JPEG header
    let buf = Buffer::new(Resolution::new(1, 1), &garbage, FrameFormat::MJPEG);
    let result = buf.decode_image::<crate::pixel_format::RgbFormat>();
    assert!(
        result.is_err(),
        "Truncated MJPEG data should return error, not panic"
    );
}

#[test]
fn decode_mjpeg_random_garbage_returns_error() {
    let garbage = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE];
    let buf = Buffer::new(Resolution::new(2, 2), &garbage, FrameFormat::MJPEG);
    let result = buf.decode_image::<crate::pixel_format::RgbFormat>();
    assert!(result.is_err(), "Random garbage as MJPEG should error");
}

#[test]
fn decode_yuyv_odd_length_data_returns_error() {
    // YUYV data length must be divisible by 4
    let bad_data = vec![128u8; 5];
    let buf = Buffer::new(Resolution::new(2, 1), &bad_data, FrameFormat::YUYV);
    let result = buf.decode_image::<crate::pixel_format::RgbFormat>();
    assert!(
        result.is_err(),
        "YUYV with non-multiple-of-4 length should error"
    );
}

#[test]
fn decode_nv12_wrong_buffer_size_returns_error() {
    // NV12 for 2x2 needs exactly 6 bytes; give it 4
    let bad_data = vec![128u8; 4];
    let buf = Buffer::new(Resolution::new(2, 2), &bad_data, FrameFormat::NV12);
    let result = buf.decode_image::<crate::pixel_format::RgbFormat>();
    assert!(result.is_err(), "NV12 with wrong buffer size should error");
}

#[test]
fn decode_mismatched_resolution_rawrgb_returns_error() {
    // 2x2 needs 12 bytes of RGB but we only provide 3
    let buf = Buffer::new(Resolution::new(2, 2), &[1, 2, 3], FrameFormat::RAWRGB);
    let result = buf.decode_image::<crate::pixel_format::RgbFormat>();
    assert!(
        result.is_err(),
        "Mismatched resolution/data size should error"
    );
}

#[test]
fn decode_gray_wrong_dest_buffer_size_returns_error() {
    let data = vec![128u8; 4]; // 4 gray pixels
    let buf = Buffer::new(Resolution::new(2, 2), &data, FrameFormat::GRAY);
    // Need 12 bytes for RGB output (4 pixels * 3), but give 10
    let mut dest = vec![0u8; 10];
    let result = buf.decode_image_to_buffer::<crate::pixel_format::RgbFormat>(&mut dest);
    assert!(
        result.is_err(),
        "Wrong destination buffer size should error"
    );
}

#[test]
fn decode_yuyv_wrong_dest_buffer_size_returns_error() {
    let yuyv = vec![128u8, 128, 128, 128]; // 2 pixels of YUYV
    let buf = Buffer::new(Resolution::new(2, 1), &yuyv, FrameFormat::YUYV);
    // Correct dest size is 6 (2 pixels * 3 RGB channels); give 4 instead
    let mut dest = vec![0u8; 4];
    let result = buf.decode_image_to_buffer::<crate::pixel_format::RgbFormat>(&mut dest);
    assert!(
        result.is_err(),
        "YUYV with wrong destination buffer size should error"
    );
}

#[test]
fn decode_mjpeg_empty_to_luma_returns_error() {
    let buf = Buffer::new(Resolution::new(1, 1), &[], FrameFormat::MJPEG);
    let result = buf.decode_image::<crate::pixel_format::LumaFormat>();
    assert!(result.is_err(), "Empty MJPEG -> Luma should return error");
}

#[test]
fn decode_mjpeg_empty_to_rgba_returns_error() {
    let buf = Buffer::new(Resolution::new(1, 1), &[], FrameFormat::MJPEG);
    let result = buf.decode_image::<crate::pixel_format::RgbAFormat>();
    assert!(result.is_err(), "Empty MJPEG -> RGBA should return error");
}

#[test]
fn decode_mjpeg_empty_to_luma_a_returns_error() {
    let buf = Buffer::new(Resolution::new(1, 1), &[], FrameFormat::MJPEG);
    let result = buf.decode_image::<crate::pixel_format::LumaAFormat>();
    assert!(result.is_err(), "Empty MJPEG -> LumaA should return error");
}

#[test]
fn decode_nv12_odd_resolution_returns_error() {
    // NV12 requires even width and height
    let data = vec![128u8; 6];
    let buf = Buffer::new(Resolution::new(3, 1), &data, FrameFormat::NV12);
    let result = buf.decode_image::<crate::pixel_format::RgbFormat>();
    assert!(result.is_err(), "NV12 with odd resolution should error");
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("bad resolution"),
        "Expected 'bad resolution' in error, got: {err}"
    );
}

#[test]
fn decode_yuyv_zero_length_does_not_panic() {
    let buf = Buffer::new(Resolution::new(0, 0), &[], FrameFormat::YUYV);
    // Zero-length YUYV with 0x0 resolution: YUYV produces empty output,
    // ImageBuffer::from_raw(0, 0, vec![]) succeeds
    let result = buf.decode_image::<crate::pixel_format::RgbFormat>();
    assert!(result.is_ok(), "0x0 YUYV decode should succeed");
}

#[test]
fn decode_oversized_buffer_rawrgb() {
    // 1x1 needs 3 bytes, but we provide 100 bytes
    let data = vec![42u8; 100];
    let buf = Buffer::new(Resolution::new(1, 1), &data, FrameFormat::RAWRGB);
    // RAWRGB passthrough returns all data; ImageBuffer::from_raw only requires
    // len >= width*height*channels, so surplus data is accepted
    let result = buf.decode_image::<crate::pixel_format::RgbFormat>();
    assert!(
        result.is_ok(),
        "Oversized RAWRGB buffer should succeed (surplus data ignored)"
    );
}

// ===== Cross-format coverage: RAWRGB/RAWBGR to LumaA, RAWBGR to Luma =====

#[test]
fn decode_rawrgb_to_luma_a_returns_error() {
    // LumaAFormat does not support RAWRGB input
    let data = vec![100u8, 150, 200];
    let buf = Buffer::new(Resolution::new(1, 1), &data, FrameFormat::RAWRGB);
    let result = buf.decode_image::<crate::pixel_format::LumaAFormat>();
    assert!(
        result.is_err(),
        "RAWRGB -> LumaA is unsupported and should error"
    );
}

#[test]
fn decode_rawbgr_to_luma_averages_channels() {
    // BGR (10, 20, 30) -> Luma avg = (10 + 20 + 30) / 3 = 20
    // Values intentionally chosen so channel order doesn't affect the average
    let bgr = vec![10u8, 20, 30];
    let buf = Buffer::new(Resolution::new(1, 1), &bgr, FrameFormat::RAWBGR);
    let img = buf
        .decode_image::<crate::pixel_format::LumaFormat>()
        .expect("RAWBGR -> LumaFormat should succeed");
    assert_eq!(img.into_raw(), vec![20u8]);
}

#[test]
fn decode_rawbgr_to_luma_a_returns_error() {
    // LumaAFormat does not support RAWBGR input
    let data = vec![10u8, 20, 30];
    let buf = Buffer::new(Resolution::new(1, 1), &data, FrameFormat::RAWBGR);
    let result = buf.decode_image::<crate::pixel_format::LumaAFormat>();
    assert!(
        result.is_err(),
        "RAWBGR -> LumaA is unsupported and should error"
    );
}

#[test]
fn decode_nv12_to_rgba_known_values() {
    // NV12 neutral gray (Y=128, U=128, V=128) -> RGBA ~(128, 128, 128, 255)
    let nv12 = vec![128, 128, 128, 128, 128, 128];
    let buf = Buffer::new(Resolution::new(2, 2), &nv12, FrameFormat::NV12);
    let img = buf
        .decode_image::<crate::pixel_format::RgbAFormat>()
        .expect("NV12 -> RgbAFormat should succeed");
    let raw = img.into_raw();
    assert_eq!(raw.len(), 16); // 4 pixels * 4 channels
    for px in raw.chunks_exact(4) {
        for &channel in &px[..3] {
            assert!(
                (120..=136).contains(&channel),
                "Expected ~128 but got {channel}"
            );
        }
        assert_eq!(px[3], 255, "Alpha channel should be 255");
    }
}

#[test]
fn buf_nv12_to_rgb_4x2_known_values() {
    use crate::types::buf_nv12_to_rgb;
    // 4x2 NV12 frame: Y plane = 8 bytes, UV plane = 4 bytes (one UV row, 2 pairs)
    // Y values vary; U=128, V=128 (neutral chroma) -> RGB ≈ (Y, Y, Y)
    let nv12: Vec<u8> = vec![
        // Y plane (4x2)
        16, 82, 170, 235, // row 0
        50, 100, 150, 200, // row 1
        // UV plane (2 pairs for 4x2 block)
        128, 128, 128, 128,
    ];
    let mut out = vec![0u8; 4 * 2 * 3]; // 8 pixels * RGB
    buf_nv12_to_rgb(Resolution::new(4, 2), &nv12, &mut out, false).unwrap();

    // With neutral chroma (U=V=128), each pixel's RGB channels should be close to each other
    // and roughly correspond to the Y value (after YUV->RGB conversion)
    for px in out.chunks_exact(3) {
        let spread = px.iter().max().unwrap() - px.iter().min().unwrap();
        assert!(
            spread <= 2,
            "Neutral chroma should give near-gray, got {px:?}"
        );
    }
    assert_eq!(out.len(), 24);

    // Spot-check boundary Y values: Y=16 maps to ~0 (clamped), Y=235 maps to ~255
    let px0_avg = (u16::from(out[0]) + u16::from(out[1]) + u16::from(out[2])) / 3;
    assert!(px0_avg <= 5, "Y=16 should map near 0, got avg {px0_avg}");
    let px3_avg = (u16::from(out[9]) + u16::from(out[10]) + u16::from(out[11])) / 3;
    assert!(
        px3_avg >= 250,
        "Y=235 should map near 255, got avg {px3_avg}"
    );
}

#[test]
fn buf_nv12_to_rgba_4x2_known_values() {
    use crate::types::buf_nv12_to_rgb;
    let nv12: Vec<u8> = vec![
        128, 128, 128, 128, // Y plane (4x2)
        128, 128, 128, 128, // row 1
        128, 128, 128, 128, // UV plane
    ];
    let mut out = vec![0u8; 4 * 2 * 4]; // 8 pixels * RGBA
    buf_nv12_to_rgb(Resolution::new(4, 2), &nv12, &mut out, true).unwrap();

    for px in out.chunks_exact(4) {
        for &ch in &px[..3] {
            assert!((120..=136).contains(&ch), "Expected ~128 but got {ch}");
        }
        assert_eq!(px[3], 255, "Alpha should be 255");
    }
}

#[test]
fn decode_yuyv_to_luma_a_correctness() {
    // YUYV [Y0=100, U=128, Y1=200, V=128] -> 2 LumaA pixels
    // With neutral chroma, luma ≈ Y, alpha = 255
    let yuyv = vec![100u8, 128, 200, 128];
    let buf = Buffer::new(Resolution::new(2, 1), &yuyv, FrameFormat::YUYV);
    let img = buf
        .decode_image::<crate::pixel_format::LumaAFormat>()
        .expect("YUYV -> LumaAFormat should succeed");
    assert_eq!(img.width(), 2);
    assert_eq!(img.height(), 1);
    let raw = img.into_raw();
    assert_eq!(raw.len(), 4); // 2 pixels * 2 channels (luma + alpha)
                              // Check alpha channels are 255
    assert_eq!(raw[1], 255, "First pixel alpha should be 255");
    assert_eq!(raw[3], 255, "Second pixel alpha should be 255");
    // Check luma values are close to Y inputs (±20 for YUV->RGB->avg rounding)
    assert!(
        (80..=120).contains(&raw[0]),
        "First pixel luma expected ~100, got {}",
        raw[0]
    );
    assert!(
        (180..=220).contains(&raw[2]),
        "Second pixel luma expected ~200, got {}",
        raw[2]
    );
}
