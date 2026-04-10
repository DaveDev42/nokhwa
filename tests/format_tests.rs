//! Non-device integration tests for the root nokhwa crate.
//! These verify that public API re-exports are accessible from outside the crate.

use nokhwa::utils::*;

#[test]
fn requested_format_creation_with_formats() {
    let req = RequestedFormat::with_formats(
        RequestedFormatType::AbsoluteHighestResolution,
        &[FrameFormat::MJPEG, FrameFormat::YUYV],
    );
    assert_eq!(
        req.requested_format_type(),
        RequestedFormatType::AbsoluteHighestResolution
    );
}

#[test]
fn camera_index_from_u32() {
    let idx = CameraIndex::Index(3);
    assert!(idx.is_index());
    assert_eq!(idx.as_index().unwrap(), 3);
    assert!(!idx.is_string());
}

#[test]
fn camera_index_from_string() {
    let idx = CameraIndex::String("/dev/video0".to_string());
    assert!(idx.is_string());
    assert_eq!(idx.as_string(), "/dev/video0");
    assert!(!idx.is_index());
    assert!(idx.as_index().is_err());
}

#[test]
fn camera_index_default() {
    let idx = CameraIndex::default();
    assert_eq!(idx, CameraIndex::Index(0));
}

#[test]
fn camera_format_default_values() {
    let fmt = CameraFormat::default();
    assert_eq!(fmt.resolution(), Resolution::new(640, 480));
    assert_eq!(fmt.format(), FrameFormat::MJPEG);
    assert_eq!(fmt.frame_rate(), 30);
}

#[test]
fn resolution_display_contains_dimensions() {
    let res = Resolution::new(1920, 1080);
    let s = format!("{res}");
    assert!(s.contains("1920"));
    assert!(s.contains("1080"));
}

#[test]
fn camera_info_construction() {
    let info = CameraInfo::new(
        "Test Camera",
        "A test camera",
        "misc",
        CameraIndex::Index(0),
    );
    assert_eq!(info.human_name(), "Test Camera");
    assert_eq!(info.description(), "A test camera");
    assert_eq!(info.misc(), "misc");
    assert_eq!(info.index(), &CameraIndex::Index(0));
}
