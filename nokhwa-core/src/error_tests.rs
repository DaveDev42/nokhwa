use super::*;
use crate::types::{ApiBackend, FrameFormat};
use std::time::Duration;

#[test]
fn general_error_display_without_backend() {
    let e = NokhwaError::general("oops");
    let s = format!("{e}");
    assert!(s.starts_with("Error:"), "got: {s}");
    assert!(s.contains("oops"));
    assert!(!s.contains("backend"));
}

#[test]
fn general_error_display_with_backend() {
    let e = NokhwaError::GeneralError {
        message: "oops".into(),
        backend: Some(ApiBackend::Video4Linux),
    };
    let s = format!("{e}");
    assert!(s.contains("backend"));
    assert!(s.contains("Video4Linux"));
    assert!(s.contains("oops"));
}

#[test]
fn open_stream_error_display_without_backend() {
    let e = NokhwaError::open_stream("denied");
    let s = format!("{e}");
    assert!(s.contains("denied"));
    assert!(!s.contains("backend"));
}

#[test]
fn open_stream_error_display_with_backend() {
    let e = NokhwaError::OpenStreamError {
        message: "denied".into(),
        backend: Some(ApiBackend::MediaFoundation),
    };
    let s = format!("{e}");
    assert!(s.contains("denied"));
    assert!(s.contains("MediaFoundation"));
}

#[test]
fn read_frame_error_display_without_format() {
    let e = NokhwaError::read_frame("eof");
    let s = format!("{e}");
    assert!(s.contains("eof"));
    assert!(!s.contains("format"));
}

#[test]
fn read_frame_error_display_with_format() {
    let e = NokhwaError::ReadFrameError {
        message: "eof".into(),
        format: Some(FrameFormat::MJPEG),
    };
    let s = format!("{e}");
    assert!(s.contains("eof"));
    assert!(s.contains("MJPEG"));
    assert!(s.contains("format"));
}

#[test]
fn stream_shutdown_error_display_without_backend() {
    let e = NokhwaError::stream_shutdown("busy");
    let s = format!("{e}");
    assert!(s.contains("busy"));
    assert!(!s.contains("backend"));
}

#[test]
fn stream_shutdown_error_display_with_backend() {
    let e = NokhwaError::StreamShutdownError {
        message: "busy".into(),
        backend: Some(ApiBackend::AVFoundation),
    };
    let s = format!("{e}");
    assert!(s.contains("busy"));
    assert!(s.contains("AVFoundation"));
}

#[test]
fn timeout_error_display_includes_duration() {
    let e = NokhwaError::TimeoutError(Duration::from_millis(250));
    let s = format!("{e}");
    assert!(s.contains("timed out"));
    assert!(s.contains("250"));
}

#[test]
fn unsupported_operation_error_display_includes_backend() {
    let e = NokhwaError::UnsupportedOperationError(ApiBackend::Video4Linux);
    let s = format!("{e}");
    assert!(s.contains("not supported"));
    assert!(s.contains("Video4Linux"));
}

#[test]
fn process_frame_error_display_includes_src_and_destination() {
    let e = NokhwaError::ProcessFrameError {
        src: FrameFormat::YUYV,
        destination: "RGB".into(),
        error: "bad sample".into(),
    };
    let s = format!("{e}");
    assert!(s.contains("YUYV"));
    assert!(s.contains("RGB"));
    assert!(s.contains("bad sample"));
}

#[test]
fn helper_constructors_default_optional_context_to_none() {
    if let NokhwaError::GeneralError { backend, .. } = NokhwaError::general("x") {
        assert!(backend.is_none());
    } else {
        panic!("wrong variant");
    }
    if let NokhwaError::OpenStreamError { backend, .. } = NokhwaError::open_stream("x") {
        assert!(backend.is_none());
    } else {
        panic!("wrong variant");
    }
    if let NokhwaError::ReadFrameError { format, .. } = NokhwaError::read_frame("x") {
        assert!(format.is_none());
    } else {
        panic!("wrong variant");
    }
    if let NokhwaError::StreamShutdownError { backend, .. } = NokhwaError::stream_shutdown("x") {
        assert!(backend.is_none());
    } else {
        panic!("wrong variant");
    }
}
