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
    pixel_format::FormatDecoder,
    traits::CaptureBackendTrait,
    types::{
        ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueSetter,
        FrameFormat, KnownCameraControl, RequestedFormat, Resolution,
    },
};
use std::{borrow::Cow, collections::HashMap};
#[cfg(feature = "output-wgpu")]
use wgpu::{Device as WgpuDevice, Queue as WgpuQueue, Texture as WgpuTexture};

/// The main `Camera` struct. This is the struct that abstracts over all the backends, providing a simplified interface for use.
pub struct Camera {
    idx: CameraIndex,
    api: ApiBackend,
    device: Box<dyn CaptureBackendTrait>,
}

impl Camera {
    /// Create a new camera from an `index` and `format`
    /// # Errors
    /// This will error if you either have a bad platform configuration (e.g. `input-v4l` but not on linux) or the backend cannot create the camera (e.g. permission denied).
    pub fn new(index: CameraIndex, format: RequestedFormat) -> Result<Self, NokhwaError> {
        Camera::with_backend(index, format, ApiBackend::Auto)
    }

    /// Create a new camera from an `index`, `format`, and `backend`. `format` can be `None`.
    /// # Errors
    /// This will error if you either have a bad platform configuration (e.g. `input-v4l` but not on linux) or the backend cannot create the camera (e.g. permission denied).
    pub fn with_backend(
        index: CameraIndex,
        format: RequestedFormat,
        backend: ApiBackend,
    ) -> Result<Self, NokhwaError> {
        let camera_backend = init_camera(&index, format, backend)?;

        Ok(Camera {
            idx: index,
            api: backend,
            device: camera_backend,
        })
    }

    /// Create a new `Camera` from raw values.
    /// # Errors
    /// This will error if you either have a bad platform configuration (e.g. `input-v4l` but not on linux) or the backend cannot create the camera (e.g. permission denied).
    #[deprecated(since = "0.10.0", note = "please use `new` instead.")]
    pub fn new_with(
        index: CameraIndex,
        width: u32,
        height: u32,
        fps: u32,
        fourcc: FrameFormat,
        backend: ApiBackend,
    ) -> Result<Self, NokhwaError> {
        let camera_format = CameraFormat::new_from(width, height, fourcc, fps);
        Camera::with_backend(
            index,
            RequestedFormat::with_formats(RequestedFormatType::Exact(camera_format), &[fourcc]),
            backend,
        )
    }

    /// Allows creation of a [`Camera`] with a custom backend. This is useful if you are creating e.g. a custom module.
    ///
    /// You **must** have set a format beforehand.
    #[must_use]
    pub fn with_custom(
        idx: CameraIndex,
        api: ApiBackend,
        device: Box<dyn CaptureBackendTrait>,
    ) -> Self {
        Self { idx, api, device }
    }

    /// Gets the current Camera's index.
    #[must_use]
    pub fn index(&self) -> &CameraIndex {
        &self.idx
    }

    /// Sets the current Camera's index. Note that this re-initializes the camera.
    /// # Errors
    /// The Backend may fail to initialize.
    pub fn set_index(&mut self, new_idx: &CameraIndex) -> Result<(), NokhwaError> {
        {
            self.device.stop_stream()?;
        }
        let new_camera_format = self.device.camera_format();
        let temp = vec![new_camera_format.format()];
        let new_camera = init_camera(
            new_idx,
            RequestedFormat::with_formats(RequestedFormatType::Exact(new_camera_format), &temp),
            self.api,
        )?;
        self.device = new_camera;
        Ok(())
    }

    /// Gets the current Camera's backend
    #[must_use]
    pub fn backend(&self) -> ApiBackend {
        self.api
    }

    /// Sets the current Camera's backend. Note that this re-initializes the camera.
    /// # Errors
    /// The new backend may not exist or may fail to initialize the new camera.
    pub fn set_backend(&mut self, new_backend: ApiBackend) -> Result<(), NokhwaError> {
        {
            self.device.stop_stream()?;
        }
        let new_camera_format = self.device.camera_format();
        let temp = vec![new_camera_format.format()];
        let new_camera = init_camera(
            &self.idx,
            RequestedFormat::with_formats(RequestedFormatType::Exact(new_camera_format), &temp),
            new_backend,
        )?;
        self.device = new_camera;
        Ok(())
    }

    /// Gets the camera information such as Name and Index as a [`CameraInfo`].
    #[must_use]
    pub fn info(&self) -> &CameraInfo {
        self.device.camera_info()
    }

    /// Gets the current [`CameraFormat`].
    #[must_use]
    pub fn camera_format(&self) -> CameraFormat {
        self.device.camera_format()
    }

