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
use crate::callback::{AVCaptureVideoCallback, FrameData};
use crate::device::AVCaptureDeviceWrapper;
use crate::session::{
    create_device_input, create_video_data_output, output_add_delegate, output_set_frame_format,
    session_add_input, session_add_output, session_begin_configuration,
    session_commit_configuration, session_is_interrupted, session_is_running, session_new,
    session_remove_input, session_remove_output, session_start, session_stop,
};
use nokhwa_core::{
    buffer::{Buffer, TimestampKind},
    error::NokhwaError,
    pixel_format::RgbFormat,
    traits::CaptureBackendTrait,
    types::{
        ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueSetter,
        FrameFormat, KnownCameraControl, RequestedFormat, RequestedFormatType, Resolution,
    },
};
use objc2::rc::Retained;
use objc2_av_foundation::{AVCaptureDeviceInput, AVCaptureSession, AVCaptureVideoDataOutput};
use std::sync::mpsc::{Receiver, Sender};
use std::{borrow::Cow, collections::HashMap, ffi::CString, sync::Arc};

/// The backend struct that interfaces with `AVFoundation`.
/// To see what this does, please see [`CaptureBackendTrait`].
/// # Quirks
/// - While working with `iOS` is allowed, it is not officially supported and may not work.
/// - You **must** call [`nokhwa_initialize`](crate::nokhwa_initialize) **before** doing anything with `AVFoundation`.
/// - This only works on 64 bit platforms.
/// - FPS adjustment does not work.
/// - If permission has not been granted and you call `init()` it will error.
pub struct AVFoundationCaptureDevice {
    device: AVCaptureDeviceWrapper,
    dev_input: Option<Retained<AVCaptureDeviceInput>>,
    session: Option<Retained<AVCaptureSession>>,
    data_out: Option<Retained<AVCaptureVideoDataOutput>>,
    data_collect: Option<AVCaptureVideoCallback>,
    info: CameraInfo,
    buffer_name: CString,
    format: CameraFormat,
    frame_buffer_receiver: Receiver<FrameData>,
    fbufsnd: Arc<Sender<FrameData>>,
}

impl AVFoundationCaptureDevice {
    /// Creates a new capture device using the `AVFoundation` backend. Indexes are gives to devices by the OS, and usually numbered by order of discovery.
    ///
    /// If `camera_format` is `None`, it will be spawned with with 640x480@15 FPS, MJPEG [`CameraFormat`] default.
    /// # Errors
    /// This function will error if the camera is currently busy or if `AVFoundation` can't read device information, or permission was not given by the user.
    pub fn new(index: &CameraIndex, req_fmt: RequestedFormat) -> Result<Self, NokhwaError> {
        let mut device = AVCaptureDeviceWrapper::new(index)?;

        let formats = device.supported_formats()?;
        let camera_fmt = req_fmt.fulfill(&formats).ok_or_else(|| {
            NokhwaError::OpenDeviceError("Cannot fulfill request".to_string(), req_fmt.to_string())
        })?;
        device.set_all(camera_fmt)?;

        let device_descriptor = device.info().clone();
        let buffername =
            CString::new(format!("{device_descriptor}_INDEX{index}_")).map_err(|why| {
                NokhwaError::StructureError {
                    structure: "CString Buffername".to_string(),
                    error: why.to_string(),
                }
            })?;

        let (send, recv) = std::sync::mpsc::channel();
        Ok(AVFoundationCaptureDevice {
            device,
            dev_input: None,
            session: None,
            data_out: None,
            data_collect: None,
            info: device_descriptor,
            buffer_name: buffername,
            format: camera_fmt,
            frame_buffer_receiver: recv,
            fbufsnd: Arc::new(send),
        })
    }

