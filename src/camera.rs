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

use nokhwa_core::format_types::{CaptureFormat, Mjpeg};
use nokhwa_core::frame::Frame;
#[cfg(feature = "output-wgpu")]
use nokhwa_core::traits::RawTextureData;
use nokhwa_core::types::RequestedFormatType;
use nokhwa_core::{
    buffer::Buffer,
    error::NokhwaError,
    traits::CaptureBackendTrait,
    types::{
        color_frame_formats, ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo,
        ControlValueSetter, FrameFormat, KnownCameraControl, RequestedFormat, Resolution,
    },
};
use std::{borrow::Cow, collections::HashMap, marker::PhantomData, time::Duration};
#[cfg(feature = "output-wgpu")]
use wgpu::{Device as WgpuDevice, Queue as WgpuQueue, Texture as WgpuTexture};

/// The main camera capture struct, abstracting over platform-specific backends.
///
/// `Camera<F>` is parameterized by a [`CaptureFormat`] marker type (e.g.
/// [`Mjpeg`], [`Yuyv`](nokhwa_core::format_types::Yuyv)) that pins the camera
/// to a specific wire format at compile time. Use [`Camera::open`] to create an
/// instance, which verifies at open time that the hardware supports the
/// requested format.
///
/// The default type parameter is [`Mjpeg`], so `Camera` (without an explicit
/// type) works like the pre-0.12 non-generic Camera.
///
/// # Creating a Camera (type-safe)
///
/// ```no_run
/// use nokhwa::Camera;
/// use nokhwa::utils::{CameraIndex, RequestedFormatType};
/// use nokhwa_core::format_types::Mjpeg;
///
/// let mut camera = Camera::open::<Mjpeg>(
///     CameraIndex::Index(0),
///     RequestedFormatType::AbsoluteHighestResolution,
/// ).expect("failed to open camera");
/// ```
///
/// # Typed frame capture
///
/// ```no_run
/// # use nokhwa::Camera;
/// # use nokhwa::utils::{CameraIndex, RequestedFormatType};
/// # use nokhwa_core::format_types::Mjpeg;
/// use nokhwa_core::frame::IntoRgb;
///
/// # let mut camera = Camera::open::<Mjpeg>(
/// #     CameraIndex::Index(0),
/// #     RequestedFormatType::AbsoluteHighestResolution,
/// # ).unwrap();
/// camera.open_stream().unwrap();
/// let frame = camera.frame_typed().unwrap();
/// let rgb_image = frame.into_rgb().materialize().unwrap();
/// ```
///
/// # Callback-based capture
///
/// For background capture with a callback, see [`CallbackCamera`](crate::threaded::CallbackCamera)
/// (requires the `output-threaded` feature).
///
/// # Cleaning up
///
/// The stream is automatically stopped when `Camera` is dropped. You can also call
/// [`Camera::stop_stream`] explicitly.
pub struct Camera<F: CaptureFormat = Mjpeg> {
    idx: CameraIndex,
    api: ApiBackend,
    device: Box<dyn CaptureBackendTrait + Send>,
    _format: PhantomData<F>,
}

impl Camera {
    /// Opens a camera with type-safe format selection.
    ///
    /// The camera is configured to capture in format `F` (e.g. `Mjpeg`, `Yuyv`).
    /// The `requested` parameter controls resolution/framerate selection strategy.
    ///
    /// # Errors
    /// Returns an error if the platform backend cannot be initialized, the camera
    /// doesn't support the requested format, or permissions are denied.
    pub fn open<F: CaptureFormat>(
        index: CameraIndex,
        requested: RequestedFormatType,
    ) -> Result<Camera<F>, NokhwaError> {
        Camera::open_with_backend::<F>(index, requested, ApiBackend::Auto)
    }

    /// Opens a camera with type-safe format selection and an explicit backend.
    ///
    /// # Errors
    /// Returns an error if the backend cannot be initialized or the camera
    /// doesn't support format `F`.
    pub fn open_with_backend<F: CaptureFormat>(
        index: CameraIndex,
        requested: RequestedFormatType,
        backend: ApiBackend,
    ) -> Result<Camera<F>, NokhwaError> {
        let req = RequestedFormat::with_formats(requested, &[F::FRAME_FORMAT]);
        let camera_backend = init_camera(&index, req, backend.clone())?;
        Camera::<F>::from_backend(index, backend, camera_backend)
    }
}