    /// Forcefully refreshes the stored camera format, bringing it into sync with "reality" (current camera state)
    /// # Errors
    /// If the camera can not get its most recent [`CameraFormat`]. this will error.
    pub fn refresh_camera_format(&mut self) -> Result<CameraFormat, NokhwaError> {
        self.device.refresh_camera_format()?;
        Ok(self.device.camera_format())
    }

    /// Will set the current [`CameraFormat`], using a [`RequestedFormat.`]
    /// This will reset the current stream if used while stream is opened.
    ///
    /// This will also update the cache.
    ///
    /// This will return the new [`CameraFormat`]
    /// # Errors
    /// If nothing fits the requested criteria, this will return an error.
    pub fn set_camera_request(
        &mut self,
        request: RequestedFormat,
    ) -> Result<CameraFormat, NokhwaError> {
        let new_format = request
            .fulfill(self.device.compatible_camera_formats()?.as_slice())
            .ok_or(NokhwaError::GetPropertyError {
                property: "Compatible Camera Format by request".to_string(),
                error: "Failed to fufill".to_string(),
            })?;
        self.device.set_camera_format(new_format)?;
        Ok(new_format)
    }

    #[deprecated(since = "0.10.0", note = "please use `set_camera_request` instead.")]
    /// Will set the current [`CameraFormat`]
    /// This will reset the current stream if used while stream is opened.
    ///
    /// This will also update the cache.
    /// # Errors
    /// If you started the stream and the camera rejects the new camera format, this will return an error.
    pub fn set_camera_format(&mut self, new_fmt: CameraFormat) -> Result<(), NokhwaError> {
        self.device.set_camera_format(new_fmt)
    }

    /// A hashmap of [`Resolution`]s mapped to framerates
    /// # Errors
    /// This will error if the camera is not queryable or a query operation has failed. Some backends will error this out as a [`UnsupportedOperationError`](crate::NokhwaError::UnsupportedOperationError).
    pub fn compatible_list_by_resolution(
        &mut self,
        fourcc: FrameFormat,
    ) -> Result<HashMap<Resolution, Vec<u32>>, NokhwaError> {
        self.device.compatible_list_by_resolution(fourcc)
    }

