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

// Same rationale as the exact-format pins below for the
// non-`(backend …)` / `(format …)` variants. Every test above
// in this file uses `contains(...)` checks that pass after any
// refactor preserving the keywords. Downstream log scrapers and
// integration tests key on the exact prefix and separator
// punctuation; pin each variant's full Display string verbatim
// so a wording change forces a deliberate, reviewer-visible test
// update. Guards `nokhwa-core/src/error.rs:25-74`.
#[test]
fn uninitialized_error_display_exact_format() {
    let e = NokhwaError::UninitializedError;
    assert_eq!(format!("{e}"), "Uninitialized Camera. Call `init()` first!");
}

#[test]
fn initialize_error_display_exact_format() {
    let e = NokhwaError::InitializeError {
        backend: ApiBackend::Video4Linux,
        error: "no /dev/video0".into(),
    };
    assert_eq!(
        format!("{e}"),
        "Could not initialize Video4Linux: no /dev/video0"
    );
}

#[test]
fn shutdown_error_display_exact_format() {
    let e = NokhwaError::ShutdownError {
        backend: ApiBackend::AVFoundation,
        error: "device busy".into(),
    };
    assert_eq!(
        format!("{e}"),
        "Could not shutdown AVFoundation: device busy"
    );
}

#[test]
fn structure_error_display_exact_format() {
    let e = NokhwaError::StructureError {
        structure: "FrameFormat".into(),
        error: "No match for FOOBAR".into(),
    };
    assert_eq!(
        format!("{e}"),
        "Could not generate required structure FrameFormat: No match for FOOBAR"
    );
}

#[test]
fn open_device_error_display_exact_format() {
    let e = NokhwaError::OpenDeviceError {
        device: "/dev/video2".into(),
        error: "permission denied".into(),
    };
    assert_eq!(
        format!("{e}"),
        "Could not open device /dev/video2: permission denied"
    );
}

#[test]
fn get_property_error_display_exact_format() {
    let e = NokhwaError::GetPropertyError {
        property: "Brightness".into(),
        error: "not supported".into(),
    };
    assert_eq!(
        format!("{e}"),
        "Could not get device property Brightness: not supported"
    );
}

#[test]
fn set_property_error_display_exact_format() {
    let e = NokhwaError::SetPropertyError {
        property: "Exposure".into(),
        value: "9999".into(),
        error: "out of range".into(),
    };
    assert_eq!(
        format!("{e}"),
        "Could not set device property Exposure with value 9999: out of range"
    );
}

#[test]
fn process_frame_error_display_exact_format() {
    let e = NokhwaError::ProcessFrameError {
        src: FrameFormat::YUYV,
        destination: "RGB".into(),
        error: "bad sample".into(),
    };
    assert_eq!(
        format!("{e}"),
        "Could not process frame YUYV to RGB: bad sample"
    );
}

#[test]
fn unsupported_operation_error_display_exact_format() {
    let e = NokhwaError::UnsupportedOperationError(ApiBackend::Video4Linux);
    assert_eq!(
        format!("{e}"),
        "This operation is not supported by backend Video4Linux."
    );
}

#[test]
fn not_implemented_error_display_exact_format() {
    let e = NokhwaError::NotImplementedError("hotplug on browser".into());
    assert_eq!(
        format!("{e}"),
        "This operation is not implemented yet: hotplug on browser"
    );
}

#[test]
fn timeout_error_display_exact_format() {
    let e = NokhwaError::TimeoutError(Duration::from_millis(250));
    // `{0:?}` on `Duration::from_millis(250)` Debug-formats as `250ms`.
    assert_eq!(format!("{e}"), "Frame capture timed out after 250ms");
}

// The four error variants below all share the same parenthetical
// formatting pattern: `Error<TAIL>: <msg>` where `<TAIL>` is either
// `" (backend {b})"`, `" (format {f})"`, or empty when the optional
// field is `None`. The pre-existing tests above use `contains(...)`
// checks that pass even after a refactor — e.g. flipping
// `(backend Video4Linux)` to `[backend Video4Linux]`, dropping the
// space between `backend` and the value, or replacing the prefix
// `Error` with `error`. Downstream log scrapers and integration
// tests key on the exact form, so pin each variant's full Display
// output verbatim. Guards `nokhwa-core/src/error.rs:31`, `:48`,
// `:53`, and `:64`.
#[test]
fn general_error_display_with_backend_exact_format() {
    let e = NokhwaError::GeneralError {
        message: "oops".into(),
        backend: Some(ApiBackend::Video4Linux),
    };
    assert_eq!(format!("{e}"), "Error (backend Video4Linux): oops");
}

