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

use crate::{
    buffer::Buffer,
    error::NokhwaError,
    types::{
        ApiBackend, CameraControl, CameraFormat, CameraInfo, ControlValueSetter, FrameFormat,
        KnownCameraControl,
    },
};
use std::{borrow::Cow, time::Duration};

#[cfg(feature = "wgpu-types")]
use wgpu::{
    Device as WgpuDevice, Extent3d, Queue as WgpuQueue, TexelCopyBufferLayout,
    Texture as WgpuTexture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages,
};

#[cfg(feature = "wgpu-types")]
use crate::wgpu::{raw_texture_layout, RawTextureData};

/// Base capabilities present on every camera: identity and camera-wide controls.
pub trait CameraDevice {
    fn backend(&self) -> ApiBackend;
    fn info(&self) -> &CameraInfo;
    /// # Errors
    /// Returns [`NokhwaError`] if enumerating controls fails.
    fn controls(&self) -> Result<Vec<CameraControl>, NokhwaError>;
    /// # Errors
    /// Returns [`NokhwaError`] if setting the control fails.
    fn set_control(
        &mut self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError>;
}

/// Continuous-frame capability: webcam streaming or DSLR live view.
///
/// After `open()` succeeds, `frame()` blocks until the next frame is produced
/// by the device. `close()` halts the stream.
pub trait FrameSource: CameraDevice {
    fn negotiated_format(&self) -> CameraFormat;
    /// # Errors
    /// Returns [`NokhwaError`] if the format is not supported by the backend.
    fn set_format(&mut self, f: CameraFormat) -> Result<(), NokhwaError>;
    /// # Errors
    /// Returns [`NokhwaError`] if enumerating compatible formats fails.
    fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError>;
    /// # Errors
    /// Returns [`NokhwaError`] if enumerating compatible fourcc codes fails.
    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError>;

    /// # Errors
    /// Returns [`NokhwaError`] if opening the stream fails.
    fn open(&mut self) -> Result<(), NokhwaError>;
    fn is_open(&self) -> bool;
    /// # Errors
    /// Returns [`NokhwaError`] if reading a frame fails.
    fn frame(&mut self) -> Result<Buffer, NokhwaError>;
    /// # Errors
    /// Returns [`NokhwaError`] if reading a frame fails or times out.
    fn frame_timeout(&mut self, _duration: Duration) -> Result<Buffer, NokhwaError> {
        self.frame()
    }
    /// # Errors
    /// Returns [`NokhwaError`] if reading a frame fails.
    fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError>;
    /// # Errors
    /// Returns [`NokhwaError`] if closing the stream fails.
    fn close(&mut self) -> Result<(), NokhwaError>;

    #[must_use]
    fn decoded_buffer_size(&self, alpha: bool) -> usize {
        let cfmt = self.negotiated_format();
        let resolution = cfmt.resolution();
        let pxwidth = match cfmt.format() {
            FrameFormat::MJPEG
            | FrameFormat::YUYV
            | FrameFormat::RAWRGB
            | FrameFormat::RAWBGR
            | FrameFormat::NV12 => 3,
            FrameFormat::GRAY => 1,
        };
        if alpha {
            return (resolution.width() * resolution.height() * (pxwidth + 1)) as usize;
        }
        (resolution.width() * resolution.height() * pxwidth) as usize
    }

    #[cfg(feature = "wgpu-types")]
    #[cfg_attr(feature = "docs-features", doc(cfg(feature = "wgpu-types")))]
    fn frame_texture(
        &mut self,
        device: &WgpuDevice,
        queue: &WgpuQueue,
        label: Option<&str>,
    ) -> Result<WgpuTexture, NokhwaError> {
        use crate::frame;
        use wgpu::{Origin3d, TexelCopyTextureInfoBase};

        let buffer = self.frame()?;
        let resolution = buffer.resolution();
        let fcc = buffer.source_frame_format();
        let rgba_data = frame::convert_to_rgba(fcc, resolution, buffer.buffer())?;
        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            image::ImageBuffer::from_raw(resolution.width_x, resolution.height_y, rgba_data)
                .ok_or(NokhwaError::ProcessFrameError {
                    src: fcc,
                    destination: "Rgba".to_string(),
                    error: "Failed to create ImageBuffer".to_string(),
                })?;
        let texture_size = Extent3d {
            width: img.width(),
            height: img.height(),
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&TextureDescriptor {
            label,
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            TexelCopyTextureInfoBase {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d { x: 0, y: 0, z: 0 },
                aspect: TextureAspect::All,
            },
            &img,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * img.width()),
                rows_per_image: Some(img.height()),
            },
            texture_size,
        );
        Ok(texture)
    }

