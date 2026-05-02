use super::*;

#[test]
fn resolution_new() {
    let res = Resolution::new(1920, 1080);
    assert_eq!(res.width(), 1920);
    assert_eq!(res.height(), 1080);
    assert_eq!(res.x(), 1920);
    assert_eq!(res.y(), 1080);
    assert_eq!(res.width_x, 1920);
    assert_eq!(res.height_y, 1080);
}

#[test]
fn resolution_display() {
    let res = Resolution::new(640, 480);
    let display = format!("{res}");
    assert!(display.contains("640"));
    assert!(display.contains("480"));
}

#[test]
fn resolution_ordering() {
    let low = Resolution::new(640, 480);
    let mid = Resolution::new(1280, 720);
    let high = Resolution::new(1920, 1080);
    assert!(low < mid);
    assert!(mid < high);
    assert!(low < high);
}

#[test]
fn resolution_ordering_equal_width_falls_through_to_height() {
    let portrait_short = Resolution::new(1920, 1080);
    let portrait_tall = Resolution::new(1920, 2160);
    assert!(portrait_short < portrait_tall);
    assert!(portrait_tall > portrait_short);
    let dup = Resolution::new(1920, 1080);
    assert_eq!(portrait_short.cmp(&dup), std::cmp::Ordering::Equal);
}

