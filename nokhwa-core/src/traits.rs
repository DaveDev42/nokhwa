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
    fn controls(&self) -> Result<Vec<CameraControl>, NokhwaError>;
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
    fn set_format(&mut self, f: CameraFormat) -> Result<(), NokhwaError>;
    fn compatible_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError>;
    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError>;

    fn open(&mut self) -> Result<(), NokhwaError>;
    fn is_open(&self) -> bool;
    fn frame(&mut self) -> Result<Buffer, NokhwaError>;
    fn frame_timeout(&mut self, _duration: Duration) -> Result<Buffer, NokhwaError> {
        self.frame()
    }
    fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError>;
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
    fn trigger(&mut self) -> Result<(), NokhwaError>;
    fn take_picture(&mut self, timeout: Duration) -> Result<Buffer, NokhwaError>;

    /// UI/physical-control lock. No-op default — webcams do not need this.
    fn lock(&mut self) -> Result<(), NokhwaError> {
        Ok(())
    }
    fn unlock(&mut self) -> Result<(), NokhwaError> {
        Ok(())
    }

    /// Convenience: lock → trigger → take_picture → unlock, with unlock always
    /// attempted (errors from `unlock` are discarded if the inner sequence failed).
    fn capture(&mut self, timeout: Duration) -> Result<Buffer, NokhwaError> {
        self.lock()?;
        self.trigger()?;
        let result = self.take_picture(timeout);
        let _ = self.unlock();
        result
    }
}

/// Optional event-stream capability.
pub trait EventSource: CameraDevice {
    /// Take the event poller. Succeeds at most once. Subsequent calls must
    /// return `Err(NokhwaError::UnsupportedOperationError(...))`.
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