    /// A Vector of compatible [`FrameFormat`]s.
    /// # Errors
    /// This will error if the camera is not queryable or a query operation has failed. Some backends will error this out as a [`UnsupportedOperationError`](crate::NokhwaError::UnsupportedOperationError).
    pub fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        self.device.compatible_fourcc()
    }

    /// A Vector of available [`CameraFormat`]s.
    /// # Errors
    /// This will error if the camera is not queryable or a query operation has failed. Some backends will error this out as a [`UnsupportedOperationError`](crate::NokhwaError::UnsupportedOperationError).
    pub fn compatible_camera_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
        self.device.compatible_camera_formats()
    }

    /// Gets the current camera resolution (See: [`Resolution`], [`CameraFormat`]). This will force refresh to the current latest if it has changed.
    #[must_use]
    pub fn resolution(&self) -> Resolution {
        self.device.resolution()
    }

    /// Will set the current [`Resolution`]
    /// This will reset the current stream if used while stream is opened.
    ///
    /// This will also update the cache.
    /// # Errors
    /// If you started the stream and the camera rejects the new resolution, this will return an error.
    pub fn set_resolution(&mut self, new_res: Resolution) -> Result<(), NokhwaError> {
        self.device.set_resolution(new_res)
    }

    /// Gets the current camera framerate (See: [`CameraFormat`]).
    #[must_use]
    pub fn frame_rate(&self) -> u32 {
        self.device.frame_rate()
    }

    /// Will set the current framerate
    /// This will reset the current stream if used while stream is opened.
    ///
    /// This will also update the cache.
    /// # Errors
    /// If you started the stream and the camera rejects the new framerate, this will return an error.
    pub fn set_frame_rate(&mut self, new_fps: u32) -> Result<(), NokhwaError> {
        self.device.set_frame_rate(new_fps)
    }

    /// Gets the current camera's frame format (See: [`FrameFormat`], [`CameraFormat`]). This will force refresh to the current latest if it has changed.
    #[must_use]
    pub fn frame_format(&self) -> FrameFormat {
        self.device.frame_format()
    }

    /// Will set the current [`FrameFormat`]
    /// This will reset the current stream if used while stream is opened.
    ///
    /// This will also update the cache.
    /// # Errors
    /// If you started the stream and the camera rejects the new frame format, this will return an error.
    pub fn set_frame_format(&mut self, fourcc: FrameFormat) -> Result<(), NokhwaError> {
        self.device.set_frame_format(fourcc)
    }

    /// Gets the current supported list of [`KnownCameraControl`](crate::utils::KnownCameraControl)
    /// # Errors
    /// If the list cannot be collected, this will error. This can be treated as a "nothing supported".
    pub fn supported_camera_controls(&self) -> Result<Vec<KnownCameraControl>, NokhwaError> {
        Ok(self
            .device
            .camera_controls()?
            .iter()
            .map(CameraControl::control)
            .collect())
    }

    /// Gets the current supported list of [`CameraControl`]s keyed by its name as a `String`.
    /// # Errors
    /// If the list cannot be collected, this will error. This can be treated as a "nothing supported".
    pub fn camera_controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
        let known_controls = self.supported_camera_controls()?;
        let controls = known_controls
            .iter()
            .filter_map(|x| self.camera_control(*x).ok())
            .collect::<Vec<CameraControl>>();

        Ok(controls)
    }

    /// Gets the current supported list of [`CameraControl`]s keyed by its name as a `String`.
    /// # Errors
    /// If the list cannot be collected, this will error. This can be treated as a "nothing supported".
    pub fn camera_controls_string(&self) -> Result<HashMap<String, CameraControl>, NokhwaError> {
        let known_controls = self.supported_camera_controls()?;
        let maybe_camera_controls = known_controls
            .iter()
            .map(|x| (x.to_string(), self.camera_control(*x)))
            .filter(|(_, x)| x.is_ok())
            .map(|(c, x)| (c, Result::unwrap(x)))
            .collect::<Vec<(String, CameraControl)>>();
        let mut control_map = HashMap::with_capacity(maybe_camera_controls.len());

        for (kc, cc) in maybe_camera_controls {
            control_map.insert(kc, cc);
        }

        Ok(control_map)
    }

    /// Gets the current supported list of [`CameraControl`]s keyed by its name as a `String`.
    /// # Errors
    /// If the list cannot be collected, this will error. This can be treated as a "nothing supported".
    pub fn camera_controls_known_camera_controls(
        &self,
    ) -> Result<HashMap<KnownCameraControl, CameraControl>, NokhwaError> {
        let known_controls = self.supported_camera_controls()?;
        let maybe_camera_controls = known_controls
            .iter()
            .map(|x| (*x, self.camera_control(*x)))
            .filter(|(_, x)| x.is_ok())
            .map(|(c, x)| (c, Result::unwrap(x)))
            .collect::<Vec<(KnownCameraControl, CameraControl)>>();
        let mut control_map = HashMap::with_capacity(maybe_camera_controls.len());

        for (kc, cc) in maybe_camera_controls {
            control_map.insert(kc, cc);
        }

        Ok(control_map)
    }

    /// Gets the value of [`KnownCameraControl`].
    /// # Errors
    /// If the `control` is not supported or there is an error while getting the camera control values (e.g. unexpected value, too high, etc)
    /// this will error.
    pub fn camera_control(
        &self,
        control: KnownCameraControl,
    ) -> Result<CameraControl, NokhwaError> {
        self.device.camera_control(control)
    }

    /// Sets the control to `control` in the camera.
    /// Usually, the pipeline is calling [`camera_control()`](crate::camera_traits::CaptureBackendTrait::camera_control), getting a camera control that way
    /// then calling [`value()`](crate::utils::CameraControl::value()) to get a [`ControlValueSetter`](crate::utils::ControlValueSetter) and setting the value that way.
    /// # Errors
    /// If the `control` is not supported, the value is invalid (less than min, greater than max, not in step), or there was an error setting the control,
    /// this will error.
    pub fn set_camera_control(
        &mut self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        self.device.set_camera_control(id, value)
    }

    /// Will open the camera stream with set parameters. This will be called internally if you try and call [`frame()`](CaptureBackendTrait::frame()) before you call [`open_stream()`](CaptureBackendTrait::open_stream()).
    /// # Errors
    /// If the specific backend fails to open the camera (e.g. already taken, busy, doesn't exist anymore) this will error.
    pub fn open_stream(&mut self) -> Result<(), NokhwaError> {
        self.device.open_stream()
    }

    /// Checks if stream if open. If it is, it will return true.
    #[must_use]
    pub fn is_stream_open(&self) -> bool {
        self.device.is_stream_open()
    }

    /// Will get a frame from the camera as a Raw RGB image buffer. Depending on the backend, if you have not called [`open_stream()`](CaptureBackendTrait::open_stream()) before you called this,
    /// it will either return an error.
    /// # Errors
    /// If the backend fails to get the frame (e.g. already taken, busy, doesn't exist anymore), the decoding fails (e.g. MJPEG -> u8), or [`open_stream()`](CaptureBackendTrait::open_stream()) has not been called yet,
    /// this will error.
    pub fn frame(&mut self) -> Result<Buffer, NokhwaError> {
        self.device.frame()
    }

    /// Will get a frame from the camera **without** any processing applied, meaning you will usually get a frame you need to decode yourself.
    /// # Errors
    /// If the backend fails to get the frame (e.g. already taken, busy, doesn't exist anymore), or [`open_stream()`](CaptureBackendTrait::open_stream()) has not been called yet, this will error.
    pub fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
        match self.device.frame_raw() {
            Ok(f) => Ok(f),
            Err(why) => Err(why),
        }
    }

    /// Directly writes the current frame into said `buffer`.
    /// # Errors
    /// If the backend fails to get the frame (e.g. already taken, busy, doesn't exist anymore), or [`open_stream()`](CaptureBackendTrait::open_stream()) has not been called yet, this will error.
    pub fn write_frame_to_buffer<F: FormatDecoder>(
        &mut self,
        buffer: &mut [u8],
    ) -> Result<(), NokhwaError> {
        self.device.frame()?.decode_image_to_buffer::<F>(buffer)
    }

    #[cfg(feature = "output-wgpu")]
    #[cfg_attr(feature = "docs-features", doc(cfg(feature = "output-wgpu")))]
    /// Directly copies a frame to a Wgpu texture. This will automatically convert the frame into a RGBA frame.
    /// # Errors
    /// If the frame cannot be captured or the resolution is 0 on any axis, this will error.
    pub fn frame_texture<'a, F: FormatDecoder>(
        &mut self,
        device: &WgpuDevice,
        queue: &WgpuQueue,
        label: Option<&'a str>,
    ) -> Result<WgpuTexture, NokhwaError> {
        self.device.frame_texture(device, queue, label)
    }

    /// Will drop the stream.
    /// # Errors
    /// Please check the `Quirks` section of each backend.
    pub fn stop_stream(&mut self) -> Result<(), NokhwaError> {
        self.device.stop_stream()
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        self.stop_stream().unwrap();
    }
}