#[test]
fn general_error_display_without_backend_exact_format() {
    let e = NokhwaError::general("oops");
    assert_eq!(format!("{e}"), "Error: oops");
}

#[test]
fn open_stream_error_display_with_backend_exact_format() {
    let e = NokhwaError::OpenStreamError {
        message: "denied".into(),
        backend: Some(ApiBackend::MediaFoundation),
    };
    assert_eq!(
        format!("{e}"),
        "Could not open device stream (backend MediaFoundation): denied"
    );
}

#[test]
fn read_frame_error_display_with_format_exact_format() {
    let e = NokhwaError::ReadFrameError {
        message: "eof".into(),
        format: Some(FrameFormat::MJPEG),
    };
    assert_eq!(
        format!("{e}"),
        "Could not capture frame (format MJPEG): eof"
    );
}

#[test]
fn stream_shutdown_error_display_with_backend_exact_format() {
    let e = NokhwaError::StreamShutdownError {
        message: "busy".into(),
        backend: Some(ApiBackend::AVFoundation),
    };
    assert_eq!(
        format!("{e}"),
        "Could not stop stream (backend AVFoundation): busy"
    );
}

// The `Some(_)` arms of the three optional-context error variants
// (`OpenStreamError`, `ReadFrameError`, `StreamShutdownError`) are
// already exact-pinned above. The matching `None` arms only have
// `contains`-style guards (`open_stream_error_display_without_backend`,
// `read_frame_error_display_without_format`,
// `stream_shutdown_error_display_without_backend`), so a refactor that
// changed the no-context branch to e.g. `" (no backend)"`,
// `"Frame capture failed:"`, or reordered the prefix tokens would
// pass the contains-checks while silently producing a different
// user-visible error. Pin the exact strings emitted when the
// optional context is absent.

#[test]
fn open_stream_error_display_without_backend_exact_format() {
    let e = NokhwaError::open_stream("denied");
    assert_eq!(format!("{e}"), "Could not open device stream: denied");
}

#[test]
fn read_frame_error_display_without_format_exact_format() {
    let e = NokhwaError::read_frame("eof");
    assert_eq!(format!("{e}"), "Could not capture frame: eof");
}

