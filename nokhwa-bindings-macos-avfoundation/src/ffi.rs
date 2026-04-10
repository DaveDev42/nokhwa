// Local FFI declarations for CoreMedia and CoreVideo C functions.
// Types like AVCaptureDevice, AVCaptureSession, CMTime etc. now come from
// objc2-av-foundation / objc2-core-media / objc2-core-video typed crates.
// This module retains only:
//   - C function bindings not provided by the typed crates
//   - Pixel format / codec constants (not exported as named constants by typed crates)
//   - GCD dispatch_queue helpers (not ObjC objects)

use objc2::encode::{Encode, Encoding};
use objc2::runtime::AnyObject;

// --- CoreMedia types (used in C function signatures) ---
// NOTE: CMTime and CMVideoDimensions are also defined in objc2-core-media.
// These local definitions are required because the C extern functions below
// use raw pointer signatures (CMSampleBufferRef = *mut c_void) that don't
// interoperate with the typed crate's opaque CF wrapper types. Both structs
// are layout-compatible (#[repr(C)] with identical fields).

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

pub type Id = *mut AnyObject;

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

/// Opaque wrapper for GCD dispatch objects (not ObjC NSObjects).
#[repr(transparent)]
#[derive(Clone)]
pub struct DispatchQueue(pub Id);

// SAFETY: DispatchQueue is repr(transparent) over a raw pointer.
unsafe impl Encode for DispatchQueue {
    const ENCODING: Encoding = Encoding::Object;
}

// dispatch_queue_create / dispatch_release are libdispatch (GCD) symbols,
// part of libSystem which is always linked on Apple platforms.
extern "C" {
    pub fn dispatch_queue_create(
        label: *const std::os::raw::c_char,
        attr: *const std::ffi::c_void,
    ) -> DispatchQueue;

    pub fn dispatch_release(object: DispatchQueue);
}

// --- CoreVideo C function bindings ---

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct __CVBuffer {
    _unused: [u8; 0],
}

pub type CVBufferRef = *mut __CVBuffer;
pub type CVImageBufferRef = CVBufferRef;
pub type CVPixelBufferRef = CVImageBufferRef;
pub type CVPixelBufferLockFlags = u64;
pub type CVReturn = i32;
pub type OSType = FourCharCode;

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
