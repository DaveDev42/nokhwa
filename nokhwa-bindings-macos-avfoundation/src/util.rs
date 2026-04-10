use crate::ffi::{
    kCMPixelFormat_24RGB, kCMPixelFormat_422YpCbCr8_yuvs, kCMPixelFormat_8IndexedGray_WhiteIsZero,
    kCMVideoCodecType_422YpCbCr8, kCMVideoCodecType_JPEG, kCMVideoCodecType_JPEG_OpenDML,
    kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange,
    kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
    kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange, OSType,
};
use nokhwa_core::types::FrameFormat;
use std::borrow::Cow;
use std::sync::mpsc::{Receiver, Sender};

#[allow(non_upper_case_globals)]
pub(crate) fn raw_fcc_to_frameformat(raw: OSType) -> Option<FrameFormat> {
    match raw {
        kCMVideoCodecType_422YpCbCr8 | kCMPixelFormat_422YpCbCr8_yuvs => Some(FrameFormat::YUYV),
        kCMVideoCodecType_JPEG | kCMVideoCodecType_JPEG_OpenDML => Some(FrameFormat::MJPEG),
        kCMPixelFormat_8IndexedGray_WhiteIsZero => Some(FrameFormat::GRAY),
        kCVPixelFormatType_420YpCbCr8BiPlanarFullRange
        | kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange
        | kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange => Some(FrameFormat::NV12),
        kCMPixelFormat_24RGB => Some(FrameFormat::RAWRGB),
        _ => None,
    }
}

pub type CompressionData<'a> = (Cow<'a, [u8]>, FrameFormat, Option<std::time::Duration>);
pub type DataPipe<'a> = (Sender<CompressionData<'a>>, Receiver<CompressionData<'a>>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yuyv_from_422ycbcr8() {
        assert_eq!(
            raw_fcc_to_frameformat(kCMVideoCodecType_422YpCbCr8),
            Some(FrameFormat::YUYV)
        );
    }

    #[test]
    fn yuyv_from_yuvs() {
        assert_eq!(
            raw_fcc_to_frameformat(kCMPixelFormat_422YpCbCr8_yuvs),
            Some(FrameFormat::YUYV)
        );
    }

    #[test]
    fn mjpeg_from_jpeg() {
        assert_eq!(
            raw_fcc_to_frameformat(kCMVideoCodecType_JPEG),
            Some(FrameFormat::MJPEG)
        );
    }

    #[test]
    fn mjpeg_from_jpeg_opendml() {
        assert_eq!(
            raw_fcc_to_frameformat(kCMVideoCodecType_JPEG_OpenDML),
            Some(FrameFormat::MJPEG)
        );
    }

    #[test]
    fn gray_from_8indexed() {
        assert_eq!(
            raw_fcc_to_frameformat(kCMPixelFormat_8IndexedGray_WhiteIsZero),
            Some(FrameFormat::GRAY)
        );
    }

    #[test]
    fn nv12_from_biplanar_full_range() {
        assert_eq!(
            raw_fcc_to_frameformat(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange),
            Some(FrameFormat::NV12)
        );
    }

    #[test]
    fn nv12_from_biplanar_video_range() {
        assert_eq!(
            raw_fcc_to_frameformat(kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange),
            Some(FrameFormat::NV12)
        );
    }

    #[test]
    fn nv12_from_10bit_biplanar() {
        assert_eq!(
            raw_fcc_to_frameformat(kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange),
            Some(FrameFormat::NV12)
        );
    }

    #[test]
    fn rawrgb_from_24rgb() {
        assert_eq!(
            raw_fcc_to_frameformat(kCMPixelFormat_24RGB),
            Some(FrameFormat::RAWRGB)
        );
    }

    #[test]
    fn unknown_fourcc_returns_none() {
        assert_eq!(raw_fcc_to_frameformat(0xDEAD_BEEF), None);
        assert_eq!(raw_fcc_to_frameformat(0), None);
    }
}
