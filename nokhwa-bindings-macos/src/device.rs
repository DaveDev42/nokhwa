use crate::ffi::{CMFormatDescriptionGetMediaSubType, CMVideoFormatDescriptionGetDimensions};
use crate::ffi::{CMFormatDescriptionRef, CMVideoDimensions};
use crate::session::{discovery_session_devices, discovery_session_with_types};
use crate::types::{AVCaptureDeviceTypeLocal, AVMediaTypeLocal};
use crate::util::raw_fcc_to_frameformat;
use nokhwa_core::{
    error::NokhwaError,
    types::{
        ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueDescription,
        ControlValueSetter, FrameFormat, KnownCameraControl, KnownCameraControlFlag, Resolution,
    },
};
use objc2::rc::Retained;
use objc2::Message;
use objc2_av_foundation::{
    AVCaptureDevice, AVCaptureDeviceFormat, AVCaptureExposureDurationCurrent,
    AVCaptureExposureMode, AVCaptureExposureTargetBiasCurrent, AVCaptureFocusMode,
    AVCaptureISOCurrent, AVCaptureTorchMode, AVCaptureWhiteBalanceGains, AVCaptureWhiteBalanceMode,
    AVFrameRateRange,
};
use objc2_core_foundation::{CGFloat, CGPoint};
use objc2_core_media::CMTime;
use std::{cmp::Ordering, collections::BTreeMap, convert::TryFrom, ffi::c_float};

pub fn query_avfoundation() -> Result<Vec<CameraInfo>, NokhwaError> {
    let session = discovery_session_with_types(&[
        AVCaptureDeviceTypeLocal::UltraWide,
        AVCaptureDeviceTypeLocal::WideAngle,
        AVCaptureDeviceTypeLocal::Telephoto,
        AVCaptureDeviceTypeLocal::TrueDepth,
        AVCaptureDeviceTypeLocal::External,
    ])?;
    Ok(discovery_session_devices(&session))
}

pub fn get_raw_device_info(index: CameraIndex, device: &AVCaptureDevice) -> CameraInfo {
    let name = unsafe { device.localizedName() };
    let manufacturer = unsafe { device.manufacturer() };
    let position = unsafe { device.position() };
    let lens_aperture: c_float = unsafe { device.lensAperture() };
    let device_type = unsafe { device.deviceType() };
    let model_id = unsafe { device.modelID() };
    let description = format!(
        "{}: {} - {}, {:?} f{}",
        manufacturer, model_id, device_type, position, lens_aperture
    );
    let misc = unsafe { device.uniqueID() };

    CameraInfo::new(
        name.to_string().as_ref(),
        &description,
        misc.to_string().as_ref(),
        index,
    )
}

/// Wrapper around `AVFrameRateRange` with a public `inner()` accessor.
pub struct AVFrameRateRangeWrapper {
    inner: Retained<AVFrameRateRange>,
}

impl AVFrameRateRangeWrapper {
    pub fn new(inner: Retained<AVFrameRateRange>) -> Self {
        Self { inner }
    }

    pub fn max(&self) -> f64 {
        unsafe { self.inner.maxFrameRate() }
    }

    pub fn min(&self) -> f64 {
        unsafe { self.inner.minFrameRate() }
    }

    pub fn inner(&self) -> &AVFrameRateRange {
        &self.inner
    }
}

#[derive(Debug)]
pub struct AVCaptureDeviceFormatWrapper {
    /// Retained to prevent deallocation while the wrapper is alive.
    #[allow(dead_code)]
    pub(crate) internal: Retained<AVCaptureDeviceFormat>,
    pub resolution: CMVideoDimensions,
    pub fps_list: Vec<f64>,
    pub fourcc: FrameFormat,
}