    /// Creates a new capture device using the `AVFoundation` backend with desired settings.
    ///
    /// # Errors
    /// This function will error if the camera is currently busy or if `AVFoundation` can't read device information, or permission was not given by the user.
    #[deprecated(since = "0.10.0", note = "please use `new` instead.")]
    #[allow(clippy::cast_possible_truncation)]
    pub fn new_with(
        index: usize,
        width: u32,
        height: u32,
        fps: u32,
        fourcc: FrameFormat,
    ) -> Result<Self, NokhwaError> {
        let camera_format = CameraFormat::new_from(width, height, fourcc, fps);
        AVFoundationCaptureDevice::new(
            &CameraIndex::Index(index as u32),
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(camera_format)),
        )
    }
}

impl CaptureBackendTrait for AVFoundationCaptureDevice {
    fn backend(&self) -> ApiBackend {
        ApiBackend::AVFoundation
    }

    fn camera_info(&self) -> &CameraInfo {
        &self.info
    }

    fn refresh_camera_format(&mut self) -> Result<(), NokhwaError> {
        self.format = self.device.active_format()?;
        Ok(())
    }

    fn camera_format(&self) -> CameraFormat {
        self.format
    }

    fn set_camera_format(&mut self, new_fmt: CameraFormat) -> Result<(), NokhwaError> {
        self.device.set_all(new_fmt)?;
        self.format = new_fmt;
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    fn compatible_list_by_resolution(
        &mut self,
        fourcc: FrameFormat,
    ) -> Result<HashMap<Resolution, Vec<u32>>, NokhwaError> {
        let supported_cfmt = self
            .device
            .supported_formats()?
            .into_iter()
            .filter(|x: &CameraFormat| x.format() == fourcc);
        let mut res_list = HashMap::new();
        for format in supported_cfmt {
            match res_list.get_mut(&format.resolution()) {
                Some(fpses) => Vec::push(fpses, format.frame_rate()),
                None => {
                    res_list.insert(format.resolution(), vec![format.frame_rate()]);
                }
            }
        }
        Ok(res_list)
    }

    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        let mut formats = self
            .device
            .supported_formats()?
            .into_iter()
            .map(|fmt: CameraFormat| fmt.format())
            .collect::<Vec<FrameFormat>>();
        formats.sort();
        formats.dedup();
        Ok(formats)
    }

    fn resolution(&self) -> Resolution {
        self.camera_format().resolution()
    }

    fn set_resolution(&mut self, new_res: Resolution) -> Result<(), NokhwaError> {
        let mut format = self.camera_format();
        format.set_resolution(new_res);
        self.set_camera_format(format)
    }

    fn frame_rate(&self) -> u32 {
        self.camera_format().frame_rate()
    }

    fn set_frame_rate(&mut self, new_fps: u32) -> Result<(), NokhwaError> {
        let mut format = self.camera_format();
        format.set_frame_rate(new_fps);
        self.set_camera_format(format)
    }

    fn frame_format(&self) -> FrameFormat {
        self.camera_format().format()
    }

    fn set_frame_format(&mut self, fourcc: FrameFormat) -> Result<(), NokhwaError> {
        let mut format = self.camera_format();
        format.set_format(fourcc);
        self.set_camera_format(format)
    }

    fn camera_control(&self, control: KnownCameraControl) -> Result<CameraControl, NokhwaError> {
        for ctrl in self.device.get_controls()? {
            if ctrl.control() == control {
                return Ok(ctrl);
            }
        }

        Err(NokhwaError::GetPropertyError {
            property: control.to_string(),
            error: "Not Found".to_string(),
        })
    }

