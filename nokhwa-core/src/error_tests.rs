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
fn uninitialized_error_display_mentions_init() {
    let e = NokhwaError::UninitializedError;
    let s = format!("{e}");
    assert!(s.contains("Uninitialized"));
    assert!(s.contains("init()"));
}

#[test]
fn initialize_error_display_includes_backend_and_error() {
    let e = NokhwaError::InitializeError {
        backend: ApiBackend::Video4Linux,
        error: "no /dev/video0".into(),
    };
    let s = format!("{e}");
    assert!(s.contains("Video4Linux"));
    assert!(s.contains("no /dev/video0"));
}

#[test]
fn shutdown_error_display_includes_backend_and_error() {
    let e = NokhwaError::ShutdownError {
        backend: ApiBackend::AVFoundation,
        error: "device busy".into(),
    };
    let s = format!("{e}");
    assert!(s.contains("AVFoundation"));
    assert!(s.contains("device busy"));
}

#[test]
fn structure_error_display_includes_structure_and_error() {
    let e = NokhwaError::StructureError {
        structure: "FrameFormat".into(),
        error: "No match for FOOBAR".into(),
    };
    let s = format!("{e}");
    assert!(s.contains("FrameFormat"));
    assert!(s.contains("No match for FOOBAR"));
}

#[test]
fn open_device_error_display_includes_device_and_error() {
    let e = NokhwaError::OpenDeviceError {
        device: "/dev/video2".into(),
        error: "permission denied".into(),
    };
    let s = format!("{e}");
    assert!(s.contains("/dev/video2"));
    assert!(s.contains("permission denied"));
}

#[test]
fn get_property_error_display_includes_property_and_error() {
    let e = NokhwaError::GetPropertyError {
        property: "Brightness".into(),
        error: "not supported".into(),
    };
    let s = format!("{e}");
    assert!(s.contains("Brightness"));
    assert!(s.contains("not supported"));
}

#[test]
fn set_property_error_display_includes_property_value_and_error() {
    let e = NokhwaError::SetPropertyError {
        property: "Exposure".into(),
        value: "9999".into(),
        error: "out of range".into(),
    };
    let s = format!("{e}");
    assert!(s.contains("Exposure"));
    assert!(s.contains("9999"));
    assert!(s.contains("out of range"));
}

#[test]
fn not_implemented_error_display_includes_message() {
    let e = NokhwaError::NotImplementedError("hotplug on browser".into());
    let s = format!("{e}");
    assert!(s.contains("not implemented"));
    assert!(s.contains("hotplug on browser"));
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
