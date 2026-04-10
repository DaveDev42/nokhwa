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
    assert_eq!(ControlValueSetter::Float(3.14).as_float(), Some(&3.14));
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
    assert!(ControlValueSetter::Float(3.14).as_integer().is_none());
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
    assert!(!desc.verify_setter(&ControlValueSetter::Float(3.14)));
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
