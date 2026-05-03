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
    // Pin the derived structural `PartialEq` directly: the two
    // constructors must produce buffers that compare equal as a
    // whole, not just field-by-field. A future hand-written
    // `impl PartialEq for Buffer` that, say, ignored
    // `capture_timestamp` (rationale: "two buffers with the same
    // data are 'the same'") would still pass the field-by-field
    // checks above (both timestamps are `None` in this test) but
    // would diverge from the field-by-field contract on a
    // timestamped buffer — break the test by forcing the whole
    // struct comparison.
    assert_eq!(buf_copy, buf_zero);
}

// `Buffer` derives `PartialEq` (`nokhwa-core/src/buffer.rs:48`),
// which is structural over all four fields (resolution, data,
// source_frame_format, capture_timestamp). No test currently
// exercises the timestamp's contribution to equality: two buffers
// with identical data + format + resolution but different
// timestamps must be `!=`. A regression introducing a hand-written
// `impl PartialEq` that dropped the timestamp from the comparison
// (a tempting "two buffers carrying the same pixels are equal"
// rule) would silently make timestamped frames look like
// duplicates, breaking any deduplication or frame-cache logic that
// relies on timestamp-distinct buffers comparing unequal.
#[test]
fn buffer_partial_eq_distinguishes_capture_timestamp() {
    let res = Resolution::new(2, 2);
    let data = vec![0u8; 12];
    let ts_a = std::time::Duration::from_millis(100);
    let ts_b = std::time::Duration::from_millis(200);

    let buf_no_ts = Buffer::new(res, &data, FrameFormat::RAWRGB);
    let buf_ts_a = Buffer::with_timestamp(
        res,
        &data,
        FrameFormat::RAWRGB,
        Some((ts_a, TimestampKind::Capture)),
    );
    let buf_ts_b = Buffer::with_timestamp(
        res,
        &data,
        FrameFormat::RAWRGB,
        Some((ts_b, TimestampKind::Capture)),
    );
    let buf_ts_a_dup = Buffer::with_timestamp(
        res,
        &data,
        FrameFormat::RAWRGB,
        Some((ts_a, TimestampKind::Capture)),
    );

    // Different timestamps → unequal.
    assert_ne!(buf_ts_a, buf_ts_b);
    // None vs Some(_) → unequal.
    assert_ne!(buf_no_ts, buf_ts_a);
    // Same timestamp + same kind → equal.
    assert_eq!(buf_ts_a, buf_ts_a_dup);
}

// Companion to the above: the `TimestampKind` is also part of the
// derived `PartialEq` (it's tupled with `Duration` in
// `capture_timestamp: Option<(Duration, TimestampKind)>`). Two
// buffers with the same `Duration` but different `TimestampKind`
// values must compare unequal. A regression that flattened the
// tuple to drop the kind discriminator would silently treat a
// `Capture` timestamp and a `Presentation` timestamp at the same
// monotonic value as equivalent, despite their different clock
// semantics.
#[test]
fn buffer_partial_eq_distinguishes_timestamp_kind() {
    let res = Resolution::new(2, 2);
    let data = vec![0u8; 12];
    let ts = std::time::Duration::from_millis(100);

    let buf_capture = Buffer::with_timestamp(
        res,
        &data,
        FrameFormat::RAWRGB,
        Some((ts, TimestampKind::Capture)),
    );
    let buf_presentation = Buffer::with_timestamp(
        res,
        &data,
        FrameFormat::RAWRGB,
        Some((ts, TimestampKind::Presentation)),
    );
    assert_ne!(buf_capture, buf_presentation);
}
