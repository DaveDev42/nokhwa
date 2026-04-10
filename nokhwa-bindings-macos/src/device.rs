use crate::ffi::{
    CMFormatDescriptionGetMediaSubType, CMFormatDescriptionRef, CMVideoDimensions,
    CMVideoFormatDescriptionGetDimensions,
};
use crate::session::AVCaptureDeviceDiscoverySession;
use crate::types::{AVCaptureDevicePosition, AVCaptureDeviceType, AVMediaType};
use crate::util::{
    create_boilerplate_impl, ns_arr_to_vec, nsstr_to_str, raw_fcc_to_frameformat, str_to_nsstr,
    try_ns_arr_to_vec,
};
use nokhwa_core::{
    error::NokhwaError,
    types::{ApiBackend, CameraFormat, CameraIndex, CameraInfo, FrameFormat, Resolution},
};
use objc2::runtime::AnyObject;
use std::{cmp::Ordering, convert::TryFrom, ffi::c_void};

// fuck it, use deprecated APIs
pub fn query_avfoundation() -> Result<Vec<CameraInfo>, NokhwaError> {
    Ok(AVCaptureDeviceDiscoverySession::new(vec![
        AVCaptureDeviceType::UltraWide,
        AVCaptureDeviceType::WideAngle,
        AVCaptureDeviceType::Telephoto,
        AVCaptureDeviceType::TrueDepth,
        AVCaptureDeviceType::External,
    ])?
    .devices())
}

pub fn get_raw_device_info(index: CameraIndex, device: *mut AnyObject) -> CameraInfo {
    let name = nsstr_to_str(unsafe { objc2::msg_send![device, localizedName] });
    let manufacturer = nsstr_to_str(unsafe { objc2::msg_send![device, manufacturer] });
    let position: AVCaptureDevicePosition = unsafe { objc2::msg_send![device, position] };
    let lens_aperture: f64 = unsafe { objc2::msg_send![device, lensAperture] };
    let device_type = nsstr_to_str(unsafe { objc2::msg_send![device, deviceType] });
    let model_id = nsstr_to_str(unsafe { objc2::msg_send![device, modelID] });
    let description = format!(
        "{}: {} - {}, {:?} f{}",
        manufacturer, model_id, device_type, position, lens_aperture
    );
    let misc = nsstr_to_str(unsafe { objc2::msg_send![device, uniqueID] });

    CameraInfo::new(name.as_ref(), &description, misc.as_ref(), index)
}

create_boilerplate_impl! {
    [pub AVFrameRateRange]
}

impl AVFrameRateRange {
    pub fn max(&self) -> f64 {
        unsafe { objc2::msg_send![self.inner, maxFrameRate] }
    }

    pub fn min(&self) -> f64 {
        unsafe { objc2::msg_send![self.inner, minFrameRate] }
    }
}

#[derive(Debug)]
pub struct AVCaptureDeviceFormat {
    pub(crate) internal: *mut AnyObject,
    pub resolution: CMVideoDimensions,
    pub fps_list: Vec<f64>,
    pub fourcc: FrameFormat,
}

impl TryFrom<*mut AnyObject> for AVCaptureDeviceFormat {
    type Error = NokhwaError;

