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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn av_capture_device_type_local_all_variants_convert() {
        let variants = [
            AVCaptureDeviceTypeLocal::Dual,
            AVCaptureDeviceTypeLocal::DualWide,
            AVCaptureDeviceTypeLocal::Triple,
            AVCaptureDeviceTypeLocal::WideAngle,
            AVCaptureDeviceTypeLocal::UltraWide,
            AVCaptureDeviceTypeLocal::Telephoto,
            AVCaptureDeviceTypeLocal::TrueDepth,
            AVCaptureDeviceTypeLocal::External,
        ];
        for variant in variants {
            // Should not panic — validates the static reference is valid
            let _ = variant.as_av_capture_device_type();
        }
    }

    #[test]
    fn av_media_type_local_all_variants_convert() {
        let variants = [
            AVMediaTypeLocal::Audio,
            AVMediaTypeLocal::ClosedCaption,
            AVMediaTypeLocal::DepthData,
            AVMediaTypeLocal::Metadata,
            AVMediaTypeLocal::MetadataObject,
            AVMediaTypeLocal::Muxed,
            AVMediaTypeLocal::Subtitle,
            AVMediaTypeLocal::Text,
            AVMediaTypeLocal::Timecode,
            AVMediaTypeLocal::Video,
        ];
        for variant in variants {
            let _ = variant.to_av_media_type();
        }
    }

    #[test]
    fn av_media_type_local_roundtrip_via_nsstring() {
        let variants = [
            AVMediaTypeLocal::Audio,
            AVMediaTypeLocal::ClosedCaption,
            AVMediaTypeLocal::DepthData,
            AVMediaTypeLocal::Metadata,
            AVMediaTypeLocal::MetadataObject,
            AVMediaTypeLocal::Muxed,
            AVMediaTypeLocal::Subtitle,
            AVMediaTypeLocal::Text,
            AVMediaTypeLocal::Timecode,
            AVMediaTypeLocal::Video,
        ];
        for variant in variants {
            let av_type: &NSString = variant.to_av_media_type();
            let back = AVMediaTypeLocal::try_from(av_type)
                .unwrap_or_else(|_| panic!("roundtrip failed for {variant:?}"));
            assert_eq!(variant, back);
        }
    }

    #[test]
    fn av_media_type_local_invalid_nsstring() {
        let invalid = NSString::from_str("com.apple.media-type.bogus");
        let result = AVMediaTypeLocal::try_from(invalid.as_ref());
        assert!(result.is_err());
    }

    #[test]
    fn av_authorization_status_repr_values() {
        assert_eq!(AVAuthorizationStatus::NotDetermined as isize, 0);
        assert_eq!(AVAuthorizationStatus::Restricted as isize, 1);
        assert_eq!(AVAuthorizationStatus::Denied as isize, 2);
        assert_eq!(AVAuthorizationStatus::Authorized as isize, 3);
    }

    #[test]
    fn av_capture_device_type_local_traits() {
        let a = AVCaptureDeviceTypeLocal::WideAngle;
        let b = a; // Copy
        assert_eq!(a, b); // Eq
        assert!(AVCaptureDeviceTypeLocal::Dual < AVCaptureDeviceTypeLocal::External);
        // Ord
    }

    #[test]
    fn av_media_type_local_traits() {
        let a = AVMediaTypeLocal::Video;
        let b = a;
        assert_eq!(a, b);
    }
}
