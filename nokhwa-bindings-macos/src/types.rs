use crate::ffi::{
    AVMediaTypeAudio, AVMediaTypeClosedCaption, AVMediaTypeDepthData, AVMediaTypeMetadata,
    AVMediaTypeMetadataObject, AVMediaTypeMuxed, AVMediaTypeSubtitle, AVMediaTypeText,
    AVMediaTypeTimecode, AVMediaTypeVideo,
};
use crate::util::{compare_ns_string, nsstr_to_str, str_to_nsstr};
use nokhwa_core::error::NokhwaError;
use objc2::encode::{Encode, Encoding};
use objc2::runtime::AnyObject;
use std::convert::TryFrom;

#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum AVCaptureDeviceType {
    Dual,
    DualWide,
    Triple,
    WideAngle,
    UltraWide,
    Telephoto,
    TrueDepth,
    External,
}

impl From<AVCaptureDeviceType> for *mut AnyObject {
    fn from(device_type: AVCaptureDeviceType) -> Self {
        match device_type {
            AVCaptureDeviceType::Dual => str_to_nsstr("AVCaptureDeviceTypeBuiltInDualCamera"),
            AVCaptureDeviceType::DualWide => {
                str_to_nsstr("AVCaptureDeviceTypeBuiltInDualWideCamera")
            }
            AVCaptureDeviceType::Triple => str_to_nsstr("AVCaptureDeviceTypeBuiltInTripleCamera"),
            AVCaptureDeviceType::WideAngle => {
                str_to_nsstr("AVCaptureDeviceTypeBuiltInWideAngleCamera")
            }
            AVCaptureDeviceType::UltraWide => {
                str_to_nsstr("AVCaptureDeviceTypeBuiltInUltraWideCamera")
            }
            AVCaptureDeviceType::Telephoto => {
                str_to_nsstr("AVCaptureDeviceTypeBuiltInTelephotoCamera")
            }
            AVCaptureDeviceType::TrueDepth => {
                str_to_nsstr("AVCaptureDeviceTypeBuiltInTrueDepthCamera")
            }
            AVCaptureDeviceType::External => str_to_nsstr("AVCaptureDeviceTypeExternal"),
        }
    }
}

impl AVCaptureDeviceType {
    pub fn into_ns_str(self) -> *mut AnyObject {
        <*mut AnyObject>::from(self)
    }
}

#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum AVMediaType {
    Audio,
    ClosedCaption,
    DepthData,
    Metadata,
    MetadataObject,
    Muxed,
    Subtitle,
    Text,
    Timecode,
    Video,
}

impl From<AVMediaType> for *mut AnyObject {
    fn from(media_type: AVMediaType) -> Self {
        match media_type {
            AVMediaType::Audio => unsafe { AVMediaTypeAudio.0 },
            AVMediaType::ClosedCaption => unsafe { AVMediaTypeClosedCaption.0 },
            AVMediaType::DepthData => unsafe { AVMediaTypeDepthData.0 },
            AVMediaType::Metadata => unsafe { AVMediaTypeMetadata.0 },
            AVMediaType::MetadataObject => unsafe { AVMediaTypeMetadataObject.0 },
            AVMediaType::Muxed => unsafe { AVMediaTypeMuxed.0 },
            AVMediaType::Subtitle => unsafe { AVMediaTypeSubtitle.0 },
            AVMediaType::Text => unsafe { AVMediaTypeText.0 },
            AVMediaType::Timecode => unsafe { AVMediaTypeTimecode.0 },
            AVMediaType::Video => unsafe { AVMediaTypeVideo.0 },
        }
    }
}

impl TryFrom<*mut AnyObject> for AVMediaType {
    type Error = NokhwaError;

    fn try_from(value: *mut AnyObject) -> Result<Self, Self::Error> {
        unsafe {
            if compare_ns_string(value, (AVMediaTypeAudio).clone()) {
                Ok(AVMediaType::Audio)
            } else if compare_ns_string(value, (AVMediaTypeClosedCaption).clone()) {
                Ok(AVMediaType::ClosedCaption)
            } else if compare_ns_string(value, (AVMediaTypeDepthData).clone()) {
                Ok(AVMediaType::DepthData)
            } else if compare_ns_string(value, (AVMediaTypeMetadata).clone()) {
                Ok(AVMediaType::Metadata)
            } else if compare_ns_string(value, (AVMediaTypeMetadataObject).clone()) {
                Ok(AVMediaType::MetadataObject)
            } else if compare_ns_string(value, (AVMediaTypeMuxed).clone()) {
                Ok(AVMediaType::Muxed)
            } else if compare_ns_string(value, (AVMediaTypeSubtitle).clone()) {
                Ok(AVMediaType::Subtitle)
            } else if compare_ns_string(value, (AVMediaTypeText).clone()) {
                Ok(AVMediaType::Text)
            } else if compare_ns_string(value, (AVMediaTypeTimecode).clone()) {
                Ok(AVMediaType::Timecode)
            } else if compare_ns_string(value, (AVMediaTypeVideo).clone()) {
                Ok(AVMediaType::Video)
            } else {
                let name = nsstr_to_str(value);
                Err(NokhwaError::GetPropertyError {
                    property: "AVMediaType".to_string(),
                    error: format!("Invalid AVMediaType {name}"),
                })
            }
        }
    }
}

impl AVMediaType {
    pub fn into_ns_str(self) -> *mut AnyObject {
        <*mut AnyObject>::from(self)
    }
}

#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[repr(isize)]
pub enum AVCaptureDevicePosition {
    Unspecified = 0,
    Back = 1,
    Front = 2,
}

// SAFETY: AVCaptureDevicePosition is repr(isize), same encoding as NSInteger.
unsafe impl Encode for AVCaptureDevicePosition {
    const ENCODING: Encoding = isize::ENCODING;
}

#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[repr(isize)]
pub enum AVAuthorizationStatus {
    NotDetermined = 0,
    Restricted = 1,
    Denied = 2,
    Authorized = 3,
}

// SAFETY: AVAuthorizationStatus is repr(isize), same encoding as NSInteger.
unsafe impl Encode for AVAuthorizationStatus {
    const ENCODING: Encoding = isize::ENCODING;
}