impl<F: CaptureFormat> Camera<F> {
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
        let camera_backend = init_camera(&index, format, backend.clone())?;
        Self::from_backend(index, backend, camera_backend)
    }

    fn from_backend(
        index: CameraIndex,
        api: ApiBackend,
        device: Box<dyn CaptureBackendTrait + Send>,
    ) -> Result<Self, NokhwaError> {
        let actual_format = device.frame_format();
        if actual_format != F::FRAME_FORMAT {
            return Err(NokhwaError::SetPropertyError {
                property: "FrameFormat".to_string(),
                value: format!("{actual_format:?}"),
                error: format!(
                    "camera negotiated {:?} but Camera<{:?}> requires {:?}",
                    actual_format,
                    F::FRAME_FORMAT,
                    F::FRAME_FORMAT
                ),
            });
        }
        Ok(Camera {
            idx: index,
            api,
            device,
            _format: PhantomData,
        })
    }

    /// Allows creation of a [`Camera`] with a custom backend. This is useful if you are creating e.g. a custom module.
    ///
    /// You **must** have set a format beforehand.
    #[must_use]
    pub fn with_custom(
        idx: CameraIndex,
        api: ApiBackend,
        device: Box<dyn CaptureBackendTrait + Send>,
    ) -> Self {
        Self {
            idx,
            api,
            device,
            _format: PhantomData,
        }
    }

    /// Create a new camera from an `index`, automatically selecting the highest available resolution.
    ///
    /// Accepts all color frame formats (MJPEG, YUYV, NV12, RAWRGB, RAWBGR, GRAY).
    /// # Errors
    /// This will error if you either have a bad platform configuration (e.g. `input-v4l` but not on linux) or the backend cannot create the camera (e.g. permission denied).
    pub fn new_with_highest_resolution(index: CameraIndex) -> Result<Self, NokhwaError> {
        Self::new(
            index,
            RequestedFormat::with_formats(
                RequestedFormatType::AbsoluteHighestResolution,
                color_frame_formats(),
            ),
        )
    }

    /// Create a new camera from an `index`, automatically selecting the highest available frame rate.
    ///
    /// Accepts all color frame formats (MJPEG, YUYV, NV12, RAWRGB, RAWBGR, GRAY).
    /// # Errors
    /// This will error if you either have a bad platform configuration (e.g. `input-v4l` but not on linux) or the backend cannot create the camera (e.g. permission denied).
    pub fn new_with_highest_framerate(index: CameraIndex) -> Result<Self, NokhwaError> {
        Self::new(
            index,
            RequestedFormat::with_formats(
                RequestedFormatType::AbsoluteHighestFrameRate,
                color_frame_formats(),
            ),
        )
    }

    /// Captures a frame and returns a type-safe [`Frame<F>`].
    ///
    /// The returned frame carries the compile-time format tag, enabling
    /// type-checked conversions (e.g. `frame.into_rgb().materialize()`).
    ///
    /// # Errors
    /// Returns an error if the stream is not open or frame capture fails.
    pub fn frame_typed(&mut self) -> Result<Frame<F>, NokhwaError> {
        let buffer = self.device.frame()?;
        Frame::try_new(buffer)
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
            self.api.clone(),
        )?;
        self.device = new_camera;
        Ok(())
    }

    /// Gets the current Camera's backend
    #[must_use]
    pub fn backend(&self) -> &ApiBackend {
        &self.api
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
                error: "Failed to fulfill".to_string(),
            })?;
        self.device.set_camera_format(new_format)?;
        Ok(new_format)
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

    /// Gets the current supported list of [`CameraControl`]s.
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

    /// Gets the current supported list of [`CameraControl`]s keyed by control name.
    /// # Errors
    /// If the list cannot be collected, this will error. This can be treated as a "nothing supported".
    pub fn camera_controls_by_name(&self) -> Result<HashMap<String, CameraControl>, NokhwaError> {
        let known_controls = self.supported_camera_controls()?;
        let control_map = known_controls
            .iter()
            .filter_map(|x| self.camera_control(*x).ok().map(|cc| (x.to_string(), cc)))
            .collect::<HashMap<String, CameraControl>>();

        Ok(control_map)
    }

    /// Gets the current supported list of [`CameraControl`]s keyed by [`KnownCameraControl`].
    /// # Errors
    /// If the list cannot be collected, this will error. This can be treated as a "nothing supported".
    pub fn camera_controls_by_id(
        &self,
    ) -> Result<HashMap<KnownCameraControl, CameraControl>, NokhwaError> {
        let known_controls = self.supported_camera_controls()?;
        let control_map = known_controls
            .iter()
            .filter_map(|x| self.camera_control(*x).ok().map(|cc| (*x, cc)))
            .collect::<HashMap<KnownCameraControl, CameraControl>>();

        Ok(control_map)
    }

    /// Use [`camera_controls_by_name()`](Camera::camera_controls_by_name) instead.
    /// # Errors
    /// See [`camera_controls_by_name()`](Camera::camera_controls_by_name).
    #[deprecated(since = "0.11.0", note = "renamed to camera_controls_by_name()")]
    pub fn camera_controls_string(&self) -> Result<HashMap<String, CameraControl>, NokhwaError> {
        self.camera_controls_by_name()
    }

    /// Use [`camera_controls_by_id()`](Camera::camera_controls_by_id) instead.
    /// # Errors
    /// See [`camera_controls_by_id()`](Camera::camera_controls_by_id).
    #[deprecated(since = "0.11.0", note = "renamed to camera_controls_by_id()")]
    pub fn camera_controls_known_camera_controls(
        &self,
    ) -> Result<HashMap<KnownCameraControl, CameraControl>, NokhwaError> {
        self.camera_controls_by_id()
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
    /// # Errors
    /// If the `control` is not supported, the value is invalid, or there was an error setting the control, this will error.
    pub fn set_camera_control(
        &mut self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        self.device.set_camera_control(id, value)
    }

    /// Will open the camera stream with set parameters.
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

    /// Will get a frame from the camera as a [`Buffer`]. This is the untyped version;
    /// prefer [`frame_typed()`](Self::frame_typed) for type-safe access.
    /// # Errors
    /// If the backend fails to get the frame, or [`open_stream()`](CaptureBackendTrait::open_stream()) has not been called yet, this will error.
    pub fn frame(&mut self) -> Result<Buffer, NokhwaError> {
        self.device.frame()
    }

    /// Will get a frame from the camera as a [`Buffer`], but with a timeout.
    /// # Errors
    /// If the backend fails to get the frame within the timeout, this will error.
    pub fn frame_timeout(&mut self, duration: Duration) -> Result<Buffer, NokhwaError> {
        struct SendPtr(*mut dyn CaptureBackendTrait);
        unsafe impl Send for SendPtr {}
        impl SendPtr {
            unsafe fn frame_timeout(&self, duration: Duration) -> Result<Buffer, NokhwaError> {
                unsafe { (&mut *self.0).frame_timeout(duration) }
            }
        }

        if duration.is_zero() {
            return Err(NokhwaError::TimeoutError(duration));
        }

        let start = std::time::Instant::now();
        let (tx, rx) = std::sync::mpsc::channel();
        let ptr = SendPtr(std::ptr::from_mut::<dyn CaptureBackendTrait>(
            &mut *self.device,
        ));
        let handle = std::thread::spawn(move || {
            let _ = tx.send(unsafe { ptr.frame_timeout(duration) });
        });
        let remaining = duration.saturating_sub(start.elapsed());
        let result = match rx.recv_timeout(remaining) {
            Ok(result) => result,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                Err(NokhwaError::TimeoutError(duration))
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err(NokhwaError::read_frame("frame capture thread panicked"))
            }
        };
        let _ = handle.join();
        result
    }

    /// Will get a frame from the camera **without** any processing applied.
    /// # Errors
    /// If the backend fails to get the frame, or [`open_stream()`](CaptureBackendTrait::open_stream()) has not been called yet, this will error.
    pub fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
        self.device.frame_raw()
    }

    #[cfg(feature = "output-wgpu")]
    #[cfg_attr(feature = "docs-features", doc(cfg(feature = "output-wgpu")))]
    /// Directly copies a frame to a Wgpu texture as RGBA.
    /// # Errors
    /// If the frame cannot be captured or the resolution is 0 on any axis, this will error.
    pub fn frame_texture(
        &mut self,
        device: &WgpuDevice,
        queue: &WgpuQueue,
        label: Option<&str>,
    ) -> Result<WgpuTexture, NokhwaError> {
        self.device.frame_texture(device, queue, label)
    }

    #[cfg(feature = "output-wgpu")]
    #[cfg_attr(feature = "docs-features", doc(cfg(feature = "output-wgpu")))]
    /// Copies a frame to a Wgpu texture in the camera's native pixel format.
    /// # Errors
    /// If the frame cannot be captured or the resolution is 0 on any axis, this will error.
    pub fn frame_texture_raw(
        &mut self,
        device: &WgpuDevice,
        queue: &WgpuQueue,
        label: Option<&str>,
    ) -> Result<RawTextureData, NokhwaError> {
        self.device.frame_texture_raw(device, queue, label)
    }

    /// Will drop the stream.
    /// # Errors
    /// Please check the `Quirks` section of each backend.
    pub fn stop_stream(&mut self) -> Result<(), NokhwaError> {
        self.device.stop_stream()
    }
}