impl AVCaptureDeviceFormatWrapper {
    pub fn try_from_format(format: &AVCaptureDeviceFormat) -> Result<Self, NokhwaError> {
        let media_type = unsafe { format.mediaType() };
        let media_type_local = AVMediaTypeLocal::try_from(media_type.as_ref())?;
        if media_type_local != AVMediaTypeLocal::Video {
            return Err(NokhwaError::StructureError {
                structure: "AVMediaType".to_string(),
                error: "Not Video".to_string(),
            });
        }

        let frame_rate_ranges = unsafe { format.videoSupportedFrameRateRanges() };
        let mut fps_list: Vec<f64> = Vec::new();
        for i in 0..frame_rate_ranges.count() {
            let range = frame_rate_ranges.objectAtIndex(i);
            let min_fps = unsafe { range.minFrameRate() };
            let max_fps = unsafe { range.maxFrameRate() };
            if min_fps != 0_f64 && min_fps != 1_f64 {
                fps_list.push(min_fps);
            }
            fps_list.push(max_fps);
        }
        fps_list.sort_by(|n, m| n.partial_cmp(m).unwrap_or(Ordering::Equal));
        fps_list.dedup();

        let description_obj = unsafe { format.formatDescription() };
        let description_ref = &*description_obj as *const _ as CMFormatDescriptionRef;
        let resolution = unsafe { CMVideoFormatDescriptionGetDimensions(description_ref) };
        let fcc_raw = unsafe { CMFormatDescriptionGetMediaSubType(description_ref) };

        #[allow(non_upper_case_globals)]
        let fourcc = match raw_fcc_to_frameformat(fcc_raw) {
            Some(fcc) => fcc,
            None => {
                return Err(NokhwaError::StructureError {
                    structure: "FourCharCode".to_string(),
                    error: format!("Unknown FourCharCode {fcc_raw:?}"),
                })
            }
        };

        Ok(AVCaptureDeviceFormatWrapper {
            internal: format.retain(),
            resolution,
            fps_list,
            fourcc,
        })
    }
}

pub struct AVCaptureDeviceWrapper {
    inner: Retained<AVCaptureDevice>,
    device: CameraInfo,
    locked: bool,
}

impl AVCaptureDeviceWrapper {
    pub fn inner(&self) -> &AVCaptureDevice {
        &self.inner
    }
}

impl AVCaptureDeviceWrapper {
    pub fn new(index: &CameraIndex) -> Result<Self, NokhwaError> {
        match &index {
            CameraIndex::Index(idx) => {
                let devices = query_avfoundation()?;

                match devices.get(*idx as usize) {
                    Some(device) => Ok(Self::from_id(&device.misc(), Some(index.clone()))?),
                    None => Err(NokhwaError::OpenDeviceError(
                        idx.to_string(),
                        "Not Found".to_string(),
                    )),
                }
            }
            CameraIndex::String(id) => Ok(Self::from_id(id, None)?),
        }
    }

    pub fn from_id(id: &str, index_hint: Option<CameraIndex>) -> Result<Self, NokhwaError> {
        let nsstr_id = objc2_foundation::NSString::from_str(id);
        let capture = unsafe { AVCaptureDevice::deviceWithUniqueID(&nsstr_id) };
        let capture = capture.ok_or_else(|| {
            NokhwaError::OpenDeviceError(id.to_string(), "Device is null".to_string())
        })?;

        let camera_info = get_raw_device_info(
            index_hint.unwrap_or_else(|| CameraIndex::String(id.to_string())),
            &capture,
        );

        Ok(AVCaptureDeviceWrapper {
            inner: capture,
            device: camera_info,
            locked: false,
        })
    }

    pub fn info(&self) -> &CameraInfo {
        &self.device
    }

    pub fn supported_formats_raw(&self) -> Result<Vec<AVCaptureDeviceFormatWrapper>, NokhwaError> {
        let formats = unsafe { self.inner.formats() };
        let mut result = Vec::new();
        for i in 0..formats.count() {
            let format = formats.objectAtIndex(i);
            match AVCaptureDeviceFormatWrapper::try_from_format(&format) {
                Ok(f) => result.push(f),
                Err(_) => continue, // skip unsupported formats
            }
        }
        Ok(result)
    }

    pub fn supported_formats(&self) -> Result<Vec<CameraFormat>, NokhwaError> {
        Ok(self
            .supported_formats_raw()?
            .iter()
            .flat_map(|av_fmt| {
                let resolution = av_fmt.resolution;
                av_fmt.fps_list.iter().map(move |fps_f64| {
                    let fps = *fps_f64 as u32;
                    let resolution =
                        Resolution::new(resolution.width as u32, resolution.height as u32);
                    CameraFormat::new(resolution, av_fmt.fourcc, fps)
                })
            })
            .filter(|x| x.frame_rate() != 0)
            .collect())
    }

    pub fn already_in_use(&self) -> bool {
        unsafe { self.inner.isInUseByAnotherApplication() }
    }

    pub fn is_suspended(&self) -> bool {
        unsafe { self.inner.isSuspended() }
    }

