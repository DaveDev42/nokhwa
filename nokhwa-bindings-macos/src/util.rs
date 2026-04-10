use crate::ffi::{
    kCMPixelFormat_24RGB, kCMPixelFormat_422YpCbCr8_yuvs, kCMPixelFormat_8IndexedGray_WhiteIsZero,
    kCMVideoCodecType_422YpCbCr8, kCMVideoCodecType_JPEG, kCMVideoCodecType_JPEG_OpenDML,
    kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange,
    kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
    kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange, OSType,
};
use flume::{Receiver, Sender};
use nokhwa_core::types::FrameFormat;
use std::borrow::Cow;

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
