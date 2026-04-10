// Local FFI declarations for CoreMedia and CoreVideo types.
// These replace the `core-media-sys` and `core-video-sys` crate dependencies
// to eliminate their legacy transitive deps (objc 0.2, metal 0.18).

use objc2::encode::{Encode, Encoding};
use objc2::runtime::AnyObject;

// --- CoreMedia types ---

pub type FourCharCode = u32;

pub type CMSampleBufferRef = *mut std::ffi::c_void;
pub type CMBlockBufferRef = *mut std::ffi::c_void;
pub type CMFormatDescriptionRef = *mut std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CMTime {
    pub value: i64,
    pub timescale: i32,
    pub flags: u32,
    pub epoch: i64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CMVideoDimensions {
    pub width: i32,
    pub height: i32,
}

// CoreMedia pixel format / codec constants (FourCharCode values)
const fn fourcc(bytes: &[u8; 4]) -> FourCharCode {
    ((bytes[0] as u32) << 24)
        | ((bytes[1] as u32) << 16)
        | ((bytes[2] as u32) << 8)
        | (bytes[3] as u32)
}

#[allow(non_upper_case_globals)]
pub const kCMPixelFormat_24RGB: FourCharCode = 24;
#[allow(non_upper_case_globals)]
pub const kCMPixelFormat_422YpCbCr8_yuvs: FourCharCode = fourcc(b"yuvs");
#[allow(non_upper_case_globals)]
pub const kCMPixelFormat_8IndexedGray_WhiteIsZero: FourCharCode = 0x0000_0028;
#[allow(non_upper_case_globals)]
pub const kCMVideoCodecType_422YpCbCr8: FourCharCode = fourcc(b"2vuy");
#[allow(non_upper_case_globals)]
pub const kCMVideoCodecType_JPEG: FourCharCode = fourcc(b"jpeg");
#[allow(non_upper_case_globals)]
pub const kCMVideoCodecType_JPEG_OpenDML: FourCharCode = fourcc(b"dmb1");

// CoreVideo pixel format constants
#[allow(non_upper_case_globals)]
pub const kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange: FourCharCode = fourcc(b"420v");
#[allow(non_upper_case_globals)]
pub const kCVPixelFormatType_420YpCbCr8BiPlanarFullRange: FourCharCode = fourcc(b"420f");
#[allow(non_upper_case_globals)]
pub const kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange: FourCharCode = fourcc(b"x420");

// CGFloat is f64 on 64-bit Apple platforms (all modern macOS/iOS)
pub type CGFloat = std::ffi::c_double;

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

pub type AVMediaTypeRaw = NSString;

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

    pub fn CMSampleBufferGetPresentationTimeStamp(sbuf: CMSampleBufferRef) -> CMTime;

    pub fn CMSampleBufferGetImageBuffer(sbuf: CMSampleBufferRef) -> CVImageBufferRef;

    pub fn CMFormatDescriptionGetMediaSubType(desc: CMFormatDescriptionRef) -> FourCharCode;
}

// dispatch_queue_create / dispatch_release are libdispatch (GCD) symbols,
// part of libSystem which is always linked on Apple platforms.
#[allow(non_snake_case)]
extern "C" {
    pub fn dispatch_queue_create(label: *const std::os::raw::c_char, attr: NSObject) -> NSObject;

    pub fn dispatch_release(object: NSObject);
}

#[allow(non_snake_case)]
#[link(name = "CoreVideo", kind = "framework")]
extern "C" {
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

    /// CFStringRef in Apple headers; cast to `*mut AnyObject` at usage site.
    pub static kCVPixelBufferPixelFormatTypeKey: *const std::ffi::c_void;
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct CGPoint {
    pub x: CGFloat,
    pub y: CGFloat,
}

// SAFETY: CGPoint is repr(C) with two CGFloat (f64) fields, matching {dd} encoding.
unsafe impl Encode for CGPoint {
    const ENCODING: Encoding = Encoding::Struct("CGPoint", &[Encoding::Double, Encoding::Double]);
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

    pub static AVMediaTypeVideo: AVMediaTypeRaw;
    pub static AVMediaTypeAudio: AVMediaTypeRaw;
    pub static AVMediaTypeText: AVMediaTypeRaw;
    pub static AVMediaTypeClosedCaption: AVMediaTypeRaw;
    pub static AVMediaTypeSubtitle: AVMediaTypeRaw;
    pub static AVMediaTypeTimecode: AVMediaTypeRaw;
    pub static AVMediaTypeMetadata: AVMediaTypeRaw;
    pub static AVMediaTypeMuxed: AVMediaTypeRaw;
    pub static AVMediaTypeMetadataObject: AVMediaTypeRaw;
    pub static AVMediaTypeDepthData: AVMediaTypeRaw;

    pub static AVCaptureLensPositionCurrent: f32;
    pub static AVCaptureExposureTargetBiasCurrent: f32;
    pub static AVCaptureExposureDurationCurrent: CMTime;
    pub static AVCaptureISOCurrent: f32;
}