    pub fn lock(&mut self) -> Result<(), NokhwaError> {
        if self.locked {
            return Ok(());
        }
        if self.already_in_use() {
            return Err(NokhwaError::InitializeError {
                backend: ApiBackend::AVFoundation,
                error: "Already in use".to_string(),
            });
        }
        unsafe {
            self.inner.lockForConfiguration().map_err(|e| {
                let desc = e.localizedDescription();
                NokhwaError::SetPropertyError {
                    property: "lockForConfiguration".to_string(),
                    value: "Locked".to_string(),
                    error: desc.to_string(),
                }
            })?;
        }
        self.locked = true;
        Ok(())
    }

    pub fn unlock(&mut self) {
        if self.locked {
            self.locked = false;
            unsafe { self.inner.unlockForConfiguration() }
        }
    }

    pub fn set_all(&mut self, descriptor: CameraFormat) -> Result<(), NokhwaError> {
        self.lock()?;
        let formats = unsafe { self.inner.formats() };

        let mut selected_format: Option<Retained<AVCaptureDeviceFormat>> = None;
        let mut selected_min_frame_duration: Option<CMTime> = None;

        for i in 0..formats.count() {
            let format = formats.objectAtIndex(i);
            let fmt_desc = unsafe { format.formatDescription() };
            let fmt_desc_ref = &*fmt_desc as *const _ as CMFormatDescriptionRef;
            let dimensions = unsafe { CMVideoFormatDescriptionGetDimensions(fmt_desc_ref) };

            if dimensions.height == descriptor.resolution().height() as i32
                && dimensions.width == descriptor.resolution().width() as i32
            {
                let ranges = unsafe { format.videoSupportedFrameRateRanges() };
                for j in 0..ranges.count() {
                    let range = ranges.objectAtIndex(j);
                    let max_fps = unsafe { range.maxFrameRate() };
                    if (f64::from(descriptor.frame_rate()) - max_fps).abs() < 0.999 {
                        selected_format = Some(format.clone());
                        selected_min_frame_duration = Some(unsafe { range.minFrameDuration() });
                        break;
                    }
                }
                if selected_format.is_some() {
                    break;
                }
            }
        }

        let (format, min_duration) = match (selected_format, selected_min_frame_duration) {
            (Some(f), Some(d)) => (f, d),
            _ => {
                return Err(NokhwaError::SetPropertyError {
                    property: "CameraFormat".to_string(),
                    value: descriptor.to_string(),
                    error: "Not Found/Rejected/Unsupported".to_string(),
                });
            }
        };

        unsafe {
            self.inner.setActiveFormat(&format);
            self.inner.setActiveVideoMinFrameDuration(min_duration);
            self.inner.setActiveVideoMaxFrameDuration(min_duration);
        }
        self.unlock();
        Ok(())
    }

    pub fn get_controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
        let active_format = unsafe { self.inner.activeFormat() };
        let mut controls = vec![];

        // Focus modes
        let focus_current = unsafe { self.inner.focusMode() };
        let focus_locked = unsafe { self.inner.isFocusModeSupported(AVCaptureFocusMode::Locked) };
        let focus_auto = unsafe {
            self.inner
                .isFocusModeSupported(AVCaptureFocusMode::AutoFocus)
        };
        let focus_continuous = unsafe {
            self.inner
                .isFocusModeSupported(AVCaptureFocusMode::ContinuousAutoFocus)
        };

        {
            let mut supported_focus_values = vec![];
            if focus_locked {
                supported_focus_values.push(0)
            }
            if focus_auto {
                supported_focus_values.push(1)
            }
            if focus_continuous {
                supported_focus_values.push(2)
            }

            controls.push(CameraControl::new(
                KnownCameraControl::Focus,
                "FocusMode".to_string(),
                ControlValueDescription::Enum {
                    value: focus_current.0 as i64,
                    possible: supported_focus_values,
                    default: focus_current.0 as i64,
                },
                vec![],
                true,
            ));
        }

        let focus_poi_supported = unsafe { self.inner.isFocusPointOfInterestSupported() };
        let focus_poi = unsafe { self.inner.focusPointOfInterest() };

        controls.push(CameraControl::new(
            KnownCameraControl::Other(0),
            "FocusPointOfInterest".to_string(),
            ControlValueDescription::Point {
                value: (focus_poi.x, focus_poi.y),
                default: (0.5, 0.5),
            },
            if !focus_poi_supported {
                vec![
                    KnownCameraControlFlag::Disabled,
                    KnownCameraControlFlag::ReadOnly,
                ]
            } else {
                vec![]
            },
            focus_auto || focus_continuous,
        ));