    fn try_from(value: *mut AnyObject) -> Result<Self, Self::Error> {
        let media_type_raw: *mut AnyObject = unsafe { objc2::msg_send![value, mediaType] };
        let media_type = AVMediaType::try_from(media_type_raw)?;
        if media_type != AVMediaType::Video {
            return Err(NokhwaError::StructureError {
                structure: "AVMediaType".to_string(),
                error: "Not Video".to_string(),
            });
        }
        let mut fps_list = ns_arr_to_vec::<AVFrameRateRange>(unsafe {
            objc2::msg_send![value, videoSupportedFrameRateRanges]
        })
        .into_iter()
        .flat_map(|v| {
            if v.min() != 0_f64 && v.min() != 1_f64 {
                vec![v.min(), v.max()]
            } else {
                vec![v.max()] // this gets deduped!
            }
        })
        .collect::<Vec<f64>>();
        fps_list.sort_by(|n, m| n.partial_cmp(m).unwrap_or(Ordering::Equal));
        fps_list.dedup();
        let description_obj: *mut AnyObject = unsafe { objc2::msg_send![value, formatDescription] };
        let resolution =
            unsafe { CMVideoFormatDescriptionGetDimensions(description_obj as *mut c_void) };
        let fcc_raw = unsafe { CMFormatDescriptionGetMediaSubType(description_obj as *mut c_void) };
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

        Ok(AVCaptureDeviceFormat {
            internal: value,
            resolution,
            fps_list,
            fourcc,
        })
    }
}

pub struct AVCaptureDevice {
    pub(crate) inner: *mut AnyObject,
    device: CameraInfo,
    locked: bool,
}

impl AVCaptureDevice {
    pub fn inner(&self) -> *mut AnyObject {
        self.inner
    }
}

impl AVCaptureDevice {
    pub fn new(index: &CameraIndex) -> Result<Self, NokhwaError> {
        match &index {
            CameraIndex::Index(idx) => {
                let devices = query_avfoundation()?;

                match devices.get(*idx as usize) {
                    Some(device) => Ok(AVCaptureDevice::from_id(
                        &device.misc(),
                        Some(index.clone()),
                    )?),
                    None => Err(NokhwaError::OpenDeviceError(
                        idx.to_string(),
                        "Not Found".to_string(),
                    )),
                }
            }
            CameraIndex::String(id) => Ok(AVCaptureDevice::from_id(id, None)?),
        }
    }

    pub fn from_id(id: &str, index_hint: Option<CameraIndex>) -> Result<Self, NokhwaError> {
        let nsstr_id = str_to_nsstr(id);
        let avfoundation_capture_cls = objc2::class!(AVCaptureDevice);
        let capture: *mut AnyObject =
            unsafe { objc2::msg_send![avfoundation_capture_cls, deviceWithUniqueID: nsstr_id] };
        if capture.is_null() {
            return Err(NokhwaError::OpenDeviceError(
                id.to_string(),
                "Device is null".to_string(),
            ));
        }
        let camera_info = get_raw_device_info(
            index_hint.unwrap_or_else(|| CameraIndex::String(id.to_string())),
            capture,
        );

        Ok(AVCaptureDevice {
            inner: capture,
            device: camera_info,
            locked: false,
        })
    }

    pub fn info(&self) -> &CameraInfo {
        &self.device
    }

