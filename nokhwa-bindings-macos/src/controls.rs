use crate::device::AVCaptureDevice;
use crate::ffi::CMTime;
use crate::ffi::{
    AVCaptureExposureDurationCurrent, AVCaptureExposureTargetBiasCurrent, AVCaptureISOCurrent,
    AVCaptureWhiteBalanceGains, CGFloat, CGPoint, EncodableCMTime,
};
use nokhwa_core::{
    error::NokhwaError,
    types::{
        CameraControl, ControlValueDescription, ControlValueSetter, KnownCameraControl,
        KnownCameraControlFlag,
    },
};
use objc2::runtime::AnyObject;
use std::{collections::BTreeMap, ffi::c_float};

impl AVCaptureDevice {
    // 0 => Focus POI
    // 1 => Focus Manual Setting
    // 2 => Exposure POI
    // 3 => Exposure Face Driven
    // 4 => Exposure Target Bias
    // 5 => Exposure ISO
    // 6 => Exposure Duration
    pub fn get_controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
        let active_format: *mut AnyObject = unsafe { objc2::msg_send![self.inner, activeFormat] };

        let mut controls = vec![];
        // get focus modes

        let focus_current: isize = unsafe { objc2::msg_send![self.inner, focusMode] };
        let focus_locked: bool =
            unsafe { objc2::msg_send![self.inner, isFocusModeSupported: 0_isize] };
        let focus_auto: bool =
            unsafe { objc2::msg_send![self.inner, isFocusModeSupported: 1_isize] };
        let focus_continuous: bool =
            unsafe { objc2::msg_send![self.inner, isFocusModeSupported: 2_isize] };

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
                    value: focus_current as i64,
                    possible: supported_focus_values,
                    default: focus_current as i64,
                },
                vec![],
                true,
            ));
        }

        let focus_poi_supported: bool =
            unsafe { objc2::msg_send![self.inner, isFocusPointOfInterestSupported] };
        let focus_poi: CGPoint = unsafe { objc2::msg_send![self.inner, focusPointOfInterest] };

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

        let focus_manual: bool =
            unsafe { objc2::msg_send![self.inner, isLockingFocusWithCustomLensPositionSupported] };
        let focus_lenspos: f32 = unsafe { objc2::msg_send![self.inner, lensPosition] };

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

        // get exposures
        let exposure_current: isize = unsafe { objc2::msg_send![self.inner, exposureMode] };
        let exposure_locked: bool =
            unsafe { objc2::msg_send![self.inner, isExposureModeSupported: 0_isize] };
        let exposure_auto: bool =
            unsafe { objc2::msg_send![self.inner, isExposureModeSupported: 1_isize] };
        let exposure_continuous: bool =
            unsafe { objc2::msg_send![self.inner, isExposureModeSupported: 2_isize] };
        let exposure_custom: bool =
            unsafe { objc2::msg_send![self.inner, isExposureModeSupported: 3_isize] };

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
                    value: exposure_current as i64,
                    possible: supported_exposure_values,
                    default: exposure_current as i64,
                },
                vec![],
                true,
            ));
        }

        let exposure_poi_supported: bool =
            unsafe { objc2::msg_send![self.inner, isExposurePointOfInterestSupported] };
        let exposure_poi: CGPoint =
            unsafe { objc2::msg_send![self.inner, exposurePointOfInterest] };

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

        let exposure_face_driven_supported: bool =
            unsafe { objc2::msg_send![self.inner, isFaceDrivenAutoExposureEnabled] };
        let exposure_face_driven: bool = unsafe {
            objc2::msg_send![
                self.inner,
                automaticallyAdjustsFaceDrivenAutoExposureEnabled
            ]
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

        let exposure_bias: f32 = unsafe { objc2::msg_send![self.inner, exposureTargetBias] };
        let exposure_bias_min: f32 = unsafe { objc2::msg_send![self.inner, minExposureTargetBias] };
        let exposure_bias_max: f32 = unsafe { objc2::msg_send![self.inner, maxExposureTargetBias] };

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

        let exposure_duration: CMTime = unsafe {
            let t: EncodableCMTime = objc2::msg_send![self.inner, exposureDuration];
            t.0
        };
        let exposure_duration_min: CMTime = unsafe {
            let t: EncodableCMTime = objc2::msg_send![active_format, minExposureDuration];
            t.0
        };
        let exposure_duration_max: CMTime = unsafe {
            let t: EncodableCMTime = objc2::msg_send![active_format, maxExposureDuration];
            t.0
        };

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

        let exposure_iso: f32 = unsafe { objc2::msg_send![self.inner, ISO] };
        let exposure_iso_min: f32 = unsafe { objc2::msg_send![active_format, minISO] };
        let exposure_iso_max: f32 = unsafe { objc2::msg_send![active_format, maxISO] };

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

        let lens_aperture: f32 = unsafe { objc2::msg_send![self.inner, lensAperture] };

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

        // get white balance
        let white_balance_current: isize =
            unsafe { objc2::msg_send![self.inner, whiteBalanceMode] };
        let white_balance_manual: bool =
            unsafe { objc2::msg_send![self.inner, isWhiteBalanceModeSupported: 0_isize] };
        let white_balance_auto: bool =
            unsafe { objc2::msg_send![self.inner, isWhiteBalanceModeSupported: 1_isize] };
        let white_balance_continuous: bool =
            unsafe { objc2::msg_send![self.inner, isWhiteBalanceModeSupported: 2_isize] };

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
                    value: white_balance_current as i64,
                    possible,
                    default: 0,
                },
                vec![],
                true,
            ));
        }

        let white_balance_gains: AVCaptureWhiteBalanceGains =
            unsafe { objc2::msg_send![self.inner, deviceWhiteBalanceGains] };
        let white_balance_default: AVCaptureWhiteBalanceGains =
            unsafe { objc2::msg_send![self.inner, grayWorldDeviceWhiteBalanceGains] };
        let white_balance_max_scalar: f32 =
            unsafe { objc2::msg_send![self.inner, maxWhiteBalanceGain] };
        let white_balance_max = AVCaptureWhiteBalanceGains {
            redGain: white_balance_max_scalar,
            greenGain: white_balance_max_scalar,
            blueGain: white_balance_max_scalar,
        };
        let white_balance_gain_supported: bool = unsafe {
            objc2::msg_send![
                self.inner,
                isLockingWhiteBalanceWithCustomDeviceGainsSupported
            ]
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

        // get flash
        let has_torch: bool = unsafe { objc2::msg_send![self.inner, isTorchAvailable] };
        let torch_off: bool =
            unsafe { objc2::msg_send![self.inner, isTorchModeSupported: 0_isize] };
        let torch_on: bool = unsafe { objc2::msg_send![self.inner, isTorchModeSupported: 1_isize] };
        let torch_auto: bool =
            unsafe { objc2::msg_send![self.inner, isTorchModeSupported: 2_isize] };

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

            let torch_mode_current: isize = unsafe { objc2::msg_send![self.inner, torchMode] };

            controls.push(CameraControl::new(
                KnownCameraControl::Other(5),
                "TorchMode".to_string(),
                ControlValueDescription::Enum {
                    value: torch_mode_current as i64,
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

        // get low light boost
        let has_llb: bool = unsafe { objc2::msg_send![self.inner, isLowLightBoostSupported] };
        let llb_enabled: bool = unsafe { objc2::msg_send![self.inner, isLowLightBoostEnabled] };

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

        // get zoom factor
        let zoom_current: CGFloat = unsafe { objc2::msg_send![self.inner, videoZoomFactor] };
        let zoom_min: CGFloat =
            unsafe { objc2::msg_send![self.inner, minAvailableVideoZoomFactor] };
        let zoom_max: CGFloat =
            unsafe { objc2::msg_send![self.inner, maxAvailableVideoZoomFactor] };

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

        // zoom distortion correction
        let distortion_correction_supported: bool =
            unsafe { objc2::msg_send![self.inner, isGeometricDistortionCorrectionSupported] };
        let distortion_correction_current_value: bool =
            unsafe { objc2::msg_send![self.inner, isGeometricDistortionCorrectionEnabled] };

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

        let null_handler: *mut AnyObject = std::ptr::null_mut();

        match id {
            KnownCameraControl::Brightness => {
                let isoctrl = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                if isoctrl.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Exposure is in improper state to set ISO (Please set to `custom`!)"
                            .to_string(),
                    });
                }

                if isoctrl.flag().contains(&KnownCameraControlFlag::Disabled) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Disabled".to_string(),
                    });
                }

                let current_duration = unsafe { AVCaptureExposureDurationCurrent };
                let new_iso = *value.as_float().ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Expected float".to_string(),
                })? as f32;

                if !isoctrl.description().verify_setter(&value) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Failed to verify value".to_string(),
                    });
                }

                let _: () = unsafe {
                    objc2::msg_send![self.inner, setExposureModeCustomWithDuration:EncodableCMTime(current_duration), ISO:new_iso, completionHandler:null_handler]
                };

                Ok(())
            }
            KnownCameraControl::Gamma => {
                let duration_ctrl = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                if duration_ctrl
                    .flag()
                    .contains(&KnownCameraControlFlag::ReadOnly)
                {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Exposure is in improper state to set Duration (Please set to `custom`!)"
                            .to_string(),
                    });
                }

                if duration_ctrl
                    .flag()
                    .contains(&KnownCameraControlFlag::Disabled)
                {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Disabled".to_string(),
                    });
                }
                let current_duration: CMTime = unsafe {
                    let t: EncodableCMTime = objc2::msg_send![self.inner, exposureDuration];
                    t.0
                };

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

                let _: () = unsafe {
                    objc2::msg_send![self.inner, setExposureModeCustomWithDuration:EncodableCMTime(new_duration), ISO:current_iso, completionHandler:null_handler]
                };

                Ok(())
            }
            KnownCameraControl::WhiteBalance => {
                let wb_enum_value = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                if wb_enum_value
                    .flag()
                    .contains(&KnownCameraControlFlag::ReadOnly)
                {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Read Only".to_string(),
                    });
                }

                if wb_enum_value
                    .flag()
                    .contains(&KnownCameraControlFlag::Disabled)
                {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Disabled".to_string(),
                    });
                }
                let setter = *value.as_enum().ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Expected Enum".to_string(),
                })? as isize;

                if !wb_enum_value.description().verify_setter(&value) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Failed to verify value".to_string(),
                    });
                }

                let _: () = unsafe { objc2::msg_send![self.inner, whiteBalanceMode: setter] };

                Ok(())
            }
            KnownCameraControl::BacklightComp => {
                let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Read Only".to_string(),
                    });
                }

                if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Disabled".to_string(),
                    });
                }

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

                let _: () = unsafe {
                    objc2::msg_send![
                        self.inner,
                        setAutomaticallyEnablesLowLightBoostWhenAvailable: setter
                    ]
                };

                Ok(())
            }
            KnownCameraControl::Gain => {
                let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Read Only".to_string(),
                    });
                }

                if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Disabled".to_string(),
                    });
                }

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
                    redGain: *r as f32,
                    greenGain: *g as f32,
                    blueGain: *b as f32,
                };
                let null_handler: *mut AnyObject = std::ptr::null_mut();
                let _: () = unsafe {
                    objc2::msg_send![
                        self.inner,
                        setWhiteBalanceModeLockedWithDeviceWhiteBalanceGains: gains,
                        completionHandler: null_handler
                    ]
                };

                Ok(())
            }
            KnownCameraControl::Zoom => {
                let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Read Only".to_string(),
                    });
                }

                if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Disabled".to_string(),
                    });
                }

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

                let _: () = unsafe {
                    objc2::msg_send![self.inner, rampToVideoZoomFactor: setter, withRate: 1.0_f32]
                };

                Ok(())
            }
            KnownCameraControl::Exposure => {
                let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Control does not exist".to_string(),
                })?;

                if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Read Only".to_string(),
                    });
                }

                if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Disabled".to_string(),
                    });
                }

                let setter = *value.as_enum().ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Expected Enum".to_string(),
                })? as isize;

                if !ctrlvalue.description().verify_setter(&value) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Failed to verify value".to_string(),
                    });
                }

                let _: () = unsafe { objc2::msg_send![self.inner, exposureMode: setter] };

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

                if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Read Only".to_string(),
                    });
                }

                if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Disabled".to_string(),
                    });
                }

                let setter = *value.as_enum().ok_or(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Expected Enum".to_string(),
                })? as isize;

                if !ctrlvalue.description().verify_setter(&value) {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Failed to verify value".to_string(),
                    });
                }

                let _: () = unsafe { objc2::msg_send![self.inner, focusMode: setter] };

                Ok(())
            }
            KnownCameraControl::Other(i) => match i {
                0 => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

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

                    let _: () =
                        unsafe { objc2::msg_send![self.inner, focusPointOfInterest: setter] };

                    Ok(())
                }
                1 => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

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

                    let _: () = unsafe {
                        objc2::msg_send![self.inner, setFocusModeLockedWithLensPosition: setter, handler: null_handler]
                    };

                    Ok(())
                }
                2 => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

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

                    let _: () =
                        unsafe { objc2::msg_send![self.inner, exposurePointOfInterest: setter] };

                    Ok(())
                }
                3 => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

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

                    let _: () = unsafe {
                        objc2::msg_send![
                            self.inner,
                            automaticallyAdjustsFaceDrivenAutoExposureEnabled: setter
                        ]
                    };

                    Ok(())
                }
                4 => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

                    let setter = *value.as_float().ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Expected Float".to_string(),
                    })? as f32;

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe {
                        objc2::msg_send![self.inner, setExposureTargetBias: setter, handler: null_handler]
                    };

                    Ok(())
                }
                5 => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

                    let setter = *value.as_enum().ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Expected Enum".to_string(),
                    })? as isize;

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe { objc2::msg_send![self.inner, torchMode: setter] };

                    Ok(())
                }
                6 => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

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

                    let _: () = unsafe {
                        objc2::msg_send![self.inner, geometricDistortionCorrectionEnabled: setter]
                    };

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
}