        let focus_manual = unsafe { self.inner.isLockingFocusWithCustomLensPositionSupported() };
        let focus_lenspos: c_float = unsafe { self.inner.lensPosition() };

        controls.push(CameraControl::new(
            KnownCameraControl::Other(1),
            "FocusManualLensPosition".to_string(),
            ControlValueDescription::FloatRange {
                min: 0.0,
                max: 1.0,
                value: focus_lenspos as f64,
                step: f64::MIN_POSITIVE,
                default: 1.0,
            },
            if focus_manual {
                vec![]
            } else {
                vec![
                    KnownCameraControlFlag::Disabled,
                    KnownCameraControlFlag::ReadOnly,
                ]
            },
            focus_manual,
        ));

        // Exposure modes
        let exposure_current = unsafe { self.inner.exposureMode() };
        let exposure_locked = unsafe {
            self.inner
                .isExposureModeSupported(AVCaptureExposureMode::Locked)
        };
        let exposure_auto = unsafe {
            self.inner
                .isExposureModeSupported(AVCaptureExposureMode::AutoExpose)
        };
        let exposure_continuous = unsafe {
            self.inner
                .isExposureModeSupported(AVCaptureExposureMode::ContinuousAutoExposure)
        };
        let exposure_custom = unsafe {
            self.inner
                .isExposureModeSupported(AVCaptureExposureMode::Custom)
        };

        {
            let mut supported_exposure_values = vec![];
            if exposure_locked {
                supported_exposure_values.push(0);
            }
            if exposure_auto {
                supported_exposure_values.push(1);
            }
            if exposure_continuous {
                supported_exposure_values.push(2);
            }
            if exposure_custom {
                supported_exposure_values.push(3);
            }

            controls.push(CameraControl::new(
                KnownCameraControl::Exposure,
                "ExposureMode".to_string(),
                ControlValueDescription::Enum {
                    value: exposure_current.0 as i64,
                    possible: supported_exposure_values,
                    default: exposure_current.0 as i64,
                },
                vec![],
                true,
            ));
        }

        let exposure_poi_supported = unsafe { self.inner.isExposurePointOfInterestSupported() };
        let exposure_poi = unsafe { self.inner.exposurePointOfInterest() };

        controls.push(CameraControl::new(
            KnownCameraControl::Other(2),
            "ExposurePointOfInterest".to_string(),
            ControlValueDescription::Point {
                value: (exposure_poi.x, exposure_poi.y),
                default: (0.5, 0.5),
            },
            if !exposure_poi_supported {
                vec![
                    KnownCameraControlFlag::Disabled,
                    KnownCameraControlFlag::ReadOnly,
                ]
            } else {
                vec![]
            },
            focus_auto || focus_continuous,
        ));

        let exposure_face_driven_supported =
            unsafe { self.inner.isFaceDrivenAutoExposureEnabled() };
        let exposure_face_driven = unsafe {
            self.inner
                .automaticallyAdjustsFaceDrivenAutoExposureEnabled()
        };

        controls.push(CameraControl::new(
            KnownCameraControl::Other(3),
            "ExposureFaceDriven".to_string(),
            ControlValueDescription::Boolean {
                value: exposure_face_driven,
                default: false,
            },
            if !exposure_face_driven_supported {
                vec![
                    KnownCameraControlFlag::Disabled,
                    KnownCameraControlFlag::ReadOnly,
                ]
            } else {
                vec![]
            },
            exposure_poi_supported,
        ));

        let exposure_bias: c_float = unsafe { self.inner.exposureTargetBias() };
        let exposure_bias_min: c_float = unsafe { self.inner.minExposureTargetBias() };
        let exposure_bias_max: c_float = unsafe { self.inner.maxExposureTargetBias() };

        controls.push(CameraControl::new(
            KnownCameraControl::Other(4),
            "ExposureBiasTarget".to_string(),
            ControlValueDescription::FloatRange {
                min: exposure_bias_min as f64,
                max: exposure_bias_max as f64,
                value: exposure_bias as f64,
                step: f32::MIN_POSITIVE as f64,
                default: unsafe { AVCaptureExposureTargetBiasCurrent } as f64,
            },
            vec![],
            true,
        ));

        let exposure_duration = unsafe { self.inner.exposureDuration() };
        let exposure_duration_min = unsafe { active_format.minExposureDuration() };
        let exposure_duration_max = unsafe { active_format.maxExposureDuration() };