#[test]
fn resolution_equality() {
    let a = Resolution::new(640, 480);
    let b = Resolution::new(640, 480);
    let c = Resolution::new(1280, 720);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn frame_format_display_roundtrip() {
    for fmt in frame_formats() {
        let s = format!("{fmt}");
        let parsed: FrameFormat = s.parse().expect("should parse back");
        assert_eq!(*fmt, parsed);
    }
}

#[test]
fn frame_formats_non_empty() {
    assert!(!frame_formats().is_empty());
}

#[test]
fn color_frame_formats_subset_of_all() {
    let all = frame_formats();
    for fmt in color_frame_formats() {
        assert!(all.contains(fmt), "{fmt:?} not in frame_formats()");
    }
}

#[test]
fn camera_format_new() {
    let res = Resolution::new(1920, 1080);
    let fmt = CameraFormat::new(res, FrameFormat::MJPEG, 30);
    assert_eq!(fmt.resolution(), res);
    assert_eq!(fmt.format(), FrameFormat::MJPEG);
    assert_eq!(fmt.frame_rate(), 30);
    assert_eq!(fmt.width(), 1920);
    assert_eq!(fmt.height(), 1080);
}

#[test]
fn camera_format_new_from() {
    let fmt = CameraFormat::new_from(640, 480, FrameFormat::YUYV, 15);
    assert_eq!(fmt.width(), 640);
    assert_eq!(fmt.height(), 480);
    assert_eq!(fmt.format(), FrameFormat::YUYV);
    assert_eq!(fmt.frame_rate(), 15);
}

#[test]
fn camera_format_default() {
    let fmt = CameraFormat::default();
    assert_eq!(fmt.width(), 640);
    assert_eq!(fmt.height(), 480);
    assert_eq!(fmt.frame_rate(), 30);
    assert_eq!(fmt.format(), FrameFormat::MJPEG);
}

#[test]
fn camera_format_setters() {
    let mut fmt = CameraFormat::default();
    fmt.set_resolution(Resolution::new(1280, 720));
    fmt.set_frame_rate(60);
    fmt.set_format(FrameFormat::NV12);
    assert_eq!(fmt.resolution(), Resolution::new(1280, 720));
    assert_eq!(fmt.frame_rate(), 60);
    assert_eq!(fmt.format(), FrameFormat::NV12);
}

#[test]
fn camera_format_display() {
    let fmt = CameraFormat::default();
    let display = format!("{fmt}");
    assert!(!display.is_empty());
}

#[test]
fn camera_index_from_u32() {
    let idx = CameraIndex::Index(0u32);
    assert!(idx.is_index());
    assert!(!idx.is_string());
    assert_eq!(idx.as_index().unwrap(), 0);
}

#[test]
fn camera_index_string() {
    let idx = CameraIndex::String("/dev/video0".to_string());
    assert!(idx.is_string());
    assert!(!idx.is_index());
    assert_eq!(idx.as_string(), "/dev/video0");
}

#[test]
fn camera_index_default_is_index_zero() {
    let idx = CameraIndex::default();
    assert!(idx.is_index());
    assert_eq!(idx.as_index().unwrap(), 0);
}

#[test]
fn camera_info_getters_setters() {
    let mut info = CameraInfo::new(
        "Test Camera",
        "A test camera",
        "misc info",
        CameraIndex::Index(0),
    );
    assert_eq!(info.human_name(), "Test Camera");
    assert_eq!(info.description(), "A test camera");
    assert_eq!(info.misc(), "misc info");
    assert_eq!(info.index(), &CameraIndex::Index(0));

    info.set_human_name("New Name");
    assert_eq!(info.human_name(), "New Name");

    info.set_description("New desc");
    assert_eq!(info.description(), "New desc");

    info.set_misc("New misc");
    assert_eq!(info.misc(), "New misc");

    info.set_index(CameraIndex::Index(1));
    assert_eq!(info.index(), &CameraIndex::Index(1));
}

#[test]
fn control_value_setter_accessors() {
    assert!(ControlValueSetter::None.as_none().is_some());
    assert_eq!(ControlValueSetter::Integer(42).as_integer(), Some(&42));
    assert_eq!(ControlValueSetter::Float(2.72).as_float(), Some(&2.72));
    assert_eq!(ControlValueSetter::Boolean(true).as_boolean(), Some(&true));
    assert_eq!(
        ControlValueSetter::String("hello".into()).as_str(),
        Some("hello")
    );
    assert_eq!(
        ControlValueSetter::Bytes(vec![1, 2, 3]).as_bytes(),
        Some([1u8, 2, 3].as_slice())
    );
}

#[test]
fn control_value_setter_wrong_type_returns_none() {
    assert!(ControlValueSetter::Integer(42).as_float().is_none());
    assert!(ControlValueSetter::Float(2.72).as_integer().is_none());
    assert!(ControlValueSetter::Boolean(true).as_str().is_none());
}

#[test]
fn camera_control_basic() {
    let control = CameraControl::new(
        KnownCameraControl::Brightness,
        "Brightness".to_string(),
        ControlValueDescription::Integer {
            value: 50,
            default: 50,
            step: 1,
        },
        vec![KnownCameraControlFlag::Manual],
        true,
    );
    assert_eq!(control.control(), KnownCameraControl::Brightness);
    assert_eq!(control.name(), "Brightness");
    assert!(control.active());
    assert_eq!(control.flag(), &[KnownCameraControlFlag::Manual]);
}

#[test]
fn control_value_description_verify_setter() {
    let desc = ControlValueDescription::IntegerRange {
        min: 0,
        max: 100,
        value: 50,
        step: 1,
        default: 50,
    };
    assert!(desc.verify_setter(&ControlValueSetter::Integer(75)));
    assert!(!desc.verify_setter(&ControlValueSetter::Float(2.72)));
}

#[test]
fn closest_format_when_exact_resolution_unavailable() {
    let available = vec![
        CameraFormat::new_from(640, 480, FrameFormat::MJPEG, 30),
        CameraFormat::new_from(1920, 1080, FrameFormat::MJPEG, 30),
        CameraFormat::new_from(1920, 1080, FrameFormat::MJPEG, 60),
    ];

    // Request 1280x720 which doesn't exist in the available formats
    let requested_fmt = CameraFormat::new_from(1280, 720, FrameFormat::MJPEG, 30);
    let req = RequestedFormat::with_formats(
        RequestedFormatType::Closest(requested_fmt),
        &[FrameFormat::MJPEG],
    );

    let result = req.fulfill(&available);
    assert!(
        result.is_some(),
        "Closest should return a format even when exact resolution is unavailable"
    );

    let result = result.unwrap();
    // 640x480 is the closest by Euclidean distance:
    // dist(1280,720 -> 640,480)   = sqrt(640^2 + 240^2) ≈ 683
    // dist(1280,720 -> 1920,1080) = sqrt(640^2 + 360^2) ≈ 734
    assert_eq!(result.resolution(), Resolution::new(640, 480));
    assert_eq!(result.format(), FrameFormat::MJPEG);
    assert_eq!(result.frame_rate(), 30);
}

// --- RequestedFormatType::fulfill() variant coverage ---

#[test]
fn fulfill_absolute_highest_resolution() {
    let available = vec![
        CameraFormat::new_from(640, 480, FrameFormat::MJPEG, 30),
        CameraFormat::new_from(1280, 720, FrameFormat::MJPEG, 60),
        CameraFormat::new_from(1920, 1080, FrameFormat::MJPEG, 30),
        CameraFormat::new_from(1920, 1080, FrameFormat::MJPEG, 60),
    ];
    let req = RequestedFormat::with_formats(
        RequestedFormatType::AbsoluteHighestResolution,
        &[FrameFormat::MJPEG],
    );
    let result = req.fulfill(&available).unwrap();
    assert_eq!(result.resolution(), Resolution::new(1920, 1080));
    // Among 1920x1080 formats, picks highest frame rate
    assert_eq!(result.frame_rate(), 60);
}

#[test]
fn fulfill_absolute_highest_framerate() {
    let available = vec![
        CameraFormat::new_from(640, 480, FrameFormat::MJPEG, 30),
        CameraFormat::new_from(640, 480, FrameFormat::MJPEG, 120),
        CameraFormat::new_from(1920, 1080, FrameFormat::MJPEG, 120),
    ];
    let req = RequestedFormat::with_formats(
        RequestedFormatType::AbsoluteHighestFrameRate,
        &[FrameFormat::MJPEG],
    );
    let result = req.fulfill(&available).unwrap();
    assert_eq!(result.frame_rate(), 120);
    // Among 120fps formats, picks highest resolution
    assert_eq!(result.resolution(), Resolution::new(1920, 1080));
}

#[test]
fn fulfill_highest_resolution_at_given_resolution() {
    let available = vec![
        CameraFormat::new_from(1280, 720, FrameFormat::MJPEG, 30),
        CameraFormat::new_from(1280, 720, FrameFormat::MJPEG, 60),
        CameraFormat::new_from(1920, 1080, FrameFormat::MJPEG, 30),
    ];
    let req = RequestedFormat::with_formats(
        RequestedFormatType::HighestResolution(Resolution::new(1280, 720)),
        &[FrameFormat::MJPEG],
    );
    let result = req.fulfill(&available).unwrap();
    assert_eq!(result.resolution(), Resolution::new(1280, 720));
    assert_eq!(result.frame_rate(), 60);
}

#[test]
fn fulfill_highest_resolution_no_match() {
    let available = vec![CameraFormat::new_from(640, 480, FrameFormat::MJPEG, 30)];
    let req = RequestedFormat::with_formats(
        RequestedFormatType::HighestResolution(Resolution::new(1920, 1080)),
        &[FrameFormat::MJPEG],
    );
    assert!(req.fulfill(&available).is_none());
}

#[test]
fn fulfill_highest_framerate_at_given_fps() {
    let available = vec![
        CameraFormat::new_from(640, 480, FrameFormat::MJPEG, 30),
        CameraFormat::new_from(1280, 720, FrameFormat::MJPEG, 30),
        CameraFormat::new_from(1920, 1080, FrameFormat::MJPEG, 60),
    ];
    let req = RequestedFormat::with_formats(
        RequestedFormatType::HighestFrameRate(30),
        &[FrameFormat::MJPEG],
    );
    let result = req.fulfill(&available).unwrap();
    assert_eq!(result.frame_rate(), 30);
    // Among 30fps formats, picks highest resolution
    assert_eq!(result.resolution(), Resolution::new(1280, 720));
}

#[test]
fn fulfill_exact_match() {
    // Note: Exact variant does not check membership in the available list —
    // it only verifies the format matches the wanted decoder.
    let target = CameraFormat::new_from(1280, 720, FrameFormat::MJPEG, 30);
    let available = vec![
        CameraFormat::new_from(640, 480, FrameFormat::MJPEG, 30),
        target,
    ];
    let req =
        RequestedFormat::with_formats(RequestedFormatType::Exact(target), &[FrameFormat::MJPEG]);
    let result = req.fulfill(&available).unwrap();
    assert_eq!(result, target);
}

#[test]
fn fulfill_exact_not_in_available_still_returns() {
    // Exact does not check membership — it only validates the decoder match.
    let target = CameraFormat::new_from(4096, 2160, FrameFormat::MJPEG, 120);
    let available = vec![CameraFormat::new_from(640, 480, FrameFormat::MJPEG, 30)];
    let req =
        RequestedFormat::with_formats(RequestedFormatType::Exact(target), &[FrameFormat::MJPEG]);
    let result = req.fulfill(&available).unwrap();
    assert_eq!(result, target);
}

#[test]
fn fulfill_exact_wrong_decoder() {
    let target = CameraFormat::new_from(1280, 720, FrameFormat::NV12, 30);
    let available = vec![target];
    // Request MJPEG decoder but format is NV12
    let req =
        RequestedFormat::with_formats(RequestedFormatType::Exact(target), &[FrameFormat::MJPEG]);
    assert!(req.fulfill(&available).is_none());
}

#[test]
fn fulfill_none_returns_first_compatible() {
    let available = vec![
        CameraFormat::new_from(640, 480, FrameFormat::NV12, 30),
        CameraFormat::new_from(640, 480, FrameFormat::MJPEG, 30),
    ];
    let req = RequestedFormat::with_formats(RequestedFormatType::None, &[FrameFormat::MJPEG]);
    let result = req.fulfill(&available).unwrap();
    assert_eq!(result.format(), FrameFormat::MJPEG);
}

#[test]
fn fulfill_none_no_compatible_format() {
    let available = vec![
        CameraFormat::new_from(640, 480, FrameFormat::NV12, 30),
        CameraFormat::new_from(640, 480, FrameFormat::YUYV, 30),
    ];
    let req = RequestedFormat::with_formats(RequestedFormatType::None, &[FrameFormat::GRAY]);
    assert!(req.fulfill(&available).is_none());
}

#[test]
fn fulfill_empty_format_list() {
    let req = RequestedFormat::with_formats(
        RequestedFormatType::AbsoluteHighestResolution,
        &[FrameFormat::MJPEG],
    );
    assert!(req.fulfill(&[]).is_none());
}

#[test]
fn fulfill_closest_picks_nearest_framerate() {
    let available = vec![
        CameraFormat::new_from(1280, 720, FrameFormat::MJPEG, 15),
        CameraFormat::new_from(1280, 720, FrameFormat::MJPEG, 30),
        CameraFormat::new_from(1280, 720, FrameFormat::MJPEG, 60),
    ];
    let requested = CameraFormat::new_from(1280, 720, FrameFormat::MJPEG, 25);
    let req = RequestedFormat::with_formats(
        RequestedFormatType::Closest(requested),
        &[FrameFormat::MJPEG],
    );
    let result = req.fulfill(&available).unwrap();
    assert_eq!(result.resolution(), Resolution::new(1280, 720));
    assert_eq!(result.frame_rate(), 30); // 30 is closest to 25
}

#[test]
fn fulfill_decoder_filter_applies_across_variants() {
    let available = vec![
        CameraFormat::new_from(1920, 1080, FrameFormat::NV12, 60),
        CameraFormat::new_from(640, 480, FrameFormat::YUYV, 30),
    ];
    // Only accept YUYV
    let req = RequestedFormat::with_formats(
        RequestedFormatType::AbsoluteHighestResolution,
        &[FrameFormat::YUYV],
    );
    let result = req.fulfill(&available).unwrap();
    assert_eq!(result.format(), FrameFormat::YUYV);
    assert_eq!(result.resolution(), Resolution::new(640, 480));
}

// --- ControlValueDescription::verify_setter() coverage ---

#[test]
fn verify_setter_none() {
    let desc = ControlValueDescription::None;
    assert!(desc.verify_setter(&ControlValueSetter::None));
    assert!(!desc.verify_setter(&ControlValueSetter::Integer(0)));
}

#[test]
fn verify_setter_integer_range_in_bounds() {
    let desc = ControlValueDescription::IntegerRange {
        min: 0,
        max: 100,
        value: 50,
        step: 1,
        default: 50,
    };
    assert!(desc.verify_setter(&ControlValueSetter::Integer(0)));
    assert!(desc.verify_setter(&ControlValueSetter::Integer(100)));
}

#[test]
fn verify_setter_integer_range_out_of_bounds() {
    let desc = ControlValueDescription::IntegerRange {
        min: 0,
        max: 100,
        value: 50,
        step: 1,
        default: 50,
    };
    assert!(!desc.verify_setter(&ControlValueSetter::Integer(-1)));
    assert!(!desc.verify_setter(&ControlValueSetter::Integer(101)));
}

#[test]
fn verify_setter_integer_range_zero_step_always_valid() {
    // When step == 0, verify_setter returns true unconditionally —
    // even for mismatched types. This is the documented implementation behavior.
    let desc = ControlValueDescription::IntegerRange {
        min: 0,
        max: 100,
        value: 50,
        step: 0,
        default: 50,
    };
    assert!(desc.verify_setter(&ControlValueSetter::Integer(42)));
    assert!(desc.verify_setter(&ControlValueSetter::Float(2.72)));
}

#[test]
fn verify_setter_boolean() {
    let desc = ControlValueDescription::Boolean {
        value: true,
        default: false,
    };
    assert!(desc.verify_setter(&ControlValueSetter::Boolean(true)));
    assert!(desc.verify_setter(&ControlValueSetter::Boolean(false)));
    assert!(!desc.verify_setter(&ControlValueSetter::Integer(1)));
}

#[test]
fn verify_setter_string() {
    let desc = ControlValueDescription::String {
        value: "current".to_string(),
        default: Some("default".to_string()),
    };
    assert!(desc.verify_setter(&ControlValueSetter::String("anything".into())));
    assert!(!desc.verify_setter(&ControlValueSetter::Integer(0)));
}

#[test]
fn verify_setter_bytes() {
    let desc = ControlValueDescription::Bytes {
        value: vec![1, 2, 3],
        default: vec![0],
    };
    assert!(desc.verify_setter(&ControlValueSetter::Bytes(vec![4, 5])));
    assert!(!desc.verify_setter(&ControlValueSetter::None));
}

#[test]
fn verify_setter_point_rejects_nan() {
    let desc = ControlValueDescription::Point {
        value: (0.0, 0.0),
        default: (0.0, 0.0),
    };
    assert!(desc.verify_setter(&ControlValueSetter::Point(1.0, 2.0)));
    assert!(!desc.verify_setter(&ControlValueSetter::Point(f64::NAN, 0.0)));
    assert!(!desc.verify_setter(&ControlValueSetter::Point(0.0, f64::NAN)));
    assert!(!desc.verify_setter(&ControlValueSetter::Point(f64::INFINITY, 0.0)));
}

#[test]
fn verify_setter_enum_checks_possible_values() {
    let desc = ControlValueDescription::Enum {
        value: 1,
        possible: vec![1, 2, 3],
        default: 1,
    };
    assert!(desc.verify_setter(&ControlValueSetter::EnumValue(1)));
    assert!(desc.verify_setter(&ControlValueSetter::EnumValue(3)));
    assert!(!desc.verify_setter(&ControlValueSetter::EnumValue(99)));
    assert!(!desc.verify_setter(&ControlValueSetter::Integer(1)));
}

#[test]
fn verify_setter_float_range_in_bounds() {
    // Zero step bypasses all validation unconditionally.
    let desc_zero_step = ControlValueDescription::FloatRange {
        min: 0.0,
        max: 1.0,
        value: 0.5,
        step: 0.0,
        default: 0.5,
    };
    assert!(desc_zero_step.verify_setter(&ControlValueSetter::Float(0.75)));
    assert!(desc_zero_step.verify_setter(&ControlValueSetter::Float(999.0)));

    // Non-zero step exercises the actual range-checking logic.
    let desc_with_step = ControlValueDescription::FloatRange {
        min: 0.0,
        max: 1.0,
        value: 0.5,
        step: 0.5,
        default: 0.0,
    };
    assert!(desc_with_step.verify_setter(&ControlValueSetter::Float(0.5)));
    assert!(!desc_with_step.verify_setter(&ControlValueSetter::Float(2.0)));
    assert!(!desc_with_step.verify_setter(&ControlValueSetter::Integer(1)));
}

#[test]
fn verify_setter_float_range_wrong_type() {
    let desc = ControlValueDescription::FloatRange {
        min: 0.0,
        max: 1.0,
        value: 0.5,
        step: 0.1,
        default: 0.5,
    };
    assert!(!desc.verify_setter(&ControlValueSetter::Integer(1)));
}

#[test]
fn verify_setter_float_range_out_of_bounds_and_non_finite() {
    // Companion to `verify_setter_float_range_in_bounds`: confirms
    // values below `min` are rejected, and NaN / infinity are
    // rejected via the step-alignment math (NaN % step == NaN,
    // which is `!= 0_f64`).
    let desc = ControlValueDescription::FloatRange {
        min: 0.0,
        max: 1.0,
        value: 0.5,
        step: 0.5,
        default: 0.0,
    };
    // Below min.
    assert!(!desc.verify_setter(&ControlValueSetter::Float(-0.5)));
    // NaN — step-alignment math yields NaN % step = NaN, which is
    // not equal to 0_f64.
    assert!(!desc.verify_setter(&ControlValueSetter::Float(f64::NAN)));
    // Infinity — same logic, infinity % step == NaN.
    assert!(!desc.verify_setter(&ControlValueSetter::Float(f64::INFINITY)));
    assert!(!desc.verify_setter(&ControlValueSetter::Float(f64::NEG_INFINITY)));
}

#[test]
fn verify_setter_float_non_finite_rejected() {
    // The unbounded `Float` variant — same step-alignment math —
    // must also reject NaN / infinity for consistency with the
    // bounded `FloatRange` and `Point` variants.
    let desc = ControlValueDescription::Float {
        value: 0.5,
        default: 0.5,
        step: 0.5,
    };
    assert!(!desc.verify_setter(&ControlValueSetter::Float(f64::NAN)));
    assert!(!desc.verify_setter(&ControlValueSetter::Float(f64::INFINITY)));
    assert!(!desc.verify_setter(&ControlValueSetter::Float(f64::NEG_INFINITY)));
}

#[test]
fn verify_setter_key_value_pair() {
    let desc = ControlValueDescription::KeyValuePair {
        key: 1,
        value: 2,
        default: (0, 0),
    };
    assert!(desc.verify_setter(&ControlValueSetter::KeyValue(10, 20)));
    assert!(!desc.verify_setter(&ControlValueSetter::Integer(1)));
}

#[test]
fn verify_setter_integer() {
    let desc = ControlValueDescription::Integer {
        value: 50,
        default: 50,
        step: 5,
    };
    // Alignment check uses OR logic: (i + default) % step == 0 || (i + value) % step == 0
    // Here value == default == 50, so both paths are equivalent: (5 + 50) % 5 == 0 → aligned
    assert!(desc.verify_setter(&ControlValueSetter::Integer(5)));
    // (3 + 50) % 5 == 3 → not aligned via either path
    assert!(!desc.verify_setter(&ControlValueSetter::Integer(3)));
    assert!(!desc.verify_setter(&ControlValueSetter::Float(1.0)));

    // Test OR-logic: value and default differ so both alignment paths are distinct
    let desc_diff = ControlValueDescription::Integer {
        value: 7,
        default: 3,
        step: 5,
    };
    // i=2: (2+3)%5==0 → passes via default path (but (2+7)%5==4 → fails via value path)
    assert!(desc_diff.verify_setter(&ControlValueSetter::Integer(2)));
    // i=3: (3+3)%5==1 fails, (3+7)%5==0 → passes via value path
    assert!(desc_diff.verify_setter(&ControlValueSetter::Integer(3)));
    // i=1: (1+3)%5==4 fails, (1+7)%5==3 fails → both paths fail
    assert!(!desc_diff.verify_setter(&ControlValueSetter::Integer(1)));
}

#[test]
fn verify_setter_integer_zero_step() {
    // When step == 0, verify_setter returns true unconditionally —
    // even for mismatched types (step==0 bypasses type checking).
    let desc = ControlValueDescription::Integer {
        value: 50,
        default: 50,
        step: 0,
    };
    assert!(desc.verify_setter(&ControlValueSetter::Integer(99)));
    assert!(desc.verify_setter(&ControlValueSetter::Float(1.0)));
}

#[test]
fn verify_setter_float() {
    // When step == 0.0, verify_setter returns true unconditionally.
    let desc_zero_step = ControlValueDescription::Float {
        value: 0.5,
        default: 0.5,
        step: 0.0,
    };
    assert!(desc_zero_step.verify_setter(&ControlValueSetter::Float(0.75)));
    assert!(desc_zero_step.verify_setter(&ControlValueSetter::Integer(1)));

    // With a non-zero step, test alignment and type rejection.
    let desc_with_step = ControlValueDescription::Float {
        value: 0.5,
        default: 0.5,
        step: 0.5,
    };
    // (1.0 - 0.5).abs() % 0.5 == 0.0 → aligned
    assert!(desc_with_step.verify_setter(&ControlValueSetter::Float(1.0)));
    // (0.75 - 0.5).abs() % 0.5 == 0.25 → not aligned
    assert!(!desc_with_step.verify_setter(&ControlValueSetter::Float(0.75)));
    assert!(!desc_with_step.verify_setter(&ControlValueSetter::Integer(1)));
}

#[test]
fn verify_setter_rgb() {
    // RGB verify_setter accepts any finite (non-NaN, non-infinite)
    // triple where each channel is within `0.0 ..= max`. Mirrors the
    // `value >= min && value <= max` shape used by IntegerRange /
    // FloatRange. See the inline comment in
    // `ControlValueDescription::verify_setter` for why this used to
    // be inverted.
    let desc = ControlValueDescription::RGB {
        value: (0.5, 0.5, 0.5),
        max: (1.0, 1.0, 1.0),
        default: (0.0, 0.0, 0.0),
    };
    // In-range values pass: at lower bound, at upper bound, in the middle.
    assert!(desc.verify_setter(&ControlValueSetter::RGB(0.0, 0.0, 0.0)));
    assert!(desc.verify_setter(&ControlValueSetter::RGB(0.5, 0.5, 0.5)));
    assert!(desc.verify_setter(&ControlValueSetter::RGB(1.0, 1.0, 1.0)));
    // Above max fails on any channel.
    assert!(!desc.verify_setter(&ControlValueSetter::RGB(2.0, 1.0, 1.0)));
    assert!(!desc.verify_setter(&ControlValueSetter::RGB(1.0, 2.0, 1.0)));
    assert!(!desc.verify_setter(&ControlValueSetter::RGB(1.0, 1.0, 2.0)));
    // Negative on any channel fails.
    assert!(!desc.verify_setter(&ControlValueSetter::RGB(-0.1, 0.5, 0.5)));
    // Non-finite (NaN, infinity) fails.
    assert!(!desc.verify_setter(&ControlValueSetter::RGB(f64::NAN, 0.5, 0.5)));
    assert!(!desc.verify_setter(&ControlValueSetter::RGB(0.5, f64::INFINITY, 0.5)));
    // Wrong setter variant always fails.
    assert!(!desc.verify_setter(&ControlValueSetter::Integer(1)));
}

// --- CameraControl value round-trip tests ---

#[test]
fn control_value_roundtrip_integer() {
    let desc = ControlValueDescription::Integer {
        value: 42,
        default: 0,
        step: 1,
    };
    let control = CameraControl::new(
        KnownCameraControl::Brightness,
        "Brightness".to_string(),
        desc.clone(),
        vec![KnownCameraControlFlag::Manual],
        true,
    );
    let setter = control.value();
    assert_eq!(setter, ControlValueSetter::Integer(42));
    assert!(desc.verify_setter(&setter));
}

#[test]
fn control_value_roundtrip_integer_range() {
    let desc = ControlValueDescription::IntegerRange {
        min: -100,
        max: 100,
        value: 75,
        step: 5,
        default: 0,
    };
    let control = CameraControl::new(
        KnownCameraControl::Contrast,
        "Contrast".to_string(),
        desc.clone(),
        vec![KnownCameraControlFlag::Manual],
        true,
    );
    let setter = control.value();
    assert_eq!(setter, ControlValueSetter::Integer(75));
    assert!(desc.verify_setter(&setter));
}

#[test]
fn control_value_roundtrip_float() {
    let desc = ControlValueDescription::Float {
        value: 1.5,
        default: 1.0,
        step: 0.5,
    };
    let control = CameraControl::new(
        KnownCameraControl::Gamma,
        "Gamma".to_string(),
        desc.clone(),
        vec![KnownCameraControlFlag::Manual],
        true,
    );
    let setter = control.value();
    assert_eq!(setter, ControlValueSetter::Float(1.5));
    assert!(desc.verify_setter(&setter));
}

#[test]
fn control_value_roundtrip_float_range() {
    let desc = ControlValueDescription::FloatRange {
        min: 0.0,
        max: 10.0,
        value: 5.0,
        step: 0.5,
        default: 1.0,
    };
    let control = CameraControl::new(
        KnownCameraControl::Exposure,
        "Exposure".to_string(),
        desc.clone(),
        vec![KnownCameraControlFlag::Automatic],
        true,
    );
    let setter = control.value();
    assert_eq!(setter, ControlValueSetter::Float(5.0));
    assert!(desc.verify_setter(&setter));
}

#[test]
fn control_value_roundtrip_boolean() {
    let desc = ControlValueDescription::Boolean {
        value: true,
        default: false,
    };
    let control = CameraControl::new(
        KnownCameraControl::BacklightComp,
        "BacklightComp".to_string(),
        desc.clone(),
        vec![KnownCameraControlFlag::Manual],
        true,
    );
    let setter = control.value();
    assert_eq!(setter, ControlValueSetter::Boolean(true));
    assert!(desc.verify_setter(&setter));
}

#[test]
fn control_value_roundtrip_string() {
    let desc = ControlValueDescription::String {
        value: "hello".to_string(),
        default: Some("world".to_string()),
    };
    let control = CameraControl::new(
        KnownCameraControl::Other(999),
        "CustomString".to_string(),
        desc.clone(),
        vec![KnownCameraControlFlag::ReadOnly],
        false,
    );
    let setter = control.value();
    assert_eq!(setter, ControlValueSetter::String("hello".to_string()));
    assert!(desc.verify_setter(&setter));
}

#[test]
fn control_value_roundtrip_bytes() {
    let desc = ControlValueDescription::Bytes {
        value: vec![0xDE, 0xAD],
        default: vec![0x00],
    };
    let control = CameraControl::new(
        KnownCameraControl::Other(1000),
        "CustomBytes".to_string(),
        desc.clone(),
        vec![],
        true,
    );
    let setter = control.value();
    assert_eq!(setter, ControlValueSetter::Bytes(vec![0xDE, 0xAD]));
    assert!(desc.verify_setter(&setter));
}

#[test]
fn control_value_roundtrip_key_value() {
    let desc = ControlValueDescription::KeyValuePair {
        key: 10,
        value: 20,
        default: (0, 0),
    };
    let control = CameraControl::new(
        KnownCameraControl::Other(2000),
        "KVPair".to_string(),
        desc.clone(),
        vec![KnownCameraControlFlag::WriteOnly],
        true,
    );
    let setter = control.value();
    assert_eq!(setter, ControlValueSetter::KeyValue(10, 20));
    assert!(desc.verify_setter(&setter));
}

#[test]
fn control_value_roundtrip_point() {
    let desc = ControlValueDescription::Point {
        value: (1.5, 2.5),
        default: (0.0, 0.0),
    };
    let control = CameraControl::new(
        KnownCameraControl::Pan,
        "Pan".to_string(),
        desc.clone(),
        vec![KnownCameraControlFlag::Continuous],
        true,
    );
    let setter = control.value();
    assert_eq!(setter, ControlValueSetter::Point(1.5, 2.5));
    assert!(desc.verify_setter(&setter));
}

#[test]
fn control_value_roundtrip_enum() {
    let desc = ControlValueDescription::Enum {
        value: 2,
        possible: vec![1, 2, 3, 4],
        default: 1,
    };
    let control = CameraControl::new(
        KnownCameraControl::WhiteBalance,
        "WhiteBalance".to_string(),
        desc.clone(),
        vec![KnownCameraControlFlag::Manual],
        true,
    );
    let setter = control.value();
    assert_eq!(setter, ControlValueSetter::EnumValue(2));
    assert!(desc.verify_setter(&setter));
}

#[test]
fn control_value_roundtrip_rgb() {
    // The current value lies strictly inside the [0, max] cube on each
    // channel — verify_setter should accept it. (Previously this test
    // relied on the inverted `>= max` predicate; see
    // `verify_setter_rgb` for the full coverage.)
    let desc = ControlValueDescription::RGB {
        value: (0.25, 0.5, 0.75),
        max: (1.0, 1.0, 1.0),
        default: (0.0, 0.0, 0.0),
    };
    let control = CameraControl::new(
        KnownCameraControl::Other(3000),
        "RGBControl".to_string(),
        desc.clone(),
        vec![KnownCameraControlFlag::Volatile],
        true,
    );
    let setter = control.value();
    assert_eq!(setter, ControlValueSetter::RGB(0.25, 0.5, 0.75));
    assert!(desc.verify_setter(&setter));
}

#[test]
fn control_value_roundtrip_none() {
    let desc = ControlValueDescription::None;
    let control = CameraControl::new(
        KnownCameraControl::Other(0),
        "NoneCtrl".to_string(),
        desc.clone(),
        vec![KnownCameraControlFlag::Disabled],
        false,
    );
    let setter = control.value();
    assert_eq!(setter, ControlValueSetter::None);
    assert!(desc.verify_setter(&setter));
}

#[test]
fn known_camera_control_all_variants_display_non_empty() {
    let controls = [
        KnownCameraControl::Brightness,
        KnownCameraControl::Contrast,
        KnownCameraControl::Hue,
        KnownCameraControl::Saturation,
        KnownCameraControl::Sharpness,
        KnownCameraControl::Gamma,
        KnownCameraControl::WhiteBalance,
        KnownCameraControl::BacklightComp,
        KnownCameraControl::Gain,
        KnownCameraControl::Pan,
        KnownCameraControl::Tilt,
        KnownCameraControl::Zoom,
        KnownCameraControl::Exposure,
        KnownCameraControl::Iris,
        KnownCameraControl::Focus,
        KnownCameraControl::Other(12345),
    ];
    for ctrl in &controls {
        // Verify Display doesn't panic and produces non-empty output
        let display = format!("{ctrl}");
        assert!(!display.is_empty(), "Display for {ctrl:?} was empty");
    }
}

#[test]
fn camera_control_set_active_toggle() {
    let mut control = CameraControl::new(
        KnownCameraControl::Focus,
        "Focus".to_string(),
        ControlValueDescription::Integer {
            value: 50,
            default: 50,
            step: 1,
        },
        vec![
            KnownCameraControlFlag::Manual,
            KnownCameraControlFlag::Automatic,
        ],
        true,
    );
    assert!(control.active());
    control.set_active(false);
    assert!(!control.active());
    control.set_active(true);
    assert!(control.active());
}

#[test]
fn camera_control_multiple_flags_preserved() {
    let flags = vec![
        KnownCameraControlFlag::Manual,
        KnownCameraControlFlag::Volatile,
        KnownCameraControlFlag::Continuous,
    ];
    let control = CameraControl::new(
        KnownCameraControl::Zoom,
        "Zoom".to_string(),
        ControlValueDescription::IntegerRange {
            min: 1,
            max: 10,
            value: 5,
            step: 1,
            default: 1,
        },
        flags.clone(),
        true,
    );
    assert_eq!(control.flag(), &flags);
}

#[test]
fn control_value_description_value_extraction_all_variants() {
    // Verify that .value() on every ControlValueDescription variant returns
    // a ControlValueSetter that matches the stored value.
    let cases: Vec<(ControlValueDescription, ControlValueSetter)> = vec![
        (ControlValueDescription::None, ControlValueSetter::None),
        (
            ControlValueDescription::Integer {
                value: -7,
                default: 0,
                step: 1,
            },
            ControlValueSetter::Integer(-7),
        ),
        (
            ControlValueDescription::IntegerRange {
                min: 0,
                max: 255,
                value: 128,
                step: 1,
                default: 0,
            },
            ControlValueSetter::Integer(128),
        ),
        (
            ControlValueDescription::Float {
                value: 3.14,
                default: 0.0,
                step: 0.01,
            },
            ControlValueSetter::Float(3.14),
        ),
        (
            ControlValueDescription::FloatRange {
                min: -1.0,
                max: 1.0,
                value: 0.5,
                step: 0.1,
                default: 0.0,
            },
            ControlValueSetter::Float(0.5),
        ),
        (
            ControlValueDescription::Boolean {
                value: false,
                default: true,
            },
            ControlValueSetter::Boolean(false),
        ),
        (
            ControlValueDescription::String {
                value: "test".to_string(),
                default: None,
            },
            ControlValueSetter::String("test".to_string()),
        ),
        (
            ControlValueDescription::Bytes {
                value: vec![1, 2, 3],
                default: vec![],
            },
            ControlValueSetter::Bytes(vec![1, 2, 3]),
        ),
        (
            ControlValueDescription::KeyValuePair {
                key: 42,
                value: 84,
                default: (0, 0),
            },
            ControlValueSetter::KeyValue(42, 84),
        ),
        (
            ControlValueDescription::Point {
                value: (9.0, 10.0),
                default: (0.0, 0.0),
            },
            ControlValueSetter::Point(9.0, 10.0),
        ),
        (
            ControlValueDescription::Enum {
                value: 3,
                possible: vec![1, 2, 3],
                default: 1,
            },
            ControlValueSetter::EnumValue(3),
        ),
        (
            ControlValueDescription::RGB {
                value: (0.1, 0.2, 0.3),
                max: (1.0, 1.0, 1.0),
                default: (0.0, 0.0, 0.0),
            },
            ControlValueSetter::RGB(0.1, 0.2, 0.3),
        ),
    ];

    for (desc, expected_setter) in &cases {
        assert_eq!(
            desc.value(),
            *expected_setter,
            "value() mismatch for {desc:?}"
        );
    }
}

// --- FrameFormat parse edge cases ---

#[test]
fn frame_format_parse_invalid_returns_error() {
    assert!("INVALID".parse::<FrameFormat>().is_err());
    assert!("".parse::<FrameFormat>().is_err());
    assert!("mjpeg".parse::<FrameFormat>().is_err()); // case-sensitive
}

#[test]
fn frame_format_rawbgr_display_parse() {
    let fmt = FrameFormat::RAWBGR;
    let s = format!("{fmt}");
    assert_eq!(s, "RAWBGR");
    let parsed: FrameFormat = s.parse().unwrap();
    assert_eq!(parsed, FrameFormat::RAWBGR);
}

// --- CameraIndex additional coverage ---

#[test]
fn camera_index_as_index_returns_err_for_string() {
    let idx = CameraIndex::String("test".to_string());
    assert!(idx.as_index().is_err());
}

#[test]
fn camera_index_display() {
    let idx = CameraIndex::Index(5);
    let s = format!("{idx}");
    assert!(s.contains('5'));
}

// --- ControlValueSetter additional accessors ---

#[test]
fn control_value_setter_key_value() {
    let setter = ControlValueSetter::KeyValue(10, 20);
    assert_eq!(setter.as_key_value(), Some((&10, &20)));
}

#[test]
fn control_value_setter_point() {
    let setter = ControlValueSetter::Point(1.5, 2.5);
    assert_eq!(setter.as_point(), Some((&1.5, &2.5)));
}

#[test]
fn control_value_setter_enum_value() {
    let setter = ControlValueSetter::EnumValue(42);
    assert_eq!(setter.as_enum(), Some(&42));
}

#[test]
fn control_value_setter_rgb() {
    let setter = ControlValueSetter::RGB(0.1, 0.2, 0.3);
    let (r, g, b) = setter.as_rgb().unwrap();
    assert!((r - 0.1).abs() < f64::EPSILON);
    assert!((g - 0.2).abs() < f64::EPSILON);
    assert!((b - 0.3).abs() < f64::EPSILON);
}

// ─── FrameFormat FourCC helpers ───

#[test]
fn frame_format_fourcc_roundtrip() {
    for &fmt in frame_formats() {
        assert_eq!(
            FrameFormat::from_fourcc(fmt.to_fourcc()),
            Some(fmt),
            "round-trip failed for {fmt:?}"
        );
    }
}

#[test]
fn frame_format_from_fourcc_unknown_returns_none() {
    assert_eq!(FrameFormat::from_fourcc("H264"), None);
    assert_eq!(FrameFormat::from_fourcc(""), None);
    assert_eq!(FrameFormat::from_fourcc("XXXX"), None);
}

#[test]
fn frame_format_to_fourcc_is_four_bytes() {
    for &fmt in frame_formats() {
        assert_eq!(
            fmt.to_fourcc().len(),
            4,
            "FourCC for {fmt:?} is not 4 bytes"
        );
    }
}

// ─── KnownCameraControl index helpers ───

#[test]
fn known_camera_control_index_roundtrip() {
    for ctrl in all_known_camera_controls() {
        let idx = ctrl.as_index().expect("standard control should have index");
        assert_eq!(
            KnownCameraControl::from_index(idx),
            Some(ctrl),
            "round-trip failed for {ctrl:?}"
        );
    }
}

#[test]
fn known_camera_control_other_has_no_index() {
    assert_eq!(KnownCameraControl::Other(42).as_index(), None);
}

#[test]
fn known_camera_control_from_index_out_of_range() {
    assert_eq!(KnownCameraControl::from_index(15), None);
    assert_eq!(KnownCameraControl::from_index(255), None);
}

#[test]
fn known_camera_control_standard_count_matches_all() {
    assert_eq!(
        KnownCameraControl::STANDARD_COUNT,
        all_known_camera_controls().len(),
        "STANDARD_COUNT and all_known_camera_controls() length must agree"
    );
}

// ─── KnownCameraControl platform-ID helpers ───

#[test]
fn known_camera_control_platform_id_roundtrip() {
    // Synthetic table: platform ID = canonical index * 100.
    let table: [u32; KnownCameraControl::STANDARD_COUNT] =
        core::array::from_fn(|i| (i as u32) * 100);

    for ctrl in all_known_camera_controls() {
        let pid = ctrl.to_platform_id(&table);
        let back = KnownCameraControl::from_platform_id(pid, &table);
        assert_eq!(back, ctrl, "round-trip failed for {ctrl:?}");
    }
}

#[test]
fn known_camera_control_from_platform_id_unknown_falls_back_to_other() {
    let table: [u32; KnownCameraControl::STANDARD_COUNT] =
        core::array::from_fn(|i| (i as u32) * 100);
    let unknown_id = 9999_u32;
    assert_eq!(
        KnownCameraControl::from_platform_id(unknown_id, &table),
        KnownCameraControl::Other(u128::from(unknown_id)),
    );
}

#[test]
fn known_camera_control_other_to_platform_id_truncates() {
    let table = [0_u32; KnownCameraControl::STANDARD_COUNT];
    let large_id: u128 = u128::from(u32::MAX) + 1;
    // Truncation: (u32::MAX + 1) as u32 == 0
    assert_eq!(
        KnownCameraControl::Other(large_id).to_platform_id(&table),
        0
    );
}