    #[cfg(feature = "wgpu-types")]
    #[cfg_attr(feature = "docs-features", doc(cfg(feature = "wgpu-types")))]
    fn frame_texture_raw(
        &mut self,
        device: &WgpuDevice,
        queue: &WgpuQueue,
        label: Option<&str>,
    ) -> Result<RawTextureData, NokhwaError> {
        use wgpu::{Origin3d, TexelCopyTextureInfoBase};

        let source_format = self.negotiated_format().format();
        let resolution = self.negotiated_format().resolution();
        let raw = self.frame_raw()?;
        let (tex_format, tex_size, bytes_per_row) = raw_texture_layout(source_format, resolution)?;
        let expected_size = (bytes_per_row * tex_size.height) as usize;
        if raw.len() < expected_size {
            return Err(NokhwaError::ProcessFrameError {
                src: source_format,
                destination: "RawTextureData".to_string(),
                error: format!(
                    "raw buffer ({} bytes) smaller than expected ({} bytes)",
                    raw.len(),
                    expected_size
                ),
            });
        }
        let texture = device.create_texture(&TextureDescriptor {
            label,
            size: tex_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: tex_format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            TexelCopyTextureInfoBase {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d { x: 0, y: 0, z: 0 },
                aspect: TextureAspect::All,
            },
            &raw,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(tex_size.height),
            },
            tex_size,
        );
        Ok(RawTextureData {
            texture,
            source_frame_format: source_format,
            texture_format: tex_format,
            resolution,
        })
    }
}

/// Shutter-trigger still-image capability (DSLR, industrial trigger cameras).
///
/// `trigger()` returns immediately. `take_picture(timeout)` blocks up to
/// `timeout` for the resulting image. `take_picture(Duration::ZERO)` is a
/// non-blocking probe — returns `Err(NokhwaError::TimeoutError(_))` if no
/// picture is buffered. Ordering/dropping semantics across multiple triggers
/// are backend-specific.
pub trait ShutterCapture: CameraDevice {
    /// # Errors
    /// Returns [`NokhwaError`] if triggering the shutter fails.
    fn trigger(&mut self) -> Result<(), NokhwaError>;
    /// # Errors
    /// Returns [`NokhwaError`] if no picture is available within `timeout`.
    fn take_picture(&mut self, timeout: Duration) -> Result<Buffer, NokhwaError>;

    /// Locks the camera's physical UI controls so that host-side commands have
    /// exclusive effect. Release with [`unlock_ui`](Self::unlock_ui). No-op
    /// default — webcams do not need this.
    /// # Errors
    /// Returns [`NokhwaError`] if acquiring the UI lock fails.
    fn lock_ui(&mut self) -> Result<(), NokhwaError> {
        Ok(())
    }
    /// Releases the UI lock acquired by [`lock_ui`](Self::lock_ui).
    /// # Errors
    /// Returns [`NokhwaError`] if releasing the UI lock fails.
    fn unlock_ui(&mut self) -> Result<(), NokhwaError> {
        Ok(())
    }

    /// Convenience: `lock_ui` → `trigger` → `take_picture` → `unlock_ui`, with
    /// `unlock_ui` always attempted (errors from `unlock_ui` are discarded if
    /// the inner sequence failed).
    /// # Errors
    /// Returns [`NokhwaError`] if any step of the sequence fails.
    fn capture(&mut self, timeout: Duration) -> Result<Buffer, NokhwaError> {
        self.lock_ui()?;
        self.trigger()?;
        let result = self.take_picture(timeout);
        let _ = self.unlock_ui();
        result
    }
}

/// Optional per-camera event-stream capability. See [`HotplugSource`] for
/// the backend-wide analog that reports device arrival / removal before
/// any camera has been opened.
pub trait EventSource: CameraDevice {
    /// Take the event poller. Succeeds at most once. Subsequent calls must
    /// return `Err(NokhwaError::UnsupportedOperationError(...))`.
    /// # Errors
    /// Returns [`NokhwaError::UnsupportedOperationError`] if the poller has
    /// already been taken or the backend does not support events.
    fn take_events(&mut self) -> Result<Box<dyn EventPoll + Send>, NokhwaError>;
}