        controls.push(CameraControl::new(
            KnownCameraControl::Gamma,
            "ExposureDuration".to_string(),
            ControlValueDescription::IntegerRange {
                min: exposure_duration_min.value,
                max: exposure_duration_max.value,
                value: exposure_duration.value,
                step: 1,
                default: unsafe { AVCaptureExposureDurationCurrent.value },
            },
            if exposure_custom {
                vec![
                    KnownCameraControlFlag::ReadOnly,
                    KnownCameraControlFlag::Volatile,
                ]
            } else {
                vec![KnownCameraControlFlag::Volatile]
            },
            exposure_custom,
        ));

        let exposure_iso: c_float = unsafe { self.inner.ISO() };
        let exposure_iso_min: c_float = unsafe { active_format.minISO() };
        let exposure_iso_max: c_float = unsafe { active_format.maxISO() };

        controls.push(CameraControl::new(
            KnownCameraControl::Brightness,
            "ExposureISO".to_string(),
            ControlValueDescription::FloatRange {
                min: exposure_iso_min as f64,
                max: exposure_iso_max as f64,
                value: exposure_iso as f64,
                step: f32::MIN_POSITIVE as f64,
                default: unsafe { AVCaptureISOCurrent } as f64,
            },
            if exposure_custom {
                vec![
                    KnownCameraControlFlag::ReadOnly,
                    KnownCameraControlFlag::Volatile,
                ]
            } else {
                vec![KnownCameraControlFlag::Volatile]
            },
            exposure_custom,
        ));

        let lens_aperture: c_float = unsafe { self.inner.lensAperture() };

        controls.push(CameraControl::new(
            KnownCameraControl::Iris,
            "LensAperture".to_string(),
            ControlValueDescription::Float {
                value: lens_aperture as f64,
                default: lens_aperture as f64,
                step: lens_aperture as f64,
            },
            vec![KnownCameraControlFlag::ReadOnly],
            false,
        ));

        // White balance
        let white_balance_current = unsafe { self.inner.whiteBalanceMode() };
        let white_balance_manual = unsafe {
            self.inner
                .isWhiteBalanceModeSupported(AVCaptureWhiteBalanceMode::Locked)
        };
        let white_balance_auto = unsafe {
            self.inner
                .isWhiteBalanceModeSupported(AVCaptureWhiteBalanceMode::AutoWhiteBalance)
        };
        let white_balance_continuous = unsafe {
            self.inner
                .isWhiteBalanceModeSupported(AVCaptureWhiteBalanceMode::ContinuousAutoWhiteBalance)
        };

        {
            let mut possible = vec![];
            if white_balance_manual {
                possible.push(0);
            }
            if white_balance_auto {
                possible.push(1);
            }
            if white_balance_continuous {
                possible.push(2);
            }

            controls.push(CameraControl::new(
                KnownCameraControl::WhiteBalance,
                "WhiteBalanceMode".to_string(),
                ControlValueDescription::Enum {
                    value: white_balance_current.0 as i64,
                    possible,
                    default: 0,
                },
                vec![],
                true,
            ));
        }

        let white_balance_gains = unsafe { self.inner.deviceWhiteBalanceGains() };
        let white_balance_default = unsafe { self.inner.grayWorldDeviceWhiteBalanceGains() };
        let white_balance_max_scalar: c_float = unsafe { self.inner.maxWhiteBalanceGain() };
        let white_balance_max = AVCaptureWhiteBalanceGains {
            redGain: white_balance_max_scalar,
            greenGain: white_balance_max_scalar,
            blueGain: white_balance_max_scalar,
        };
        let white_balance_gain_supported = unsafe {
            self.inner
                .isLockingWhiteBalanceWithCustomDeviceGainsSupported()
        };

        controls.push(CameraControl::new(
            KnownCameraControl::Gain,
            "WhiteBalanceGain".to_string(),
            ControlValueDescription::RGB {
                value: (
                    white_balance_gains.redGain as f64,
                    white_balance_gains.greenGain as f64,
                    white_balance_gains.blueGain as f64,
                ),
                max: (
                    white_balance_max.redGain as f64,
                    white_balance_max.greenGain as f64,
                    white_balance_max.blueGain as f64,
                ),
                default: (
                    white_balance_default.redGain as f64,
                    white_balance_default.greenGain as f64,
                    white_balance_default.blueGain as f64,
                ),
            },
            if !white_balance_gain_supported {
                vec![
                    KnownCameraControlFlag::Disabled,
                    KnownCameraControlFlag::ReadOnly,
                ]
            } else {
                vec![]
            },
            white_balance_gain_supported,
        ));

