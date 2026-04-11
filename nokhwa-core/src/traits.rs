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
        KnownCameraControl, Resolution,
    },
};
use std::{borrow::Cow, collections::HashMap, time::Duration};
#[cfg(feature = "wgpu-types")]
use wgpu::{
    Device as WgpuDevice, Extent3d, Queue as WgpuQueue, TexelCopyBufferLayout,
    Texture as WgpuTexture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages,
};

/// A GPU texture containing raw camera frame data in its native pixel format,
/// without conversion to RGBA. Consumers can use GPU shaders to decode the
/// data according to the [`FrameFormat`] and [`TextureFormat`] provided.
#[derive(Debug)]
#[cfg(feature = "wgpu-types")]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "wgpu-types")))]
pub struct RawTextureData {
    /// The wgpu texture containing the raw frame data.
    pub texture: WgpuTexture,
    /// The camera's native pixel format (e.g. NV12, YUYV).
    pub source_frame_format: FrameFormat,
    /// The wgpu texture format used to store the data.
    pub texture_format: TextureFormat,
    /// The original frame resolution.
    pub resolution: Resolution,
}

/// Returns the wgpu [`TextureFormat`] and texture dimensions suitable for
/// storing raw frame data of the given [`FrameFormat`] and [`Resolution`].
///
/// For formats without a direct wgpu equivalent, the data is packed into a
/// single-channel or two-channel texture that a shader can decode.
///
/// # Errors
/// Returns `Err` for [`FrameFormat::MJPEG`] because compressed MJPEG data
/// cannot be uploaded as a raw texture — use
/// [`CaptureBackendTrait::frame_texture()`] instead.
#[cfg(feature = "wgpu-types")]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "wgpu-types")))]
pub fn raw_texture_layout(
    format: FrameFormat,
    resolution: Resolution,
) -> Result<(TextureFormat, Extent3d, u32), NokhwaError> {
    let w = resolution.width();
    let h = resolution.height();
    match format {
        // YUYV: 2 bytes per pixel, packed as [Y0 U0 Y1 V0]. Store as Rg8Unorm
        // so each texel holds one byte-pair. Width is the full pixel width
        // (each texel = 1 pixel channel), but total bytes per row = 2*w.
        FrameFormat::YUYV => {
            if !w.is_multiple_of(2) {
                return Err(NokhwaError::ProcessFrameError {
                    src: format,
                    destination: "RawTextureData".to_string(),
                    error: format!("YUYV requires even width, got {w}"),
                });
            }
            Ok((
                TextureFormat::Rg8Unorm,
                Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                2 * w,
            ))
        }
        // NV12: Y plane (w*h bytes) + interleaved UV plane (w*h/2 bytes).
        // Total = 1.5 * w * h bytes. Store as R8Unorm with height * 3/2.
        FrameFormat::NV12 => {
            if !w.is_multiple_of(2) || !h.is_multiple_of(2) {
                return Err(NokhwaError::ProcessFrameError {
                    src: format,
                    destination: "RawTextureData".to_string(),
                    error: format!("NV12 requires even dimensions, got {w}x{h}"),
                });
            }
            Ok((
                TextureFormat::R8Unorm,
                Extent3d {
                    width: w,
                    height: h * 3 / 2,
                    depth_or_array_layers: 1,
                },
                w,
            ))
        }
        // GRAY: 1 byte per pixel. Directly maps to R8Unorm.
        FrameFormat::GRAY => Ok((
            TextureFormat::R8Unorm,
            Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            w,
        )),
        // RAWRGB / RAWBGR: 3 bytes per pixel. No exact 3-channel unorm in
        // wgpu, so use R8Unorm with width*3 to avoid any padding requirement.
        FrameFormat::RAWRGB | FrameFormat::RAWBGR => Ok((
            TextureFormat::R8Unorm,
            Extent3d {
                width: w * 3,
                height: h,
                depth_or_array_layers: 1,
            },
            w * 3,
        )),
        // MJPEG is a compressed format — raw bytes cannot be uploaded directly.
        FrameFormat::MJPEG => Err(NokhwaError::general(
            "frame_texture_raw() cannot be used with MJPEG sources; \
             use frame_texture() instead",
        )),
    }
}