impl<F: CaptureFormat> Drop for Camera<F> {
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
    backend: &ApiBackend,
    index: &CameraIndex,
    format: RequestedFormat,
) -> Result<Box<dyn CaptureBackendTrait + Send>, NokhwaError> {
    #[cfg(all(
        feature = "input-avfoundation",
        any(target_os = "macos", target_os = "ios")
    ))]
    use crate::backends::capture::AVFoundationCaptureDevice;
    #[cfg(all(feature = "input-msmf", target_os = "windows"))]
    use crate::backends::capture::MediaFoundationCaptureDevice;
    #[cfg(feature = "input-opencv")]
    use crate::backends::capture::OpenCvCaptureDevice;
    #[cfg(all(feature = "input-v4l", target_os = "linux"))]
    use crate::backends::capture::V4LCaptureDevice;

    match backend {
        ApiBackend::Auto => Err(NokhwaError::NotImplementedError(
            "ApiBackend::Auto must be resolved before creating a backend.".to_string(),
        )),

        #[cfg(all(feature = "input-v4l", target_os = "linux"))]
        ApiBackend::Video4Linux => Ok(Box::new(V4LCaptureDevice::new(index, format)?)),

        #[cfg(all(feature = "input-msmf", target_os = "windows"))]
        ApiBackend::MediaFoundation => {
            Ok(Box::new(MediaFoundationCaptureDevice::new(index, format)?))
        }

        #[cfg(all(
            feature = "input-avfoundation",
            any(target_os = "macos", target_os = "ios")
        ))]
        ApiBackend::AVFoundation => Ok(Box::new(AVFoundationCaptureDevice::new(index, format)?)),

        #[cfg(feature = "input-opencv")]
        ApiBackend::OpenCv => Ok(Box::new(OpenCvCaptureDevice::new(index, format)?)),

        _ => Err(NokhwaError::NotImplementedError(format!(
            "Backend {backend} is not available (not enabled or wrong platform)."
        ))),
    }
}

fn init_camera(
    index: &CameraIndex,
    format: RequestedFormat,
    backend: ApiBackend,
) -> Result<Box<dyn CaptureBackendTrait + Send>, NokhwaError> {
    let resolved = match backend {
        ApiBackend::Auto => figure_out_auto().ok_or_else(|| {
            NokhwaError::NotImplementedError(
                "No suitable backend found for the current platform.".to_string(),
            )
        })?,
        other => other,
    };

    create_backend(&resolved, index, format)
}
