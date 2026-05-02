use super::*;

#[test]
fn yuyv_marker_maps_to_frame_format_yuyv() {
    assert_eq!(Yuyv::FRAME_FORMAT, FrameFormat::YUYV);
}

#[test]
fn nv12_marker_maps_to_frame_format_nv12() {
    assert_eq!(Nv12::FRAME_FORMAT, FrameFormat::NV12);
}

#[test]
fn mjpeg_marker_maps_to_frame_format_mjpeg() {
    assert_eq!(Mjpeg::FRAME_FORMAT, FrameFormat::MJPEG);
}

#[test]
fn gray_marker_maps_to_frame_format_gray() {
    assert_eq!(Gray::FRAME_FORMAT, FrameFormat::GRAY);
}

#[test]
fn raw_rgb_marker_maps_to_frame_format_rawrgb() {
    assert_eq!(RawRgb::FRAME_FORMAT, FrameFormat::RAWRGB);
}

#[test]
fn raw_bgr_marker_maps_to_frame_format_rawbgr() {
    assert_eq!(RawBgr::FRAME_FORMAT, FrameFormat::RAWBGR);
}

#[test]
fn marker_constants_are_pairwise_distinct() {
    let consts = [
        Yuyv::FRAME_FORMAT,
        Nv12::FRAME_FORMAT,
        Mjpeg::FRAME_FORMAT,
        Gray::FRAME_FORMAT,
        RawRgb::FRAME_FORMAT,
        RawBgr::FRAME_FORMAT,
    ];
    let mut sorted = consts.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        consts.len(),
        "two markers map to the same FrameFormat: {consts:?}",
    );
}

#[test]
fn marker_zsts_are_zero_sized() {
    use std::mem::size_of;
    assert_eq!(size_of::<Yuyv>(), 0);
    assert_eq!(size_of::<Nv12>(), 0);
    assert_eq!(size_of::<Mjpeg>(), 0);
    assert_eq!(size_of::<Gray>(), 0);
    assert_eq!(size_of::<RawRgb>(), 0);
    assert_eq!(size_of::<RawBgr>(), 0);
}
