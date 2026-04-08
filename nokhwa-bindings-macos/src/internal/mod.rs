mod callback;
mod device;
mod helpers;
mod session;

#[allow(non_snake_case)]
pub mod core_media {
    // all of this is stolen from bindgen
    // steal it idc
    use crate::internal::CGFloat;
    use core_media_sys::{
        CMBlockBufferRef, CMFormatDescriptionRef, CMSampleBufferRef, CMTime, CMVideoDimensions,
        FourCharCode,
    };
    use objc::{runtime::Object, Message};
    use std::ops::Deref;

    pub type Id = *mut Object;

    #[repr(transparent)]
    #[derive(Clone)]
    pub struct NSObject(pub Id);
    impl Deref for NSObject {
        type Target = Object;
        fn deref(&self) -> &Self::Target {
            unsafe { &*self.0 }
        }
    }
    unsafe impl Message for NSObject {}
    impl NSObject {
        pub fn alloc() -> Self {
            Self(unsafe { msg_send!(objc::class!(NSObject), alloc) })
        }
    }

    #[repr(transparent)]
    #[derive(Clone)]
    pub struct NSString(pub Id);
    impl Deref for NSString {
        type Target = Object;
        fn deref(&self) -> &Self::Target {
            unsafe { &*self.0 }
        }
    }
    unsafe impl Message for NSString {}
    impl NSString {
        pub fn alloc() -> Self {
            Self(unsafe { msg_send!(objc::class!(NSString), alloc) })
        }
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

        pub fn dispatch_queue_create(
            label: *const std::os::raw::c_char,
            attr: NSObject,
        ) -> NSObject;

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

        pub fn CVPixelBufferGetBaseAddress(
            pixelBuffer: CVPixelBufferRef,
        ) -> *mut std::os::raw::c_void;

        pub fn CVPixelBufferGetPixelFormatType(pixelBuffer: CVPixelBufferRef) -> OSType;
    }

    #[repr(C)]
    #[derive(Clone, Debug, PartialEq, PartialOrd)]
    pub struct CGPoint {
        pub x: CGFloat,
        pub y: CGFloat,
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
}

use crate::core_media::{
    dispatch_queue_create, AVCaptureExposureDurationCurrent, AVCaptureExposureTargetBiasCurrent,
    AVCaptureISOCurrent, AVCaptureWhiteBalanceGains, AVMediaTypeAudio, AVMediaTypeClosedCaption,
    AVMediaTypeDepthData, AVMediaTypeMetadata, AVMediaTypeMetadataObject, AVMediaTypeMuxed,
    AVMediaTypeSubtitle, AVMediaTypeText, AVMediaTypeTimecode, AVMediaTypeVideo, CGPoint,
    CMSampleBufferGetImageBuffer, CMVideoFormatDescriptionGetDimensions, CVImageBufferRef,
    CVPixelBufferGetBaseAddress, CVPixelBufferGetDataSize, CVPixelBufferGetPixelFormatType,
    CVPixelBufferLockBaseAddress, CVPixelBufferUnlockBaseAddress, NSObject, OSType,
};

use block::ConcreteBlock;
use cocoa_foundation::{
    base::Nil,
    foundation::{NSArray, NSDictionary, NSInteger, NSString, NSUInteger},
};
use core_media_sys::{
    kCMPixelFormat_24RGB, kCMPixelFormat_422YpCbCr8_yuvs, kCMPixelFormat_8IndexedGray_WhiteIsZero,
    kCMVideoCodecType_422YpCbCr8, kCMVideoCodecType_JPEG, kCMVideoCodecType_JPEG_OpenDML,
    CMFormatDescriptionGetMediaSubType, CMFormatDescriptionRef, CMSampleBufferRef, CMTime,
    CMVideoDimensions,
};
use core_video_sys::{
    kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange,
    kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
    kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
};
use flume::{Receiver, Sender};
use nokhwa_core::{
    error::NokhwaError,
    types::{
        ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueDescription,
        ControlValueSetter, FrameFormat, KnownCameraControl, KnownCameraControlFlag, Resolution,
    },
};
use objc::runtime::objc_getClass;
use objc::{
    declare::ClassDecl,
    runtime::{Class, Object, Protocol, Sel, BOOL, NO, YES},
};
use once_cell::sync::Lazy;
use std::ffi::CString;
use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::BTreeMap,
    convert::TryFrom,
    error::Error,
    ffi::{c_float, c_void, CStr},
    sync::Arc,
};

pub(crate) const UTF8_ENCODING: usize = 4;
pub(crate) type CGFloat = c_float;

macro_rules! create_boilerplate_impl {
    {
        $( [$class_vis:vis $class_name:ident : $( {$field_vis:vis $field_name:ident : $field_type:ty} ),*] ),+
    } => {
        $(
            $class_vis struct $class_name {
                pub(crate) inner: *mut Object,
                $(
                    $field_vis $field_name : $field_type
                )*
            }

            impl $class_name {
                pub fn inner(&self) -> *mut Object {
                    self.inner
                }
            }
        )+
    };

    {
        $( [$class_vis:vis $class_name:ident ] ),+
    } => {
        $(
            $class_vis struct $class_name {
                pub(crate) inner: *mut Object,
            }

            impl $class_name {
                pub fn inner(&self) -> *mut Object {
                    self.inner
                }
            }

            impl From<*mut Object> for $class_name {
                fn from(obj: *mut Object) -> Self {
                    $class_name {
                        inner: obj,
                    }
                }
            }
        )+
    };
}

pub(crate) use create_boilerplate_impl;

pub(crate) use callback::*;
pub use device::*;
pub(crate) use helpers::*;
pub use helpers::{CompressionData, DataPipe};
pub use session::*;
