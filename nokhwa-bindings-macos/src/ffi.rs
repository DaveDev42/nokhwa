// all of this is stolen from bindgen
// steal it idc
use core_media_sys::{
    CMBlockBufferRef, CMFormatDescriptionRef, CMSampleBufferRef, CMTime, CMVideoDimensions,
    FourCharCode,
};
use objc2::encode::{Encode, Encoding};
use objc2::runtime::AnyObject;

pub type CGFloat = std::ffi::c_float;

pub type Id = *mut AnyObject;

#[repr(transparent)]
#[derive(Clone)]
pub struct NSObject(pub Id);

// SAFETY: NSObject is repr(transparent) over *mut AnyObject, which is a pointer.
unsafe impl Encode for NSObject {
    const ENCODING: Encoding = Encoding::Object;
}

#[repr(transparent)]
#[derive(Clone)]
pub struct NSString(pub Id);

// SAFETY: NSString is repr(transparent) over *mut AnyObject, which is a pointer.
unsafe impl Encode for NSString {
    const ENCODING: Encoding = Encoding::Object;
}

pub type AVMediaType = NSString;

#[allow(non_snake_case)]
#[link(name = "CoreMedia", kind = "framework")]
extern "C" {
    pub fn CMVideoFormatDescriptionGetDimensions(
        videoDesc: CMFormatDescriptionRef,
    ) -> CMVideoDimensions;

    pub fn CMTimeMake(value: i64, scale: i32) -> CMTime;

    pub fn CMBlockBufferGetDataLength(theBuffer: CMBlockBufferRef) -> std::os::raw::c_int;

    pub fn CMBlockBufferCopyDataBytes(
        theSourceBuffer: CMBlockBufferRef,
        offsetToData: usize,
        dataLength: usize,
        destination: *mut std::os::raw::c_void,
    ) -> std::os::raw::c_int;

    pub fn CMSampleBufferGetDataBuffer(sbuf: CMSampleBufferRef) -> CMBlockBufferRef;

    pub fn dispatch_queue_create(label: *const std::os::raw::c_char, attr: NSObject) -> NSObject;

    pub fn dispatch_release(object: NSObject);

    pub fn CMSampleBufferGetImageBuffer(sbuf: CMSampleBufferRef) -> CVImageBufferRef;

    pub fn CVPixelBufferLockBaseAddress(
        pixelBuffer: CVPixelBufferRef,
        lockFlags: CVPixelBufferLockFlags,
    ) -> CVReturn;

    pub fn CVPixelBufferUnlockBaseAddress(
        pixelBuffer: CVPixelBufferRef,
        unlockFlags: CVPixelBufferLockFlags,
    ) -> CVReturn;

    pub fn CVPixelBufferGetDataSize(pixelBuffer: CVPixelBufferRef) -> std::os::raw::c_ulong;

    pub fn CVPixelBufferGetBaseAddress(pixelBuffer: CVPixelBufferRef) -> *mut std::os::raw::c_void;

    pub fn CVPixelBufferGetPixelFormatType(pixelBuffer: CVPixelBufferRef) -> OSType;
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct CGPoint {
    pub x: CGFloat,
    pub y: CGFloat,
}

// SAFETY: CGPoint is repr(C) with two f32 fields, matching {ff} encoding.
unsafe impl Encode for CGPoint {
    const ENCODING: Encoding = Encoding::Struct("CGPoint", &[Encoding::Float, Encoding::Float]);
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct __CVBuffer {
    _unused: [u8; 0],
}

#[allow(non_snake_case)]
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
#[repr(C)]
pub struct AVCaptureWhiteBalanceGains {
    pub blueGain: f32,
    pub greenGain: f32,
    pub redGain: f32,
}

// SAFETY: AVCaptureWhiteBalanceGains is repr(C) with three f32 fields.
unsafe impl Encode for AVCaptureWhiteBalanceGains {
    const ENCODING: Encoding = Encoding::Struct(
        "AVCaptureWhiteBalanceGains",
        &[Encoding::Float, Encoding::Float, Encoding::Float],
    );
}

/// A local newtype wrapper around `CMTime` to implement `Encode`.
/// This is needed because both `CMTime` and `Encode` are from external crates.
#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct EncodableCMTime(pub CMTime);

// SAFETY: EncodableCMTime is repr(transparent) over CMTime which is repr(C) with fields {i64, i32, u32, i64}.
unsafe impl Encode for EncodableCMTime {
    const ENCODING: Encoding = Encoding::Struct(
        "CMTime",
        &[
            Encoding::LongLong,
            Encoding::Int,
            Encoding::UInt,
            Encoding::LongLong,
        ],
    );
}

impl From<EncodableCMTime> for CMTime {
    fn from(e: EncodableCMTime) -> CMTime {
        e.0
    }
}

impl From<CMTime> for EncodableCMTime {
    fn from(t: CMTime) -> EncodableCMTime {
        EncodableCMTime(t)
    }
}

pub type CVBufferRef = *mut __CVBuffer;

pub type CVImageBufferRef = CVBufferRef;
pub type CVPixelBufferRef = CVImageBufferRef;
pub type CVPixelBufferLockFlags = u64;
pub type CVReturn = i32;

pub type OSType = FourCharCode;
pub type AVVideoCodecType = NSString;

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {
    pub static AVVideoCodecKey: NSString;
    pub static AVVideoCodecTypeHEVC: AVVideoCodecType;
    pub static AVVideoCodecTypeH264: AVVideoCodecType;
    pub static AVVideoCodecTypeJPEG: AVVideoCodecType;
    pub static AVVideoCodecTypeAppleProRes4444: AVVideoCodecType;
    pub static AVVideoCodecTypeAppleProRes422: AVVideoCodecType;
    pub static AVVideoCodecTypeAppleProRes422HQ: AVVideoCodecType;
    pub static AVVideoCodecTypeAppleProRes422LT: AVVideoCodecType;
    pub static AVVideoCodecTypeAppleProRes422Proxy: AVVideoCodecType;
    pub static AVVideoCodecTypeHEVCWithAlpha: AVVideoCodecType;
    pub static AVVideoCodecHEVC: NSString;
    pub static AVVideoCodecH264: NSString;
    pub static AVVideoCodecJPEG: NSString;
    pub static AVVideoCodecAppleProRes4444: NSString;
    pub static AVVideoCodecAppleProRes422: NSString;
    pub static AVVideoWidthKey: NSString;
    pub static AVVideoHeightKey: NSString;
    pub static AVVideoExpectedSourceFrameRateKey: NSString;

    pub static AVMediaTypeVideo: AVMediaType;
    pub static AVMediaTypeAudio: AVMediaType;
    pub static AVMediaTypeText: AVMediaType;
    pub static AVMediaTypeClosedCaption: AVMediaType;
    pub static AVMediaTypeSubtitle: AVMediaType;
    pub static AVMediaTypeTimecode: AVMediaType;
    pub static AVMediaTypeMetadata: AVMediaType;
    pub static AVMediaTypeMuxed: AVMediaType;
    pub static AVMediaTypeMetadataObject: AVMediaType;
    pub static AVMediaTypeDepthData: AVMediaType;

    pub static AVCaptureLensPositionCurrent: f32;
    pub static AVCaptureExposureTargetBiasCurrent: f32;
    pub static AVCaptureExposureDurationCurrent: CMTime;
    pub static AVCaptureISOCurrent: f32;
}