    fn camera_controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
        self.device.get_controls()
    }

    fn set_camera_control(
        &mut self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        self.device.lock()?;
        let res = self.device.set_control(id, value);
        self.device.unlock();
        res
    }

    fn open_stream(&mut self) -> Result<(), NokhwaError> {
        self.refresh_camera_format()?;

        let input = create_device_input(self.device.inner())?;
        let session = session_new();
        session_begin_configuration(&session);
        session_add_input(&session, &input)?;

        self.device.set_all(self.format)?;

        let bufname = &self.buffer_name;
        let videocallback = AVCaptureVideoCallback::new(bufname, &self.fbufsnd)?;
        let output = create_video_data_output();
        output_add_delegate(&output, &videocallback)?;
        output_set_frame_format(&output, self.camera_format().format())?;
        session_add_output(&session, &output)?;
        session_commit_configuration(&session);
        session_start(&session)?;

        self.dev_input = Some(input);
        self.session = Some(session);
        self.data_collect = Some(videocallback);
        self.data_out = Some(output);
        Ok(())
    }

    fn is_stream_open(&self) -> bool {
        match (
            &self.session,
            &self.data_out,
            &self.data_collect,
            &self.dev_input,
        ) {
            (Some(session), Some(_), Some(_), Some(_)) => {
                !session_is_interrupted(session) && session_is_running(session)
            }
            _ => false,
        }
    }

    fn frame(&mut self) -> Result<Buffer, NokhwaError> {
        self.refresh_camera_format()?;
        let cfmt = self.camera_format();
        let (bytes, _fmt, capture_ts) = self
            .frame_buffer_receiver
            .recv()
            .map_err(|why| NokhwaError::ReadFrameError(why.to_string()))?;
        let buffer = Buffer::with_timestamp(
            cfmt.resolution(),
            &bytes,
            cfmt.format(),
            capture_ts.map(|ts| (ts, TimestampKind::Presentation)),
        );
        self.frame_buffer_receiver.try_iter().for_each(drop);
        Ok(buffer)
    }

    fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
        match self.frame_buffer_receiver.recv() {
            Ok(recv) => Ok(Cow::from(recv.0)),
            Err(why) => Err(NokhwaError::ReadFrameError(why.to_string())),
        }
    }

    fn stop_stream(&mut self) -> Result<(), NokhwaError> {
        if !self.is_stream_open() {
            return Ok(());
        }

        let Some(session) = &self.session else {
            return Err(NokhwaError::GetPropertyError {
                property: "AVCaptureSession".to_string(),
                error: "Doesnt Exist".to_string(),
            });
        };

        let Some(output) = &self.data_out else {
            return Err(NokhwaError::GetPropertyError {
                property: "AVCaptureVideoDataOutput".to_string(),
                error: "Doesnt Exist".to_string(),
            });
        };

        let Some(input) = &self.dev_input else {
            return Err(NokhwaError::GetPropertyError {
                property: "AVCaptureDeviceInput".to_string(),
                error: "Doesnt Exist".to_string(),
            });
        };

        session_remove_output(session, output);
        session_remove_input(session, input);
        session_stop(session);

        self.frame_buffer_receiver.try_iter().for_each(drop);
        self.dev_input = None;
        self.session = None;
        self.data_collect = None;
        self.data_out = None;

        Ok(())
    }
}

impl Drop for AVFoundationCaptureDevice {
    fn drop(&mut self) {
        let _ = self.stop_stream();
        self.device.unlock();
    }
}

// SAFETY: AVFoundationCaptureDevice is safe to Send (move) between threads because:
// - All access goes through &mut self, so after a move the new thread has exclusive
//   ownership — no aliasing across threads occurs. We do NOT implement Sync.
// - The Retained<AVCaptureDevice/Session/Input/Output> fields hold reference-counted
//   ObjC pointers. objc2 conservatively marks them !Send because Apple docs recommend
//   dispatching calls on a specific queue, but exclusive &mut self access satisfies
//   that constraint: only one thread touches them at a time.
// - The GCD DispatchQueue is designed for cross-thread use.
// - The *mut AnyObject delegate is only accessed during setup and by the GCD queue
//   callback; it is not touched directly after construction.
// - The mpsc Sender/Receiver are themselves Send.
unsafe impl Send for AVFoundationCaptureDevice {}