#[test]
fn stream_shutdown_error_display_without_backend_exact_format() {
    let e = NokhwaError::stream_shutdown("busy");
    assert_eq!(format!("{e}"), "Could not stop stream: busy");
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

// ───────────────────── Clone / Debug / source() ──────────────────────

#[test]
fn clone_unit_variant_round_trips_display() {
    let e = NokhwaError::UninitializedError;
    let cloned = e.clone();
    assert_eq!(format!("{e}"), format!("{cloned}"));
}

#[test]
fn clone_struct_variant_preserves_fields() {
    let e = NokhwaError::ProcessFrameError {
        src: FrameFormat::MJPEG,
        destination: "RGB".to_string(),
        error: "boom".to_string(),
    };
    let cloned = e.clone();
    if let NokhwaError::ProcessFrameError {
        src,
        destination,
        error,
    } = cloned
    {
        assert_eq!(src, FrameFormat::MJPEG);
        assert_eq!(destination, "RGB");
        assert_eq!(error, "boom");
    } else {
        panic!("clone changed variant");
    }
}

#[test]
fn clone_optional_backend_field_preserves_some() {
    let e = NokhwaError::GeneralError {
        message: "x".to_string(),
        backend: Some(ApiBackend::Video4Linux),
    };
    let cloned = e.clone();
    // Previously destructured `backend, ..` and discarded the
    // message field. A regression where `Clone` dropped or mutated
    // the message field of `GeneralError` (e.g. derived with a
    // hand-written impl that left message empty, or one that swapped
    // it with the backend's `Display` form) would have slipped past
    // the existing `Some(_)` check. Pin the message round-trip
    // alongside the optional-context field.
    if let NokhwaError::GeneralError { message, backend } = cloned {
        assert_eq!(message, "x");
        assert_eq!(backend, Some(ApiBackend::Video4Linux));
    } else {
        panic!("clone changed variant");
    }
}

// Hardened from contains-only checks. The previous three tests
// (`debug_format_includes_variant_name`,
// `debug_format_for_timeout_includes_variant_and_duration`,
// `debug_format_for_struct_variant_includes_field_names`) confirmed
// that the variant name + a few selected substrings appeared in the
// derived `Debug` output, but a regression that, say, replaced the
// derive with a hand-written `impl Debug` collapsing the struct to
// a tuple-style `OpenDeviceError("cam0", "ENOENT")`, or that
// changed `TimeoutError(2s)` to `TimeoutError { duration: 2s }`,
// would still satisfy the loose checks while breaking any log
// scraper or downstream test pinned to the exact form. Pin all
// three variants verbatim. `Duration`'s `Debug` formats whole
// seconds as `2s`, which the timeout pin captures.
#[test]
fn debug_format_unit_variant_exact_format() {
    assert_eq!(
        format!("{:?}", NokhwaError::UninitializedError),
        "UninitializedError"
    );
}

#[test]
fn debug_format_timeout_variant_exact_format() {
    assert_eq!(
        format!("{:?}", NokhwaError::TimeoutError(Duration::from_secs(2))),
        "TimeoutError(2s)"
    );
}

#[test]
fn debug_format_open_device_struct_variant_exact_format() {
    let e = NokhwaError::OpenDeviceError {
        device: "cam0".to_string(),
        error: "ENOENT".to_string(),
    };
    assert_eq!(
        format!("{e:?}"),
        "OpenDeviceError { device: \"cam0\", error: \"ENOENT\" }"
    );
}

#[test]
fn error_source_is_none_for_all_variants() {
    use std::error::Error;
    let cases: Vec<NokhwaError> = vec![
        NokhwaError::UninitializedError,
        NokhwaError::general("x"),
        NokhwaError::open_stream("y"),
        NokhwaError::read_frame("z"),
        NokhwaError::stream_shutdown("w"),
        NokhwaError::TimeoutError(Duration::from_millis(1)),
        NokhwaError::UnsupportedOperationError(ApiBackend::Browser),
        NokhwaError::NotImplementedError("nope".to_string()),
        NokhwaError::ProcessFrameError {
            src: FrameFormat::YUYV,
            destination: "RGB".to_string(),
            error: "e".to_string(),
        },
    ];
    for case in cases {
        assert!(
            case.source().is_none(),
            "expected source() == None for {case:?}"
        );
    }
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

// `helper_constructors_default_optional_context_to_none` discards
// the `message` field via `..` — it pins only the variant identity
// and the `None`-ness of the optional context. A regression in any
// of the four helpers (`general` / `open_stream` / `read_frame` /
// `stream_shutdown`, `nokhwa-core/src/error.rs:79-110`) that
// silently mutated the message — e.g. prefixed it
// (`format!("error: {}", message)`), called `.trim()`, uppercased
// it, or swapped two helpers' bodies — would slip past. Pin the
// message field as a verbatim round-trip.
#[test]
fn helper_constructors_pass_message_through_unchanged() {
    if let NokhwaError::GeneralError { message, .. } = NokhwaError::general("sentinel") {
        assert_eq!(message, "sentinel");
    } else {
        panic!("wrong variant");
    }
    if let NokhwaError::OpenStreamError { message, .. } = NokhwaError::open_stream("sentinel") {
        assert_eq!(message, "sentinel");
    } else {
        panic!("wrong variant");
    }
    if let NokhwaError::ReadFrameError { message, .. } = NokhwaError::read_frame("sentinel") {
        assert_eq!(message, "sentinel");
    } else {
        panic!("wrong variant");
    }
    if let NokhwaError::StreamShutdownError { message, .. } =
        NokhwaError::stream_shutdown("sentinel")
    {
        assert_eq!(message, "sentinel");
    } else {
        panic!("wrong variant");
    }
}