/// The core trait that every camera backend implements.
///
/// `CaptureBackendTrait` defines the full lifecycle of a camera: opening, configuring,
/// streaming frames, and closing. The high-level [`Camera`] struct wraps a
/// `Box<dyn CaptureBackendTrait>` and delegates to it, so most users interact with this
/// trait indirectly.
///
/// Many backends are **blocking** — if the camera device is occupied the call will
/// block until it becomes available.
///
/// # Typical lifecycle
///
/// 1. **Open the stream** — call [`open_stream()`](CaptureBackendTrait::open_stream)
/// 2. **Capture frames** — call [`frame()`](CaptureBackendTrait::frame) for decoded
///    frames, or [`frame_raw()`](CaptureBackendTrait::frame_raw) for unprocessed bytes
/// 3. **Query/set controls** — use [`camera_controls()`](CaptureBackendTrait::camera_controls)
///    and [`set_camera_control()`](CaptureBackendTrait::set_camera_control)
/// 4. **Stop** — call [`stop_stream()`](CaptureBackendTrait::stop_stream) (also runs on drop)
///
/// # Notes
///
/// - Backends default to 640×480 @ 15 FPS, MJPEG if no format is specified.
/// - Behaviour can differ between backends. If you use the raw backend structs
///   directly, read the **Quirks** section in each backend's documentation.
/// - After calling [`stop_stream()`](CaptureBackendTrait::stop_stream), you must call
///   [`open_stream()`](CaptureBackendTrait::open_stream) again to resume capture.
pub trait CaptureBackendTrait {
    /// Returns the current backend used.
    fn backend(&self) -> ApiBackend;

    /// Gets the camera information such as Name and Index as a [`CameraInfo`].
    fn camera_info(&self) -> &CameraInfo;

    /// Forcefully refreshes the stored camera format, bringing it into sync with "reality" (current camera state)
    /// # Errors
    /// If the camera can not get its most recent [`CameraFormat`]. this will error.
    fn refresh_camera_format(&mut self) -> Result<(), NokhwaError>;

    /// Gets the current [`CameraFormat`]. This will force refresh to the current latest if it has changed.
    fn camera_format(&self) -> CameraFormat;

    /// Will set the current [`CameraFormat`]
    /// This will reset the current stream if used while stream is opened.
    ///
    /// This will also update the cache.
    /// # Errors
    /// If you started the stream and the camera rejects the new camera format, this will return an error.
    fn set_camera_format(&mut self, new_fmt: CameraFormat) -> Result<(), NokhwaError>;

    /// A hashmap of [`Resolution`]s mapped to framerates. Not sorted!
    /// # Errors
    /// This will error if the camera is not queryable or a query operation has failed. Some backends will error this out as a Unsupported Operation ([`UnsupportedOperationError`](crate::error::NokhwaError::UnsupportedOperationError)).
    fn compatible_list_by_resolution(
        &mut self,
        fourcc: FrameFormat,
    ) -> Result<HashMap<Resolution, Vec<u32>>, NokhwaError>;

    /// Gets the compatible [`CameraFormat`] of the camera
    /// # Errors
    /// If it fails to get, this will error.
    fn compatible_camera_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
        let mut compatible_formats = vec![];
        for fourcc in self.compatible_fourcc()? {
            for (resolution, fps_list) in self.compatible_list_by_resolution(fourcc)? {
                for fps in fps_list {
                    compatible_formats.push(CameraFormat::new(resolution, fourcc, fps));
                }
            }
        }