    pub fn supported_formats_raw(&self) -> Result<Vec<AVCaptureDeviceFormat>, NokhwaError> {
        try_ns_arr_to_vec::<AVCaptureDeviceFormat, NokhwaError>(unsafe {
            objc2::msg_send![self.inner, formats]
        })
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
                        Resolution::new(resolution.width as u32, resolution.height as u32); // FIXME: what the fuck?
                    CameraFormat::new(resolution, av_fmt.fourcc, fps)
                })
            })
            .filter(|x| x.frame_rate() != 0)
            .collect())
    }

    pub fn already_in_use(&self) -> bool {
        unsafe {
            let result: bool = objc2::msg_send![self.inner(), isInUseByAnotherApplication];
            result
        }
    }

    pub fn is_suspended(&self) -> bool {
        unsafe {
            let result: bool = objc2::msg_send![self.inner, isSuspended];
            result
        }
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
        let mut err_ptr: *mut AnyObject = std::ptr::null_mut();
        let accepted: bool =
            unsafe { objc2::msg_send![self.inner, lockForConfiguration: &mut err_ptr as *mut _] };
        if !accepted {
            return Err(NokhwaError::SetPropertyError {
                property: "lockForConfiguration".to_string(),
                value: "Locked".to_string(),
                error: if !err_ptr.is_null() {
                    let desc: *mut AnyObject =
                        unsafe { objc2::msg_send![err_ptr, localizedDescription] };
                    nsstr_to_str(desc).into_owned()
                } else {
                    "Lock rejected".to_string()
                },
            });
        }
        self.locked = true;
        Ok(())
    }

    pub fn unlock(&mut self) {
        if self.locked {
            self.locked = false;
            unsafe { objc2::msg_send![self.inner, unlockForConfiguration] }
        }
    }

    // thank you ffmpeg
    pub fn set_all(&mut self, descriptor: CameraFormat) -> Result<(), NokhwaError> {
        self.lock()?;
        let format_list = try_ns_arr_to_vec::<AVCaptureDeviceFormat, NokhwaError>(unsafe {
            objc2::msg_send![self.inner, formats]
        })?;
        let format_description_sel = objc2::sel!(formatDescription);

        let mut selected_format: *mut AnyObject = std::ptr::null_mut();
        let mut selected_range: *mut AnyObject = std::ptr::null_mut();

        for format in format_list {
            let format_desc_ref: CMFormatDescriptionRef = unsafe {
                objc2::msg_send![format.internal, performSelector: format_description_sel]
            };
            let dimensions = unsafe { CMVideoFormatDescriptionGetDimensions(format_desc_ref) };

            if dimensions.height == descriptor.resolution().height() as i32
                && dimensions.width == descriptor.resolution().width() as i32
            {
                selected_format = format.internal;

                for range in ns_arr_to_vec::<AVFrameRateRange>(unsafe {
                    objc2::msg_send![format.internal, videoSupportedFrameRateRanges]
                }) {
                    let max_fps: f64 = unsafe { objc2::msg_send![range.inner, maxFrameRate] };
                    // Older Apple cameras (i.e. iMac 2013) return 29.97000002997 as FPS.
                    if (f64::from(descriptor.frame_rate()) - max_fps).abs() < 0.999 {
                        selected_range = range.inner;
                        break;
                    }
                }
            }
        }
        if selected_range.is_null() || selected_format.is_null() {
            return Err(NokhwaError::SetPropertyError {
                property: "CameraFormat".to_string(),
                value: descriptor.to_string(),
                error: "Not Found/Rejected/Unsupported".to_string(),
            });
        }

        let activefmtkey = str_to_nsstr("activeFormat");
        let min_frame_duration = str_to_nsstr("minFrameDuration");
        let active_video_min_frame_duration = str_to_nsstr("activeVideoMinFrameDuration");
        let active_video_max_frame_duration = str_to_nsstr("activeVideoMaxFrameDuration");
        let _: () =
            unsafe { objc2::msg_send![self.inner, setValue:selected_format, forKey:activefmtkey] };
        let min_frame_duration: *mut AnyObject =
            unsafe { objc2::msg_send![selected_range, valueForKey: min_frame_duration] };
        let _: () = unsafe {
            objc2::msg_send![self.inner, setValue:min_frame_duration, forKey:active_video_min_frame_duration]
        };
        let _: () = unsafe {
            objc2::msg_send![self.inner, setValue:min_frame_duration, forKey:active_video_max_frame_duration]
        };
        self.unlock();
        Ok(())
    }

    pub fn active_format(&self) -> Result<CameraFormat, NokhwaError> {
        let af: *mut AnyObject = unsafe { objc2::msg_send![self.inner, activeFormat] };
        let avf_format = AVCaptureDeviceFormat::try_from(af)?;
        let resolution = avf_format.resolution;
        let fourcc = avf_format.fourcc;
        let mut a = avf_format
            .fps_list
            .into_iter()
            .map(move |fps_f64| {
                let fps = fps_f64 as u32;

                let resolution = Resolution::new(resolution.width as u32, resolution.height as u32); // FIXME: what the fuck?
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