/// Sync event-polling interface. Backends wrap their internal channel in a
/// type that implements this trait.
pub trait EventPoll: Send {
    fn try_next(&mut self) -> Option<CameraEvent>;
    fn next_timeout(&mut self, d: Duration) -> Option<CameraEvent>;
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum CameraEvent {
    Disconnected,
    CaptureError { code: i32, message: String },
    WillShutDown,
}

/// Optional backend-level hotplug capability.
///
/// Distinct from [`EventSource`], which is scoped to a single already-opened
/// camera. `HotplugSource` is implemented by a backend-wide context or
/// registry type and reports plug / unplug signals for the backend as a
/// whole — including devices that appear **before** any camera has been
/// opened. Typical implementors wrap Canon EDSDK's
/// `EdsSetCameraAddedHandler`, Linux `inotify` on `/dev/video*`, macOS `IOKit`
/// matching notifications, or Windows MSMF device-change notifications.
///
/// `HotplugSource` is intentionally **not** a supertrait of [`CameraDevice`]:
/// hotplug is a backend-registry concern, not a per-camera concern. It is
/// implemented on a backend-wide context type, not on individual cameras.
///
/// This trait is **optional**. Backends that cannot detect hotplug (for
/// example `OpenCV`, which wraps platform APIs without surfacing
/// device-change callbacks) simply do not implement it.
///
/// [`HotplugEvent::Connected`] carries the [`CameraInfo`] of the newly
/// visible device, so consumers can immediately open it.
/// [`HotplugEvent::Disconnected`] carries the [`CameraInfo`] of the device
/// that disappeared so consumers can match against their currently-open
/// camera instances and tear them down. Backends **must** guarantee that
/// the [`CameraInfo::index()`] of a `Disconnected` event matches the `index`
/// previously delivered in the corresponding `Connected` event (or the
/// `index` seen during initial enumeration); the human-readable
/// `human_name` / `description` / `misc` fields are best-effort and may
/// drift between arrival and removal, so consumers should match on
/// [`CameraInfo::index()`] rather than structural equality.
///
/// Re-plugging the same physical device may produce a *new* `index` on
/// the second `Connected` event; backends are not required to recognize
/// the device as the same across a disconnect / reconnect cycle.
///
/// Ordering and delivery guarantees (coalescing, duplicate suppression,
/// backpressure, per-bus vs. global scope) are backend-specific; consult
/// each implementor's documentation.
pub trait HotplugSource {
    /// Take the hotplug-event poller. Succeeds at most once per backend
    /// instance. Subsequent calls must return
    /// `Err(NokhwaError::UnsupportedOperationError(...))`, mirroring
    /// [`EventSource::take_events`].
    ///
    /// The returned poller is the only handle through which hotplug
    /// events can be observed; dropping it silently unsubscribes the
    /// caller, hence the `#[must_use]` hint.
    /// # Errors
    /// Returns [`NokhwaError::UnsupportedOperationError`] if the poller has
    /// already been taken or the backend does not support hotplug events.
    #[must_use = "the hotplug poller is the only way to observe hotplug events; dropping it discards the subscription"]
    fn take_hotplug_events(&mut self) -> Result<Box<dyn HotplugEventPoll + Send>, NokhwaError>;
}

/// Sync hotplug-event polling interface. Mirrors [`EventPoll`].
///
/// The `Send` bound mirrors [`EventPoll`] so that pollers can be handed
/// off to a dedicated supervisor thread. `Sync` is intentionally not
/// required: most backends protect their internal event queue with
/// interior mutability that only guarantees `Send`, and applications that
/// need shared access can wrap the poller in `Arc<Mutex<_>>`.
pub trait HotplugEventPoll: Send {
    /// Non-blocking poll. Returns the next buffered [`HotplugEvent`], or
    /// `None` when no event is currently buffered. `None` does **not**
    /// mean the source is closed — callers should keep polling.
    fn try_next(&mut self) -> Option<HotplugEvent>;
    /// Block for up to `d` waiting for the next [`HotplugEvent`]. Returns
    /// `None` when the wait times out without an event arriving; this
    /// does **not** indicate that the source is closed.
    fn next_timeout(&mut self, d: Duration) -> Option<HotplugEvent>;
}

/// Backend-level plug / unplug signal produced by a [`HotplugSource`].
///
/// Distinct from [`CameraEvent::Disconnected`], which is scoped to a camera
/// the application has already opened. `HotplugEvent` covers devices the
/// application has not yet opened — in particular, arrivals.
///
/// `PartialEq` / `Eq` / `Hash` are derived so consumers can dedupe events
/// (useful when a backend does not suppress duplicates) or use them as
/// hashmap keys. Note that structural equality compares the full
/// [`CameraInfo`]; to match a `Disconnected` event against a previously
/// seen `Connected`, prefer comparing [`CameraInfo::index()`] — see the
/// [`HotplugSource`] docs for the ordering guarantee.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HotplugEvent {
    /// A camera became visible to the backend. The [`CameraInfo`] is
    /// sufficient to open the device.
    Connected(CameraInfo),
    /// A camera was removed from the backend. Consumers should match by
    /// [`CameraInfo::index()`] against any currently-open camera instances
    /// they hold; other fields may have drifted since arrival.
    Disconnected(CameraInfo),
}