        Ok(compatible_formats)
    }

    /// A Vector of compatible [`FrameFormat`]s. Will only return 2 elements at most.
    /// # Errors
    /// This will error if the camera is not queryable or a query operation has failed. Some backends will error this out as a Unsupported Operation ([`UnsupportedOperationError`](crate::error::NokhwaError::UnsupportedOperationError)).
    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError>;

    /// Gets the current camera resolution (See: [`Resolution`], [`CameraFormat`]). This will force refresh to the current latest if it has changed.
    fn resolution(&self) -> Resolution;

    /// Will set the current [`Resolution`]
    /// This will reset the current stream if used while stream is opened.
    ///
    /// This will also update the cache.
    /// # Errors
    /// If you started the stream and the camera rejects the new resolution, this will return an error.
    fn set_resolution(&mut self, new_res: Resolution) -> Result<(), NokhwaError>;

    /// Gets the current camera framerate (See: [`CameraFormat`]). This will force refresh to the current latest if it has changed.
    fn frame_rate(&self) -> u32;

    /// Will set the current framerate
    /// This will reset the current stream if used while stream is opened.
    ///
    /// This will also update the cache.
    /// # Errors
    /// If you started the stream and the camera rejects the new framerate, this will return an error.
    fn set_frame_rate(&mut self, new_fps: u32) -> Result<(), NokhwaError>;

    /// Gets the current camera's frame format (See: [`FrameFormat`], [`CameraFormat`]). This will force refresh to the current latest if it has changed.
    fn frame_format(&self) -> FrameFormat;

    /// Will set the current [`FrameFormat`]
    /// This will reset the current stream if used while stream is opened.
    ///
    /// This will also update the cache.
    /// # Errors
    /// If you started the stream and the camera rejects the new frame format, this will return an error.
    fn set_frame_format(&mut self, fourcc: FrameFormat) -> Result<(), NokhwaError>;

    /// Gets the value of [`KnownCameraControl`].
    /// # Errors
    /// If the `control` is not supported or there is an error while getting the camera control values (e.g. unexpected value, too high, etc)
    /// this will error.
    fn camera_control(&self, control: KnownCameraControl) -> Result<CameraControl, NokhwaError>;

    /// Returns all supported [`CameraControl`]s for this camera.
    ///
    /// Each [`CameraControl`] describes a property (brightness, contrast, etc.) with its
    /// current value, valid range, and step size.
    ///
    /// ```ignore
    /// # fn example(camera: &nokhwa::Camera) -> Result<(), nokhwa_core::error::NokhwaError> {
    /// for control in camera.camera_controls()? {
    ///     println!("{}: {:?}", control.control(), control.value());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    /// If the list cannot be collected, this will error. This can be treated as "nothing supported".
    fn camera_controls(&self) -> Result<Vec<CameraControl>, NokhwaError>;

    /// Sets the control to `control` in the camera.
    /// Usually, the pipeline is calling [`camera_control()`](CaptureBackendTrait::camera_control), getting a camera control that way
    /// then calling [`value()`](CameraControl::value()) to get a [`ControlValueSetter`] and setting the value that way.
    /// # Errors
    /// If the `control` is not supported, the value is invalid (less than min, greater than max, not in step), or there was an error setting the control,
    /// this will error.
    fn set_camera_control(
        &mut self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError>;

    /// Opens the camera stream with the currently set parameters.
    ///
    /// You must call this before calling [`frame()`](CaptureBackendTrait::frame) or
    /// [`frame_raw()`](CaptureBackendTrait::frame_raw).
    ///
    /// ```ignore
    /// # fn example(camera: &mut nokhwa::Camera) -> Result<(), nokhwa_core::error::NokhwaError> {
    /// camera.open_stream()?;
    /// assert!(camera.is_stream_open());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    /// If the specific backend fails to open the camera (e.g. already taken, busy, doesn't exist anymore) this will error.
    fn open_stream(&mut self) -> Result<(), NokhwaError>;

    /// Checks if stream if open. If it is, it will return true.
    fn is_stream_open(&self) -> bool;

    /// Captures a frame from the camera as a [`Buffer`].
    ///
    /// The returned [`Buffer`] contains raw frame data along with format metadata.
    /// Wrap it in a [`Frame<F>`](crate::frame::Frame) for type-safe conversions.
    ///
    /// # Errors
    /// If the backend fails to get the frame (e.g. already taken, busy, doesn't exist anymore),
    /// the decoding fails (e.g. MJPEG → RGB), or [`open_stream()`](CaptureBackendTrait::open_stream())
    /// has not been called yet, this will error.
    fn frame(&mut self) -> Result<Buffer, NokhwaError>;

    /// Will get a frame from the camera as a [`Buffer`], but with a timeout. If the frame is not
    /// received within the given `duration`, this will return a [`TimeoutError`](crate::error::NokhwaError::TimeoutError).
    ///
    /// The default implementation simply delegates to [`frame()`](CaptureBackendTrait::frame())
    /// without enforcing the timeout. The [`Camera`] wrapper provides a threaded timeout
    /// mechanism. Backends should override this for more efficient platform-specific timeout
    /// support.
    ///
    /// # Errors
    /// If the backend fails to get the frame within the timeout, or if the underlying
    /// [`frame()`](CaptureBackendTrait::frame()) call fails, this will error.
    fn frame_timeout(&mut self, _duration: Duration) -> Result<Buffer, NokhwaError> {
        // NOTE: timeout is intentionally ignored in the default impl; the Camera
        // wrapper provides threaded timeout enforcement.
        self.frame()
    }

    /// Captures a frame **without** any decoding or processing.
    ///
    /// The returned bytes are in the camera's native pixel format (e.g. MJPEG, YUYV, NV12).
    /// Use this when you want to handle decoding yourself or forward the raw data elsewhere.
    ///
    /// ```ignore
    /// # fn example(camera: &mut nokhwa::Camera) -> Result<(), nokhwa_core::error::NokhwaError> {
    /// camera.open_stream()?;
    /// let raw_bytes = camera.frame_raw()?;
    /// println!("raw frame: {} bytes", raw_bytes.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    /// If the backend fails to get the frame (e.g. already taken, busy, doesn't exist anymore),
    /// or [`open_stream()`](CaptureBackendTrait::open_stream()) has not been called yet, this will error.
    fn frame_raw(&mut self) -> Result<Cow<'_, [u8]>, NokhwaError>;

    /// The minimum buffer size needed to write the current frame. If `alpha` is true, it will instead return the minimum size of the buffer with an alpha channel as well.
    /// This assumes that you are decoding to RGB/RGBA for [`FrameFormat::MJPEG`] or [`FrameFormat::YUYV`] and Luma8/LumaA8 for [`FrameFormat::GRAY`]
    #[must_use]
    fn decoded_buffer_size(&self, alpha: bool) -> usize {
        let cfmt = self.camera_format();
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
    /// Directly copies a frame to a Wgpu texture as RGBA. Uses the new
    /// Frame conversion API internally.
    /// # Errors
    /// If the frame cannot be captured or the resolution is 0 on any axis, this will error.
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

        // Convert to RGBA using the internal conversion path
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
    /// Copies a frame to a Wgpu texture in the camera's **native pixel format**
    /// (e.g. NV12, YUYV) without converting to RGBA. The returned
    /// [`RawTextureData`] carries the format metadata so consumers can apply
    /// the appropriate GPU shader for decoding.
    ///
    /// The default implementation calls [`frame_raw()`](CaptureBackendTrait::frame_raw)
    /// and uploads the bytes into a texture whose layout is determined by
    /// [`raw_texture_layout()`]. Backends may override this to provide a more
    /// efficient implementation (e.g. zero-copy).
    ///
    /// # Errors
    /// If the frame cannot be captured or the resolution is 0 on any axis, this will error.
    fn frame_texture_raw(
        &mut self,
        device: &WgpuDevice,
        queue: &WgpuQueue,
        label: Option<&str>,
    ) -> Result<RawTextureData, NokhwaError> {
        use wgpu::{Origin3d, TexelCopyTextureInfoBase};

        let source_format = self.frame_format();
        let resolution = self.resolution();
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

    /// Will drop the stream.
    /// # Errors
    /// Please check the `Quirks` section of each backend.
    fn stop_stream(&mut self) -> Result<(), NokhwaError>;
}

impl<T> From<T> for Box<dyn CaptureBackendTrait>
where
    T: CaptureBackendTrait + 'static,
{
    fn from(capbackend: T) -> Self {
        Box::new(capbackend)
    }
}
