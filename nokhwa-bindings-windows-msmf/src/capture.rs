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
use crate::wmf::MediaFoundationDevice;
use nokhwa_core::{
    buffer::{Buffer, TimestampKind},
    error::NokhwaError,
    traits::{CameraDevice, FrameSource},
    types::{
        all_known_camera_controls, color_frame_formats, ApiBackend, CameraControl, CameraFormat,
        CameraIndex, CameraInfo, ControlValueSetter, FrameFormat, KnownCameraControl,
        RequestedFormat, RequestedFormatType,
    },
};
use std::borrow::Cow;

/// The backend that deals with Media Foundation on Windows.
/// Implements [`CameraDevice`] and [`FrameSource`].
///
/// Note: This requires Windows 7 or newer to work.
/// # Quirks
/// - This does build on non-windows platforms, however when you do the backend will be empty and will return an error for any given operation.
/// - Please check [`nokhwa-bindings-windows-msmf`](https://github.com/l1npengtul/nokhwa/tree/senpai/nokhwa-bindings-windows-msmf) source code to see the internal raw interface.
/// - The symbolic link for the device is listed in the `misc` attribute of the [`CameraInfo`].
/// - The names may contain invalid characters since they were converted from UTF16.
/// - When you call new or drop the struct, `initialize`/`de_initialize` will automatically be called.
pub struct MediaFoundationCaptureDevice {
    inner: MediaFoundationDevice,
    info: CameraInfo,
}

// SAFETY: MediaFoundationCaptureDevice is safe to Send (move) between threads because:
// - All access goes through &mut self, so after a move the new thread has exclusive
//   ownership — no aliasing across threads occurs. We do NOT implement Sync.
// - The inner IMFSourceReader (COM interface) wraps NonNull<c_void> which is !Send by
//   default. nokhwa's MSMF backend initializes COM in the **multi-threaded apartment**
//   (MTA) — see `nokhwa_bindings_windows_msmf::wmf::initialize_mf`, which calls
//   `CoInitializeEx(None, COINIT_MULTITHREADED | COINIT_DISABLE_OLE1DDE)`. MTA objects
//   are not tied to a specific thread and may be accessed from any thread holding a
//   reference, so moving exclusive ownership to another thread is sound.
// - CameraInfo and CameraFormat are plain data types that are already Send.
unsafe impl Send for MediaFoundationCaptureDevice {}

impl MediaFoundationCaptureDevice {
    /// Creates a new capture device using the Media Foundation backend. Indexes are gives to devices by the OS, and usually numbered by order of discovery.
    /// # Errors
    /// This function will error if Media Foundation fails to get the device.
    pub fn new(index: &CameraIndex, camera_fmt: RequestedFormat) -> Result<Self, NokhwaError> {
        let mut mf_device = MediaFoundationDevice::new(index.clone())?;

        let info = CameraInfo::new(
            &mf_device.name(),
            "MediaFoundation Camera Device",
            &mf_device.symlink(),
            index.clone(),
        );

        let availible = mf_device.compatible_format_list()?;

        let desired = camera_fmt
            .fulfill(&availible)
            .ok_or(NokhwaError::InitializeError {
                backend: ApiBackend::MediaFoundation,
                error: "Failed to fulfill requested format".to_string(),
            })?;

        mf_device.set_format(desired)?;

        let mut new_cam = MediaFoundationCaptureDevice {
            inner: mf_device,
            info,
        };
        new_cam.refresh_camera_format()?;
        Ok(new_cam)
    }

    /// Create a new Media Foundation Device with desired settings.
    /// # Errors
    /// This function will error if Media Foundation fails to get the device.
    #[deprecated(since = "0.10.0", note = "please use `new` instead.")]
    pub fn new_with(
        index: &CameraIndex,
        width: u32,
        height: u32,
        fps: u32,
        fourcc: FrameFormat,
    ) -> Result<Self, NokhwaError> {
        let camera_format = RequestedFormat::with_formats(
            RequestedFormatType::Exact(CameraFormat::new_from(width, height, fourcc, fps)),
            color_frame_formats(),
        );
        MediaFoundationCaptureDevice::new(index, camera_format)
    }