        // Torch
        let has_torch = unsafe { self.inner.isTorchAvailable() };
        let torch_off = unsafe { self.inner.isTorchModeSupported(AVCaptureTorchMode::Off) };
        let torch_on = unsafe { self.inner.isTorchModeSupported(AVCaptureTorchMode::On) };
        let torch_auto = unsafe { self.inner.isTorchModeSupported(AVCaptureTorchMode::Auto) };

        {
            let mut possible = vec![];
            if torch_off {
                possible.push(0);
            }
            if torch_on {
                possible.push(1);
            }
            if torch_auto {
                possible.push(2);
            }

            let torch_mode_current = unsafe { self.inner.torchMode() };

            controls.push(CameraControl::new(
                KnownCameraControl::Other(5),
                "TorchMode".to_string(),
                ControlValueDescription::Enum {
                    value: torch_mode_current.0 as i64,
                    possible,
                    default: 0,
                },
                if !has_torch {
                    vec![
                        KnownCameraControlFlag::Disabled,
                        KnownCameraControlFlag::ReadOnly,
                    ]
                } else {
                    vec![]
                },
                has_torch,
            ));
        }

        // Low light boost
        let has_llb = unsafe { self.inner.isLowLightBoostSupported() };
        let llb_enabled = unsafe { self.inner.isLowLightBoostEnabled() };

        {
            controls.push(CameraControl::new(
                KnownCameraControl::BacklightComp,
                "LowLightCompensation".to_string(),
                ControlValueDescription::Boolean {
                    value: llb_enabled,
                    default: false,
                },
                if !has_llb {
                    vec![
                        KnownCameraControlFlag::Disabled,
                        KnownCameraControlFlag::ReadOnly,
                    ]
                } else {
                    vec![]
                },
                has_llb,
            ));
        }

        // Zoom
        let zoom_current: CGFloat = unsafe { self.inner.videoZoomFactor() };
        let zoom_min: CGFloat = unsafe { self.inner.minAvailableVideoZoomFactor() };
        let zoom_max: CGFloat = unsafe { self.inner.maxAvailableVideoZoomFactor() };

        controls.push(CameraControl::new(
            KnownCameraControl::Zoom,
            "Zoom".to_string(),
            ControlValueDescription::FloatRange {
                min: zoom_min,
                max: zoom_max,
                value: zoom_current,
                step: f32::MIN_POSITIVE as f64,
                default: 1.0,
            },
            vec![],
            true,
        ));

        // Geometric distortion correction
        let distortion_correction_supported =
            unsafe { self.inner.isGeometricDistortionCorrectionSupported() };
        let distortion_correction_current_value =
            unsafe { self.inner.isGeometricDistortionCorrectionEnabled() };

        controls.push(CameraControl::new(
            KnownCameraControl::Other(6),
            "DistortionCorrection".to_string(),
            ControlValueDescription::Boolean {
                value: distortion_correction_current_value,
                default: false,
            },
            if !distortion_correction_supported {
                vec![
                    KnownCameraControlFlag::ReadOnly,
                    KnownCameraControlFlag::Disabled,
                ]
            } else {
                vec![]
            },
            distortion_correction_supported,
        ));

