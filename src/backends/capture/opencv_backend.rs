/*
 * Copyright 2022 l1npengtul <l1npengtul@protonmail.com> / The Nokhwa Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use nokhwa_core::types::RequestedFormatType;
use nokhwa_core::{
    buffer::Buffer,
    error::NokhwaError,
    traits::{CameraDevice, FrameSource},
    types::{
        ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueDescription,
        ControlValueSetter, FrameFormat, KnownCameraControl, RequestedFormat, Resolution,
    },
};
use opencv::{
    core::{Mat, MatTraitConst, MatTraitConstManual, Vec3b},
    videoio::{
        VideoCapture, VideoCaptureProperties, VideoCaptureTrait, VideoCaptureTraitConst, CAP_ANY,
        CAP_AVFOUNDATION, CAP_MSMF, CAP_PROP_FPS, CAP_PROP_FRAME_HEIGHT, CAP_PROP_FRAME_WIDTH,
        CAP_V4L2,
    },
};
use std::borrow::Cow;

/// Attempts to convert a [`KnownCameraControl`] into an `OpenCV` video capture property.
/// If the associated control is not found, this will return `Err`.
///
/// # Errors
/// Returns [`NokhwaError::UnsupportedOperationError`] if the control has no
/// `OpenCV` equivalent.
pub fn known_camera_control_to_video_capture_property(
    ctrl: KnownCameraControl,
) -> Result<VideoCaptureProperties, NokhwaError> {
    match ctrl {
        KnownCameraControl::Brightness => Ok(VideoCaptureProperties::CAP_PROP_BRIGHTNESS),
        KnownCameraControl::Contrast => Ok(VideoCaptureProperties::CAP_PROP_CONTRAST),
        KnownCameraControl::Hue => Ok(VideoCaptureProperties::CAP_PROP_HUE),
        KnownCameraControl::Saturation => Ok(VideoCaptureProperties::CAP_PROP_SATURATION),
        KnownCameraControl::Sharpness => Ok(VideoCaptureProperties::CAP_PROP_SHARPNESS),
        KnownCameraControl::Gamma => Ok(VideoCaptureProperties::CAP_PROP_GAMMA),
        KnownCameraControl::BacklightComp => Ok(VideoCaptureProperties::CAP_PROP_BACKLIGHT),
        KnownCameraControl::Gain => Ok(VideoCaptureProperties::CAP_PROP_GAIN),
        KnownCameraControl::Pan => Ok(VideoCaptureProperties::CAP_PROP_PAN),
        KnownCameraControl::Tilt => Ok(VideoCaptureProperties::CAP_PROP_TILT),
        KnownCameraControl::Zoom => Ok(VideoCaptureProperties::CAP_PROP_ZOOM),
        KnownCameraControl::Exposure => Ok(VideoCaptureProperties::CAP_PROP_EXPOSURE),
        KnownCameraControl::Iris => Ok(VideoCaptureProperties::CAP_PROP_IRIS),
        KnownCameraControl::Focus => Ok(VideoCaptureProperties::CAP_PROP_FOCUS),
        _ => Err(NokhwaError::UnsupportedOperationError(ApiBackend::OpenCv)),
    }
}

/// The backend struct that interfaces with `OpenCV`. An `opencv` install
/// matching the compile-time version must be present on the user's machine
/// (usually 4.5.2 or greater). See
/// [`opencv-rust`](https://github.com/twistedfall/opencv-rust) and
/// [`OpenCV VideoCapture Docs`](https://docs.opencv.org/4.5.2/d8/dfe/classcv_1_1VideoCapture.html).
///
/// Implements [`CameraDevice`] and [`FrameSource`].
///
/// # Quirks
///  - **Setting [`Resolution`], FPS, [`FrameFormat`] is best-effort** — many
///    drivers silently ignore the request and keep 640×480 @ 30 FPS.
///  - This is a **cross-platform** backend; it will work on most platforms where
///    `OpenCV` is present.
///  - This backend can also accept an IP-camera URL as its [`CameraIndex::String`].
///  - The API preference order is the native OS API (linux → `V4L2`,
///    mac → `AVFoundation`, windows → `MSMF`), else `CAP_ANY`.
///  - `OpenCV` does not support device enumeration: [`FrameSource::compatible_formats`]
///    and [`FrameSource::compatible_fourcc`] return [`NokhwaError::UnsupportedOperationError`].
///  - [`CameraInfo`]'s human name will be "`OpenCV` Capture Device {location}".
///  - [`CameraInfo`]'s description will contain the Camera's Index or IP.
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "input-opencv")))]
pub struct OpenCvCaptureDevice {
    camera_format: CameraFormat,
    camera_location: CameraIndex,
    camera_info: CameraInfo,
    api_preference: i32,
    video_capture: VideoCapture,
}

/// Fallback format used when the caller does not pin an [`RequestedFormatType::Exact`]
/// format (e.g. `OpenRequest::any()`). OpenCV does not expose enumeration, so we
/// pick the VideoCapture-reported baseline (640×480 MJPEG @ 30fps).
const OPENCV_DEFAULT_FORMAT: (u32, u32, FrameFormat, u32) = (640, 480, FrameFormat::MJPEG, 30);

impl OpenCvCaptureDevice {
    /// Creates a new capture device using the `OpenCV` backend.
    ///
    /// Indexes are given to devices by the OS, and usually numbered by order of discovery.
    ///
    /// `IPCameras` follow the format:
    /// ```.ignore
    /// <protocol>://<IP>:<port>/
    /// ```
    /// but refer to the manufacturer for the actual IP format.
    ///
    /// # Errors
    /// Errors if the backend fails to open the camera (e.g. device does not
    /// exist at the specified index/ip), the camera does not support the
    /// specified [`CameraFormat`], or any other `OpenCV` error occurs.
    /// # Panics
    /// Panics if the requested `CameraIndex::Index` does not fit into `i32`.
    #[allow(clippy::cast_possible_wrap)]
    pub fn new(index: &CameraIndex, cam_fmt: RequestedFormat) -> Result<Self, NokhwaError> {
        let api_pref = if index.is_string() {
            CAP_ANY
        } else {
            get_api_pref_int()
        };

        let mut video_capture = match index {
            CameraIndex::Index(idx) => VideoCapture::new(*idx as i32, api_pref),
            CameraIndex::String(ip) => VideoCapture::from_file(ip.as_str(), api_pref),
        }
        .map_err(|why| NokhwaError::OpenDeviceError {
            device: index.to_string(),
            error: why.to_string(),
        })?;

        // OpenCV has no enumeration API, so requests that aren't `Exact` fall
        // back to the backend's baseline (640×480 MJPEG @ 30fps). This matches
        // the "best-effort" quirk documented above — many drivers will ignore
        // the requested format regardless of what we ask for.
        let camera_format = match cam_fmt.requested_format_type() {
            RequestedFormatType::Exact(exact) => exact,
            _ => {
                let (w, h, ff, fps) = OPENCV_DEFAULT_FORMAT;
                CameraFormat::new_from(w, h, ff, fps)
            }
        };

        set_properties(&mut video_capture, camera_format)?;

        let camera_info = CameraInfo::new(
            format!("OpenCV Capture Device {index}").as_str(),
            index.to_string().as_str(),
            "",
            index.clone(),
        );

        Ok(OpenCvCaptureDevice {
            camera_format,
            camera_location: index.clone(),
            camera_info,
            api_preference: api_pref,
            video_capture,
        })
    }

    /// Returns `true` if this capture device was opened as an IP camera.
    #[must_use]
    pub fn is_ip_camera(&self) -> bool {
        matches!(self.camera_location, CameraIndex::String(_))
    }

    /// Returns `true` if this capture device was opened by OS-assigned index.
    #[must_use]
    pub fn is_index_camera(&self) -> bool {
        matches!(self.camera_location, CameraIndex::Index(_))
    }

    /// Camera location this backend was opened against.
    #[must_use]
    pub fn camera_location(&self) -> &CameraIndex {
        &self.camera_location
    }

    /// `OpenCV` API preference integer. See
    /// [`OpenCV VideoCapture flag docs`](https://docs.opencv.org/4.5.2/d4/d15/group__videoio__flags__base.html).
    #[must_use]
    pub fn opencv_preference(&self) -> i32 {
        self.api_preference
    }

    /// Gets the RGB24 frame directly read from `OpenCV` without any additional processing.
    /// # Errors
    /// Errors if the frame fails to be read.
    #[allow(clippy::cast_sign_loss)]
    pub fn raw_frame_vec(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
        let frame_fmt = Some(self.camera_format.format());
        if !self.is_open() {
            return Err(NokhwaError::ReadFrameError {
                message: "Stream is not open!".to_string(),
                format: frame_fmt,
            });
        }

        let mut frame = Mat::default();
        match self.video_capture.read(&mut frame) {
            Ok(a) => {
                if !a {
                    return Err(NokhwaError::ReadFrameError {
                        message: "Failed to read frame from videocapture: OpenCV return false, camera disconnected?".to_string(),
                        format: frame_fmt,
                    });
                }
            }
            Err(why) => {
                return Err(NokhwaError::ReadFrameError {
                    message: format!("Failed to read frame from videocapture: {why}"),
                    format: frame_fmt,
                })
            }
        }

        if frame.empty() {
            return Err(NokhwaError::ReadFrameError {
                message: "Frame Empty!".to_string(),
                format: frame_fmt,
            });
        }

        match frame.size() {
            Ok(size) => {
                if size.width > 0 {
                    return if frame.is_continuous() {
                        let mut raw_vec: Vec<u8> = Vec::new();

                        let frame_data_vec = match Mat::data_typed::<Vec3b>(&frame) {
                            Ok(v) => v,
                            Err(why) => {
                                return Err(NokhwaError::ReadFrameError {
                                    message: format!(
                                        "Failed to convert frame into raw Vec3b: {why}"
                                    ),
                                    format: frame_fmt,
                                })
                            }
                        };

                        for pixel in frame_data_vec.iter() {
                            let pixel_slice: &[u8; 3] = pixel;
                            raw_vec.push(pixel_slice[2]);
                            raw_vec.push(pixel_slice[1]);
                            raw_vec.push(pixel_slice[0]);
                        }

                        Ok(Cow::from(raw_vec))
                    } else {
                        Err(NokhwaError::ReadFrameError {
                            message: "Failed to read frame from videocapture: not cont".to_string(),
                            format: frame_fmt,
                        })
                    };
                }
                Err(NokhwaError::ReadFrameError {
                    message: "Frame width is less than zero!".to_string(),
                    format: frame_fmt,
                })
            }
            Err(why) => Err(NokhwaError::ReadFrameError {
                message: format!(
                    "Failed to read frame from videocapture: failed to read size: {why}"
                ),
                format: frame_fmt,
            }),
        }
    }

    /// Resolution as currently reported by `OpenCV`.
    /// # Errors
    /// Errors if the resolution cannot be read (e.g. invalid or not supported).
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    pub fn raw_resolution(&self) -> Result<Resolution, NokhwaError> {
        let width = match self.video_capture.get(CAP_PROP_FRAME_WIDTH) {
            Ok(width) => width as u32,
            Err(why) => {
                return Err(NokhwaError::GetPropertyError {
                    property: "Width".to_string(),
                    error: why.to_string(),
                })
            }
        };

        let height = match self.video_capture.get(CAP_PROP_FRAME_HEIGHT) {
            Ok(height) => height as u32,
            Err(why) => {
                return Err(NokhwaError::GetPropertyError {
                    property: "Height".to_string(),
                    error: why.to_string(),
                })
            }
        };

        Ok(Resolution::new(width, height))
    }

    /// Framerate as currently reported by `OpenCV`.
    /// # Errors
    /// Errors if the framerate cannot be read (e.g. invalid or not supported).
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    pub fn raw_framerate(&self) -> Result<u32, NokhwaError> {
        match self.video_capture.get(CAP_PROP_FPS) {
            Ok(fps) => Ok(fps as u32),
            Err(why) => Err(NokhwaError::GetPropertyError {
                property: "Framerate".to_string(),
                error: why.to_string(),
            }),
        }
    }

    /// Look up a single control by its [`KnownCameraControl`] identifier.
    /// Kept as an inherent helper after the trait split; used internally by
    /// `set_control` to verify writes, mirroring the v4l backend.
    ///
    /// The returned [`CameraControl`] reports only the current value. OpenCV
    /// does not expose min/max/step/default/flags for `VideoCapture` properties,
    /// so those fields are placeholders (`default: 0.0`, `step: 0.0`, empty flags).
    ///
    /// # Errors
    /// Returns [`NokhwaError`] if the control has no `OpenCV` equivalent or
    /// the underlying get fails.
    pub fn camera_control(
        &self,
        control: KnownCameraControl,
    ) -> Result<CameraControl, NokhwaError> {
        let id = known_camera_control_to_video_capture_property(control)? as i32;
        let current = self
            .video_capture
            .get(id)
            .map_err(|why| NokhwaError::GetPropertyError {
                property: format!("{control:?}"),
                error: why.to_string(),
            })?;
        Ok(CameraControl::new(
            control,
            id.to_string(),
            ControlValueDescription::Float {
                value: current,
                default: 0.0,
                step: 0.0,
            },
            vec![],
            true,
        ))
    }
}

impl CameraDevice for OpenCvCaptureDevice {
    fn backend(&self) -> ApiBackend {
        ApiBackend::OpenCv
    }

    fn info(&self) -> &CameraInfo {
        &self.camera_info
    }

    fn controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
        Err(NokhwaError::UnsupportedOperationError(ApiBackend::OpenCv))
    }

    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::cast_lossless)]
    fn set_control(
        &mut self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        let control_val = match &value {
            ControlValueSetter::Integer(i) => *i as f64,
            ControlValueSetter::Float(f) => *f,
            ControlValueSetter::Boolean(b) => u8::from(*b) as f64,
            val => {
                return Err(NokhwaError::SetPropertyError {
                    property: format!("{id:?}"),
                    value: val.to_string(),
                    error: "unsupported value".to_string(),
                })
            }
        };

        if !self
            .video_capture
            .set(
                known_camera_control_to_video_capture_property(id)? as i32,
                control_val,
            )
            .map_err(|why| NokhwaError::SetPropertyError {
                property: format!("{id:?}"),
                value: control_val.to_string(),
                error: why.to_string(),
            })?
        {
            return Err(NokhwaError::SetPropertyError {
                property: format!("{id:?}"),
                value: control_val.to_string(),
                error: "false".to_string(),
            });
        }

        let set_value = self.camera_control(id)?.value();
        if set_value != value {
            return Err(NokhwaError::SetPropertyError {
                property: format!("{id:?}"),
                value: control_val.to_string(),
                error: "failed to set value: rejected".to_string(),
            });
        }

        Ok(())
    }
}

impl FrameSource for OpenCvCaptureDevice {
    fn negotiated_format(&self) -> CameraFormat {
        self.camera_format
    }

    fn set_format(&mut self, new_fmt: CameraFormat) -> Result<(), NokhwaError> {
        let current_format = self.camera_format;
        let was_open = match self.video_capture.is_opened() {
            Ok(opened) => opened,
            Err(why) => {
                return Err(NokhwaError::GetPropertyError {
                    property: "Is Stream Open".to_string(),
                    error: why.to_string(),
                })
            }
        };

        self.camera_format = new_fmt;

        if let Err(why) = set_properties(&mut self.video_capture, new_fmt) {
            self.camera_format = current_format;
            return Err(why);
        }
        if was_open {
            self.close()?;
            if let Err(why) = self.open() {
                // Revert so the backend's advertised format reflects the
                // last successfully-applied one. The device stays closed —
                // the caller must reopen explicitly after diagnosing `why`.
                self.camera_format = current_format;
                let _ = set_properties(&mut self.video_capture, current_format);
                return Err(NokhwaError::OpenDeviceError {
                    device: self.camera_location.to_string(),
                    error: why.to_string(),
                });
            }
        }
        Ok(())
    }

    fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
        Err(NokhwaError::UnsupportedOperationError(ApiBackend::OpenCv))
    }

    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        Err(NokhwaError::UnsupportedOperationError(ApiBackend::OpenCv))
    }

    #[allow(clippy::cast_possible_wrap)]
    fn open(&mut self) -> Result<(), NokhwaError> {
        match self.camera_location.clone() {
            CameraIndex::Index(idx) => {
                match self.video_capture.open(idx as i32, get_api_pref_int()) {
                    Ok(open) => {
                        if open {
                            return Ok(());
                        }
                        Err(NokhwaError::OpenStreamError {
                            message: "Stream is not opened after stream open attempt opencv"
                                .to_string(),
                            backend: Some(ApiBackend::OpenCv),
                        })
                    }
                    Err(why) => Err(NokhwaError::OpenDeviceError {
                        device: idx.to_string(),
                        error: format!("Failed to open device: {why}"),
                    }),
                }
            }
            CameraIndex::String(s) => Err(NokhwaError::OpenDeviceError {
                device: s.to_string(),
                error: "String index not supported (try NetworkCamera instead)".to_string(),
            }),
        }?;

        match self.video_capture.is_opened() {
            Ok(open) => {
                if open {
                    return Ok(());
                }
                Err(NokhwaError::OpenStreamError {
                    message: "Stream is not opened after stream open attempt opencv".to_string(),
                    backend: Some(ApiBackend::OpenCv),
                })
            }
            Err(why) => Err(NokhwaError::GetPropertyError {
                property: "Is Stream Open After Open Stream".to_string(),
                error: why.to_string(),
            }),
        }
    }

    fn is_open(&self) -> bool {
        self.video_capture.is_opened().unwrap_or(false)
    }

    fn frame(&mut self) -> Result<Buffer, NokhwaError> {
        let camera_resolution = self.camera_format.resolution();
        let expected_size = (camera_resolution.width() * camera_resolution.height() * 3) as usize;
        let raw = self.frame_raw()?;
        let data = match raw {
            Cow::Owned(v) => v,
            Cow::Borrowed(s) => s.to_vec(),
        };
        if data.len() != expected_size {
            return Err(NokhwaError::ReadFrameError {
                message: format!(
                    "OpenCV produced {} bytes, expected {expected_size} \
                     ({}×{} RGB24) — driver likely returned a different \
                     resolution than requested",
                    data.len(),
                    camera_resolution.width(),
                    camera_resolution.height(),
                ),
                format: Some(self.camera_format.format()),
            });
        }
        Ok(Buffer::from_vec(
            camera_resolution,
            data,
            self.camera_format.format(),
        ))
    }

    fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
        self.raw_frame_vec()
    }

    fn close(&mut self) -> Result<(), NokhwaError> {
        match self.video_capture.release() {
            Ok(()) => Ok(()),
            Err(why) => Err(NokhwaError::StreamShutdownError {
                message: why.to_string(),
                backend: Some(ApiBackend::OpenCv),
            }),
        }
    }
}

fn get_api_pref_int() -> i32 {
    match std::env::consts::OS {
        "linux" => CAP_V4L2,
        "windows" => CAP_MSMF,
        "macos" | "ios" => CAP_AVFOUNDATION,
        _ => CAP_ANY,
    }
}

// Historical note: setting OpenCV camera properties is unreliable across drivers.
// Many backends silently ignore the requested width/height/fps. We surface any
// explicit error but do not attempt to verify the applied format.
fn set_properties(vc: &mut VideoCapture, camera_format: CameraFormat) -> Result<(), NokhwaError> {
    if !vc
        .set(CAP_PROP_FRAME_WIDTH, f64::from(camera_format.width()))
        .map_err(|why| NokhwaError::SetPropertyError {
            property: "Resolution Width".to_string(),
            value: camera_format.to_string(),
            error: why.to_string(),
        })?
    {
        return Err(NokhwaError::SetPropertyError {
            property: "Resolution Width".to_string(),
            value: camera_format.to_string(),
            error: "false".to_string(),
        });
    }
    if !vc
        .set(CAP_PROP_FRAME_HEIGHT, f64::from(camera_format.height()))
        .map_err(|why| NokhwaError::SetPropertyError {
            property: "Resolution Height".to_string(),
            value: camera_format.to_string(),
            error: why.to_string(),
        })?
    {
        return Err(NokhwaError::SetPropertyError {
            property: "Resolution Height".to_string(),
            value: camera_format.to_string(),
            error: "false".to_string(),
        });
    }
    if !vc
        .set(CAP_PROP_FPS, f64::from(camera_format.frame_rate()))
        .map_err(|why| NokhwaError::SetPropertyError {
            property: "FPS".to_string(),
            value: camera_format.to_string(),
            error: why.to_string(),
        })?
    {
        return Err(NokhwaError::SetPropertyError {
            property: "FPS".to_string(),
            value: camera_format.to_string(),
            error: "false".to_string(),
        });
    }
    Ok(())
}