    /// Gets the list of supported [`KnownCameraControl`]s
    /// # Errors
    /// May error if there is an error from `MediaFoundation`.
    pub fn supported_camera_controls(&self) -> Vec<KnownCameraControl> {
        let mut supported_camera_controls: Vec<KnownCameraControl> = vec![];

        for camera_control in all_known_camera_controls() {
            if let Ok(supported) = self.inner.control(camera_control) {
                supported_camera_controls.push(supported.control());
            }
        }
        supported_camera_controls
    }
}

impl MediaFoundationCaptureDevice {
    /// Refreshes the cached camera format by querying the Media Foundation device.
    /// Kept as an inherent helper after the trait split; used internally by `frame()`.
    fn refresh_camera_format(&mut self) -> Result<(), NokhwaError> {
        let _ = self.inner.format_refreshed()?;
        Ok(())
    }

    /// Look up a single control by its [`KnownCameraControl`] identifier.
    /// Kept as an inherent helper after the trait split; used internally by `controls()`.
    pub fn camera_control(
        &self,
        control: KnownCameraControl,
    ) -> Result<CameraControl, NokhwaError> {
        self.inner.control(control)
    }
}

impl CameraDevice for MediaFoundationCaptureDevice {
    fn backend(&self) -> ApiBackend {
        ApiBackend::MediaFoundation
    }

    fn info(&self) -> &CameraInfo {
        &self.info
    }

    fn controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
        let mut camera_ctrls = Vec::with_capacity(15);
        for ctrl_id in all_known_camera_controls() {
            let Ok(ctrl) = self.camera_control(ctrl_id) else {
                continue;
            };

            camera_ctrls.push(ctrl);
        }
        camera_ctrls.shrink_to_fit();
        Ok(camera_ctrls)
    }

    fn set_control(
        &mut self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        self.inner.set_control(id, value)
    }
}

impl FrameSource for MediaFoundationCaptureDevice {
    fn negotiated_format(&self) -> CameraFormat {
        self.inner.format()
    }

    fn set_format(&mut self, new_fmt: CameraFormat) -> Result<(), NokhwaError> {
        self.inner.set_format(new_fmt)
    }

    fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
        self.inner.compatible_format_list()
    }

    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        let mut formats: Vec<FrameFormat> = self
            .inner
            .compatible_format_list()?
            .into_iter()
            .map(|fmt| fmt.format())
            .collect();
        formats.sort();
        formats.dedup();
        Ok(formats)
    }

    fn open(&mut self) -> Result<(), NokhwaError> {
        self.inner.start_stream()
    }

    fn is_open(&self) -> bool {
        self.inner.is_stream_open()
    }

    fn frame(&mut self) -> Result<Buffer, NokhwaError> {
        self.refresh_camera_format()?;
        let self_ctrl = self.negotiated_format();
        let (bytes, capture_ts) = self.inner.raw_bytes()?;
        let ts = capture_ts.map(|ts| (ts, TimestampKind::MonotonicClock));
        Ok(match bytes {
            Cow::Owned(vec) => {
                Buffer::from_vec_with_timestamp(self_ctrl.resolution(), vec, self_ctrl.format(), ts)
            }
            Cow::Borrowed(slice) => {
                Buffer::with_timestamp(self_ctrl.resolution(), slice, self_ctrl.format(), ts)
            }
        })
    }

    fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError> {
        let (bytes, _capture_ts) = self.inner.raw_bytes()?;
        Ok(bytes)
    }

    fn close(&mut self) -> Result<(), NokhwaError> {
        self.inner.stop_stream();
        Ok(())
    }
}
