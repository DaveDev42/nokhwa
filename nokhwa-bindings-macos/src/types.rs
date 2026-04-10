use nokhwa_core::error::NokhwaError;
use objc2_av_foundation::{
    AVCaptureDeviceType, AVCaptureDeviceTypeBuiltInDualCamera,
    AVCaptureDeviceTypeBuiltInDualWideCamera, AVCaptureDeviceTypeBuiltInTelephotoCamera,
    AVCaptureDeviceTypeBuiltInTripleCamera, AVCaptureDeviceTypeBuiltInTrueDepthCamera,
    AVCaptureDeviceTypeBuiltInUltraWideCamera, AVCaptureDeviceTypeBuiltInWideAngleCamera,
    AVCaptureDeviceTypeExternal, AVMediaType, AVMediaTypeAudio, AVMediaTypeClosedCaption,
    AVMediaTypeDepthData, AVMediaTypeMetadata, AVMediaTypeMetadataObject, AVMediaTypeMuxed,
    AVMediaTypeSubtitle, AVMediaTypeText, AVMediaTypeTimecode, AVMediaTypeVideo,
};
use objc2_foundation::NSString;

#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum AVCaptureDeviceTypeLocal {
    Dual,
    DualWide,
    Triple,
    WideAngle,
    UltraWide,
    Telephoto,
    TrueDepth,
    External,
}

impl AVCaptureDeviceTypeLocal {
    pub fn as_av_capture_device_type(self) -> &'static AVCaptureDeviceType {
        unsafe {
            match self {
                Self::Dual => AVCaptureDeviceTypeBuiltInDualCamera,
                Self::DualWide => AVCaptureDeviceTypeBuiltInDualWideCamera,
                Self::Triple => AVCaptureDeviceTypeBuiltInTripleCamera,
                Self::WideAngle => AVCaptureDeviceTypeBuiltInWideAngleCamera,
                Self::UltraWide => AVCaptureDeviceTypeBuiltInUltraWideCamera,
                Self::Telephoto => AVCaptureDeviceTypeBuiltInTelephotoCamera,
                Self::TrueDepth => AVCaptureDeviceTypeBuiltInTrueDepthCamera,
                Self::External => AVCaptureDeviceTypeExternal,
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum AVMediaTypeLocal {
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

impl AVMediaTypeLocal {
    pub fn to_av_media_type(self) -> &'static AVMediaType {
        // unwrap(): objc2 wraps extern statics as Option<&T> for weak-linking support,
        // but these AVMediaType constants are always available on supported Apple platforms.
        unsafe {
            match self {
                Self::Audio => AVMediaTypeAudio.unwrap(),
                Self::ClosedCaption => AVMediaTypeClosedCaption.unwrap(),
                Self::DepthData => AVMediaTypeDepthData.unwrap(),
                Self::Metadata => AVMediaTypeMetadata.unwrap(),
                Self::MetadataObject => AVMediaTypeMetadataObject.unwrap(),
                Self::Muxed => AVMediaTypeMuxed.unwrap(),
                Self::Subtitle => AVMediaTypeSubtitle.unwrap(),
                Self::Text => AVMediaTypeText.unwrap(),
                Self::Timecode => AVMediaTypeTimecode.unwrap(),
                Self::Video => AVMediaTypeVideo.unwrap(),
            }
        }
    }
}

impl TryFrom<&NSString> for AVMediaTypeLocal {
    type Error = NokhwaError;

    fn try_from(value: &NSString) -> Result<Self, Self::Error> {
        // unwrap(): see comment in to_av_media_type() — always non-null on supported platforms.
        unsafe {
            if value.isEqualToString(AVMediaTypeAudio.unwrap()) {
                Ok(Self::Audio)
            } else if value.isEqualToString(AVMediaTypeClosedCaption.unwrap()) {
                Ok(Self::ClosedCaption)
            } else if value.isEqualToString(AVMediaTypeDepthData.unwrap()) {
                Ok(Self::DepthData)
            } else if value.isEqualToString(AVMediaTypeMetadata.unwrap()) {
                Ok(Self::Metadata)
            } else if value.isEqualToString(AVMediaTypeMetadataObject.unwrap()) {
                Ok(Self::MetadataObject)
            } else if value.isEqualToString(AVMediaTypeMuxed.unwrap()) {
                Ok(Self::Muxed)
            } else if value.isEqualToString(AVMediaTypeSubtitle.unwrap()) {
                Ok(Self::Subtitle)
            } else if value.isEqualToString(AVMediaTypeText.unwrap()) {
                Ok(Self::Text)
            } else if value.isEqualToString(AVMediaTypeTimecode.unwrap()) {
                Ok(Self::Timecode)
            } else if value.isEqualToString(AVMediaTypeVideo.unwrap()) {
                Ok(Self::Video)
            } else {
                Err(NokhwaError::GetPropertyError {
                    property: "AVMediaType".to_string(),
                    error: format!("Invalid AVMediaType {value}"),
                })
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[repr(isize)]
pub enum AVAuthorizationStatus {
    NotDetermined = 0,
    Restricted = 1,
    Denied = 2,
    Authorized = 3,
}