// TODO: Update as we go
#[allow(clippy::ifs_same_cond)]
fn figure_out_auto() -> Option<ApiBackend> {
    let platform = std::env::consts::OS;
    let mut cap = ApiBackend::Auto;
    if cfg!(feature = "input-v4l") && platform == "linux" {
        cap = ApiBackend::Video4Linux;
    } else if cfg!(feature = "input-msmf") && platform == "windows" {
        cap = ApiBackend::MediaFoundation;
    } else if cfg!(feature = "input-avfoundation") && (platform == "macos" || platform == "ios") {
        cap = ApiBackend::AVFoundation;
    } else if cfg!(feature = "input-opencv") {
        cap = ApiBackend::OpenCv;
    }
    if cap == ApiBackend::Auto {
        return None;
    }
    Some(cap)
}

fn create_backend(
    backend: ApiBackend,
    index: &CameraIndex,
    format: RequestedFormat,
) -> Result<Box<dyn CaptureBackendTrait>, NokhwaError> {
    match backend {
        #[cfg(all(feature = "input-v4l", target_os = "linux"))]
        ApiBackend::Video4Linux => {
            use crate::backends::capture::V4LCaptureDevice;
            Ok(Box::new(V4LCaptureDevice::new(index, format)?))
        }

        #[cfg(all(feature = "input-msmf", target_os = "windows"))]
        ApiBackend::MediaFoundation => {
            use crate::backends::capture::MediaFoundationCaptureDevice;
            Ok(Box::new(MediaFoundationCaptureDevice::new(index, format)?))
        }

        #[cfg(all(
            feature = "input-avfoundation",
            any(target_os = "macos", target_os = "ios")
        ))]
        ApiBackend::AVFoundation => {
            use crate::backends::capture::AVFoundationCaptureDevice;
            Ok(Box::new(AVFoundationCaptureDevice::new(index, format)?))
        }

        #[cfg(feature = "input-opencv")]
        ApiBackend::OpenCv => {
            use crate::backends::capture::OpenCvCaptureDevice;
            Ok(Box::new(OpenCvCaptureDevice::new(index, format)?))
        }

        _ => Err(NokhwaError::NotImplementedError(format!(
            "Backend {backend} is not available (not enabled or wrong platform)."
        ))),
    }
}

fn init_camera(
    index: &CameraIndex,
    format: RequestedFormat,
    backend: ApiBackend,
) -> Result<Box<dyn CaptureBackendTrait>, NokhwaError> {
    let resolved = match backend {
        ApiBackend::Auto => figure_out_auto().ok_or_else(|| {
            NokhwaError::NotImplementedError(
                "No suitable backend found for the current platform.".to_string(),
            )
        })?,
        other => other,
    };

    create_backend(resolved, index, format)
}

#[cfg(feature = "camera-sync-impl")]
unsafe impl Send for Camera {}