        Ok(controls)
    }

    pub fn set_control(
        &mut self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        let rc = self.get_controls()?;
        let controls = rc
            .iter()
            .map(|cc| (cc.control(), cc))
            .collect::<BTreeMap<_, _>>();

        match id {
            KnownCameraControl::Brightness => {
                let isoctrl = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                check_control_flags(isoctrl, &id, &value)?;

                let current_duration = unsafe { AVCaptureExposureDurationCurrent };
                let new_iso = *value.as_float().ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Expected float".to_string(),
                })? as c_float;

                if !isoctrl.description().verify_setter(&value) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Failed to verify value".to_string(),
                    });
                }

                unsafe {
                    self.inner
                        .setExposureModeCustomWithDuration_ISO_completionHandler(
                            current_duration,
                            new_iso,
                            None,
                        );
                }

                Ok(())
            }
            KnownCameraControl::Gamma => {
                let duration_ctrl = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                check_control_flags(duration_ctrl, &id, &value)?;

                let current_duration = unsafe { self.inner.exposureDuration() };
                let current_iso = unsafe { AVCaptureISOCurrent };
                let new_duration = CMTime {
                    value: *value.as_integer().ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Expected i64".to_string(),
                    })?,
                    timescale: current_duration.timescale,
                    flags: current_duration.flags,
                    epoch: current_duration.epoch,
                };

                if !duration_ctrl.description().verify_setter(&value) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Failed to verify value".to_string(),
                    });
                }

                unsafe {
                    self.inner
                        .setExposureModeCustomWithDuration_ISO_completionHandler(
                            new_duration,
                            current_iso,
                            None,
                        );
                }

                Ok(())
            }
            KnownCameraControl::WhiteBalance => {
                let wb_enum_value = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                check_control_flags(wb_enum_value, &id, &value)?;

                let setter = *value.as_enum().ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Expected Enum".to_string(),
                })?;

                if !wb_enum_value.description().verify_setter(&value) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Failed to verify value".to_string(),
                    });
                }

                unsafe {
                    self.inner
                        .setWhiteBalanceMode(AVCaptureWhiteBalanceMode(setter as isize));
                }

                Ok(())
            }
            KnownCameraControl::BacklightComp => {
                let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                check_control_flags(ctrlvalue, &id, &value)?;

                let setter = *value.as_boolean().ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Expected Boolean".to_string(),
                })?;

                if !ctrlvalue.description().verify_setter(&value) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Failed to verify value".to_string(),
                    });
                }

                unsafe {
                    self.inner
                        .setAutomaticallyEnablesLowLightBoostWhenAvailable(setter);
                }

                Ok(())
            }
            KnownCameraControl::Gain => {
                let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                check_control_flags(ctrlvalue, &id, &value)?;

                let (r, g, b) = value.as_rgb().ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Expected RGB".to_string(),
                })?;

                if !ctrlvalue.description().verify_setter(&value) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Failed to verify value".to_string(),
                    });
                }

                let gains = AVCaptureWhiteBalanceGains {
                    redGain: *r as c_float,
                    greenGain: *g as c_float,
                    blueGain: *b as c_float,
                };
                unsafe {
                    self.inner
                        .setWhiteBalanceModeLockedWithDeviceWhiteBalanceGains_completionHandler(
                            gains, None,
                        );
                }

                Ok(())
            }
            KnownCameraControl::Zoom => {
                let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                check_control_flags(ctrlvalue, &id, &value)?;

                let setter = *value.as_float().ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Expected float".to_string(),
                })? as CGFloat;

                if !ctrlvalue.description().verify_setter(&value) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Failed to verify value".to_string(),
                    });
                }

                unsafe {
                    self.inner.rampToVideoZoomFactor_withRate(setter, 1.0_f32);
                }

                Ok(())
            }
            KnownCameraControl::Exposure => {
                let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                check_control_flags(ctrlvalue, &id, &value)?;

                let setter = *value.as_enum().ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Expected Enum".to_string(),
                })?;

                if !ctrlvalue.description().verify_setter(&value) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Failed to verify value".to_string(),
                    });
                }

                unsafe {
                    self.inner
                        .setExposureMode(AVCaptureExposureMode(setter as isize));
                }

                Ok(())
            }
            KnownCameraControl::Iris => Err(NokhwaError::SetPropertyError {
                property: id.to_string(),
                value: value.to_string(),
                error: "Read Only".to_string(),
            }),
            KnownCameraControl::Focus => {
                let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                check_control_flags(ctrlvalue, &id, &value)?;

                let setter = *value.as_enum().ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Expected Enum".to_string(),
                })?;

                if !ctrlvalue.description().verify_setter(&value) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Failed to verify value".to_string(),
                    });
                }

                unsafe {
                    self.inner.setFocusMode(AVCaptureFocusMode(setter as isize));
                }

                Ok(())
            }
            KnownCameraControl::Other(i) => match i {
                0 => {
                    // Focus point of interest
                    let ctrlvalue = get_and_check_control(&controls, &id, &value)?;

                    let setter = value
                        .as_point()
                        .ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Point".to_string(),
                        })
                        .map(|(x, y)| CGPoint {
                            x: *x as CGFloat,
                            y: *y as CGFloat,
                        })?;

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    unsafe { self.inner.setFocusPointOfInterest(setter) };

                    Ok(())
                }
                1 => {
                    // Focus manual lens position
                    let ctrlvalue = get_and_check_control(&controls, &id, &value)?;

                    let setter = *value.as_float().ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Expected float".to_string(),
                    })? as c_float;

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    unsafe {
                        self.inner
                            .setFocusModeLockedWithLensPosition_completionHandler(setter, None);
                    }

                    Ok(())
                }
                2 => {
                    // Exposure point of interest
                    let ctrlvalue = get_and_check_control(&controls, &id, &value)?;

                    let setter = value
                        .as_point()
                        .ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Point".to_string(),
                        })
                        .map(|(x, y)| CGPoint {
                            x: *x as CGFloat,
                            y: *y as CGFloat,
                        })?;

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    unsafe { self.inner.setExposurePointOfInterest(setter) };

                    Ok(())
                }
                3 => {
                    // Face-driven auto exposure
                    let ctrlvalue = get_and_check_control(&controls, &id, &value)?;

                    let setter: bool =
                        *value.as_boolean().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Boolean".to_string(),
                        })?;

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    unsafe {
                        self.inner
                            .setAutomaticallyAdjustsFaceDrivenAutoExposureEnabled(setter);
                    }

                    Ok(())
                }
                4 => {
                    // Exposure target bias
                    let ctrlvalue = get_and_check_control(&controls, &id, &value)?;

                    let setter = *value.as_float().ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Expected Float".to_string(),
                    })? as c_float;

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    unsafe {
                        self.inner
                            .setExposureTargetBias_completionHandler(setter, None);
                    }

                    Ok(())
                }
                5 => {
                    // Torch mode
                    let ctrlvalue = get_and_check_control(&controls, &id, &value)?;

                    let setter = *value.as_enum().ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Expected Enum".to_string(),
                    })?;

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    unsafe {
                        self.inner.setTorchMode(AVCaptureTorchMode(setter as isize));
                    }

                    Ok(())
                }
                6 => {
                    // Geometric distortion correction
                    let ctrlvalue = get_and_check_control(&controls, &id, &value)?;

                    let setter: bool =
                        *value.as_boolean().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Boolean".to_string(),
                        })?;

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    unsafe {
                        self.inner.setGeometricDistortionCorrectionEnabled(setter);
                    }

                    Ok(())
                }
                _ => Err(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Unknown Control".to_string(),
                }),
            },
            _ => Err(NokhwaError::SetPropertyError {
                property: id.to_string(),
                value: value.to_string(),
                error: "Unknown Control".to_string(),
            }),
        }
    }

    pub fn active_format(&self) -> Result<CameraFormat, NokhwaError> {
        let af = unsafe { self.inner.activeFormat() };
        let avf_format = AVCaptureDeviceFormatWrapper::try_from_format(&af)?;
        let resolution = avf_format.resolution;
        let fourcc = avf_format.fourcc;
        let mut a = avf_format
            .fps_list
            .into_iter()
            .map(move |fps_f64| {
                let fps = fps_f64 as u32;
                let resolution = Resolution::new(resolution.width as u32, resolution.height as u32);
                CameraFormat::new(resolution, fourcc, fps)
            })
            .collect::<Vec<_>>();
        a.sort_by_key(|a| a.frame_rate());

        if !a.is_empty() {
            Ok(a[a.len() - 1])
        } else {
            Err(NokhwaError::GetPropertyError {
                property: "activeFormat".to_string(),
                error: "None??".to_string(),
            })
        }
    }
}

fn check_control_flags(
    ctrl: &CameraControl,
    id: &KnownCameraControl,
    value: &ControlValueSetter,
) -> Result<(), NokhwaError> {
    if ctrl.flag().contains(&KnownCameraControlFlag::ReadOnly) {
        return Err(NokhwaError::SetPropertyError {
            property: id.to_string(),
            value: value.to_string(),
            error: "Read Only".to_string(),
        });
    }
    if ctrl.flag().contains(&KnownCameraControlFlag::Disabled) {
        return Err(NokhwaError::SetPropertyError {
            property: id.to_string(),
            value: value.to_string(),
            error: "Disabled".to_string(),
        });
    }
    Ok(())
}

fn get_and_check_control<'a>(
    controls: &'a BTreeMap<KnownCameraControl, &'a CameraControl>,
    id: &KnownCameraControl,
    value: &ControlValueSetter,
) -> Result<&'a CameraControl, NokhwaError> {
    let ctrlvalue = controls.get(id).ok_or(NokhwaError::SetPropertyError {
        property: id.to_string(),
        value: value.to_string(),
        error: "Control does not exist".to_string(),
    })?;
    check_control_flags(ctrlvalue, id, value)?;
    Ok(ctrlvalue)
}
