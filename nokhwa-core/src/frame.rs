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

//! Type-safe frame handles and lazy conversion traits.
//!
//! A [`Frame<F>`] is a thin wrapper around [`Buffer`] that carries a compile-time
//! [`CaptureFormat`] tag. Conversion traits ([`IntoRgb`], [`IntoRgba`], [`IntoLuma`])
//! are selectively implemented per format so that invalid conversions (e.g. grayscale
//! to RGB) are caught at compile time.

use crate::buffer::Buffer;
use crate::error::NokhwaError;
use crate::format_types::{CaptureFormat, Gray, Mjpeg, Nv12, RawBgr, RawRgb, Yuyv};
use crate::types::{
    buf_bgr_to_rgb, buf_mjpeg_to_rgb, buf_nv12_extract_luma, buf_nv12_to_rgb, buf_yuyv422_to_rgb,
    buf_yuyv_extract_luma, mjpeg_to_rgb, nv12_to_rgb, yuyv422_to_rgb, FrameFormat, Resolution,
};
use image::{ImageBuffer, Luma, Rgb, Rgba};
use std::io::{Seek, Write};
use std::marker::PhantomData;
use std::time::Duration;

use crate::buffer::TimestampKind;

/// A typed frame handle carrying raw camera data tagged with its [`CaptureFormat`].
///
/// `Frame<F>` is cheap to clone (the underlying [`Buffer`] uses `bytes::Bytes`
/// reference counting). Conversion methods like [`into_rgb()`](IntoRgb::into_rgb)
/// are infallible and return lazy conversion structs; actual pixel processing
/// happens when you call [`materialize()`](RgbConversion::materialize) or
/// [`write_to()`](RgbConversion::write_to).
#[derive(Clone, Debug)]
pub struct Frame<F: CaptureFormat> {
    buffer: Buffer,
    _format: PhantomData<F>,
}

impl<F: CaptureFormat> Frame<F> {
    /// Creates a new `Frame` from a [`Buffer`].
    ///
    /// # Panics
    /// Panics if the buffer's [`FrameFormat`] does not match `F::FRAME_FORMAT`.
    /// Use [`try_new`](Self::try_new) for a fallible version.
    #[must_use]
    pub fn new(buffer: Buffer) -> Self {
        assert_eq!(
            buffer.source_frame_format(),
            F::FRAME_FORMAT,
            "Buffer FrameFormat {:?} does not match expected {:?}",
            buffer.source_frame_format(),
            F::FRAME_FORMAT,
        );
        Self {
            buffer,
            _format: PhantomData,
        }
    }

    /// Fallible version of [`new`](Self::new). Returns an error if the buffer's
    /// [`FrameFormat`] does not match `F::FRAME_FORMAT`.
    /// # Errors
    /// Returns [`NokhwaError::ProcessFrameError`] on format mismatch.
    pub fn try_new(buffer: Buffer) -> Result<Self, NokhwaError> {
        if buffer.source_frame_format() != F::FRAME_FORMAT {
            return Err(NokhwaError::ProcessFrameError {
                src: buffer.source_frame_format(),
                destination: format!("Frame<{:?}>", F::FRAME_FORMAT),
                error: format!(
                    "expected {:?}, got {:?}",
                    F::FRAME_FORMAT,
                    buffer.source_frame_format()
                ),
            });
        }
        Ok(Self {
            buffer,
            _format: PhantomData,
        })
    }

    /// Returns the frame resolution.
    #[must_use]
    pub fn resolution(&self) -> Resolution {
        self.buffer.resolution()
    }

    /// Returns the raw frame data as a byte slice.
    #[must_use]
    pub fn buffer(&self) -> &[u8] {
        self.buffer.buffer()
    }

    /// Returns a reference to the underlying [`Buffer`].
    #[must_use]
    pub fn as_buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Consumes the frame and returns the underlying [`Buffer`].
    #[must_use]
    pub fn into_buffer(self) -> Buffer {
        self.buffer
    }

    /// Returns the capture timestamp, if available.
    #[must_use]
    pub fn capture_timestamp(&self) -> Option<Duration> {
        self.buffer.capture_timestamp()
    }

    /// Returns the capture timestamp with its [`TimestampKind`], if available.
    #[must_use]
    pub fn capture_timestamp_with_kind(&self) -> Option<(Duration, TimestampKind)> {
        self.buffer.capture_timestamp_with_kind()
    }
}

// ---------------------------------------------------------------------------
// Conversion traits
// ---------------------------------------------------------------------------

/// Convert a frame to RGB888.
///
/// Implemented for all color formats (YUYV, NV12, MJPEG, RAWRGB, RAWBGR).
/// **Not** implemented for [`Gray`] — attempting `Frame<Gray>.into_rgb()` is a
/// compile error.
///
/// ```compile_fail
/// use nokhwa_core::format_types::Gray;
/// use nokhwa_core::frame::{Frame, IntoRgb};
/// fn gray_to_rgb(f: Frame<Gray>) {
///     let _ = f.into_rgb(); // ERROR: Gray does not implement IntoRgb
/// }
/// ```
pub trait IntoRgb {
    /// Returns a lazy RGB conversion handle. No pixel processing happens here.
    fn into_rgb(self) -> RgbConversion;
}

/// Convert a frame to RGBA8888.
///
/// Implemented for all color formats (YUYV, NV12, MJPEG, RAWRGB, RAWBGR).
/// **Not** implemented for [`Gray`] — attempting `Frame<Gray>.into_rgba()` is a
/// compile error.
///
/// ```compile_fail
/// use nokhwa_core::format_types::Gray;
/// use nokhwa_core::frame::{Frame, IntoRgba};
/// fn gray_to_rgba(f: Frame<Gray>) {
///     let _ = f.into_rgba(); // ERROR: Gray does not implement IntoRgba
/// }
/// ```
pub trait IntoRgba {
    /// Returns a lazy RGBA conversion handle. No pixel processing happens here.
    fn into_rgba(self) -> RgbaConversion;
}

/// Convert a frame to 8-bit grayscale (Luma).
///
/// Implemented for all formats. For YUYV and NV12, uses direct Y-channel
/// extraction rather than an intermediate RGB conversion.
pub trait IntoLuma {
    /// Returns a lazy luma conversion handle. No pixel processing happens here.
    fn into_luma(self) -> LumaConversion;
}

// ---------------------------------------------------------------------------
// Conversion structs (lazy)
// ---------------------------------------------------------------------------

/// Lazy RGB conversion. Call [`materialize()`](Self::materialize) to perform
/// the actual pixel conversion.
#[must_use = "conversion is lazy; call .materialize() or .write_to() to perform it"]
pub struct RgbConversion {
    buffer: Buffer,
}

impl RgbConversion {
    /// Performs the conversion and returns an `ImageBuffer<Rgb<u8>, Vec<u8>>`.
    /// # Errors
    /// Returns an error if the pixel data is malformed or decoding fails.
    pub fn materialize(self) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>, NokhwaError> {
        let resolution = self.buffer.resolution();
        let data = self.buffer.buffer();
        let fcc = self.buffer.source_frame_format();

        let rgb_data = convert_to_rgb(fcc, resolution, data)?;

        ImageBuffer::from_raw(resolution.width_x, resolution.height_y, rgb_data).ok_or(
            NokhwaError::ProcessFrameError {
                src: fcc,
                destination: "Rgb".to_string(),
                error: "Failed to create ImageBuffer".to_string(),
            },
        )
    }

    /// Writes the converted RGB data into the provided buffer.
    /// # Errors
    /// Returns an error if the pixel data is malformed, decoding fails, or the
    /// destination buffer is too small.
    pub fn write_to(self, dest: &mut [u8]) -> Result<(), NokhwaError> {
        let resolution = self.buffer.resolution();
        let data = self.buffer.buffer();
        let fcc = self.buffer.source_frame_format();

        convert_to_rgb_buffer(fcc, resolution, data, dest)
    }

    /// Writes the converted RGB image as PNG to the given writer.
    /// # Errors
    /// Returns an error if pixel conversion or PNG encoding fails.
    pub fn write_png<W: Write + Seek>(self, writer: W) -> Result<(), NokhwaError> {
        let fcc = self.buffer.source_frame_format();
        let img = self.materialize()?;
        let dyn_image = image::DynamicImage::ImageRgb8(img);
        dyn_image
            .write_to(
                &mut std::io::BufWriter::new(writer),
                image::ImageFormat::Png,
            )
            .map_err(|e| NokhwaError::ProcessFrameError {
                src: fcc,
                destination: "PNG".to_string(),
                error: e.to_string(),
            })
    }
}

/// Lazy RGBA conversion. Call [`materialize()`](Self::materialize) to perform
/// the actual pixel conversion.
#[must_use = "conversion is lazy; call .materialize() or .write_to() to perform it"]
pub struct RgbaConversion {
    buffer: Buffer,
}

impl RgbaConversion {
    /// Performs the conversion and returns an `ImageBuffer<Rgba<u8>, Vec<u8>>`.
    /// # Errors
    /// Returns an error if the pixel data is malformed or decoding fails.
    pub fn materialize(self) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, NokhwaError> {
        let resolution = self.buffer.resolution();
        let data = self.buffer.buffer();
        let fcc = self.buffer.source_frame_format();

        let rgba_data = convert_to_rgba(fcc, resolution, data)?;

        ImageBuffer::from_raw(resolution.width_x, resolution.height_y, rgba_data).ok_or(
            NokhwaError::ProcessFrameError {
                src: fcc,
                destination: "Rgba".to_string(),
                error: "Failed to create ImageBuffer".to_string(),
            },
        )
    }

    /// Writes the converted RGBA data into the provided buffer.
    /// # Errors
    /// Returns an error if the pixel data is malformed, decoding fails, or the
    /// destination buffer is too small.
    pub fn write_to(self, dest: &mut [u8]) -> Result<(), NokhwaError> {
        let resolution = self.buffer.resolution();
        let data = self.buffer.buffer();
        let fcc = self.buffer.source_frame_format();

        convert_to_rgba_buffer(fcc, resolution, data, dest)
    }
}

/// Lazy luma (grayscale) conversion. Call [`materialize()`](Self::materialize)
/// to perform the actual pixel conversion.
#[must_use = "conversion is lazy; call .materialize() or .write_to() to perform it"]
pub struct LumaConversion {
    buffer: Buffer,
}

impl LumaConversion {
    /// Performs the conversion and returns an `ImageBuffer<Luma<u8>, Vec<u8>>`.
    /// # Errors
    /// Returns an error if the pixel data is malformed or decoding fails.
    pub fn materialize(self) -> Result<ImageBuffer<Luma<u8>, Vec<u8>>, NokhwaError> {
        let resolution = self.buffer.resolution();
        let data = self.buffer.buffer();
        let fcc = self.buffer.source_frame_format();

        let luma_data = convert_to_luma(fcc, resolution, data)?;

        ImageBuffer::from_raw(resolution.width_x, resolution.height_y, luma_data).ok_or(
            NokhwaError::ProcessFrameError {
                src: fcc,
                destination: "Luma".to_string(),
                error: "Failed to create ImageBuffer".to_string(),
            },
        )
    }

    /// Writes the converted luma data into the provided buffer.
    /// # Errors
    /// Returns an error if the pixel data is malformed, decoding fails, or the
    /// destination buffer is too small.
    pub fn write_to(self, dest: &mut [u8]) -> Result<(), NokhwaError> {
        let resolution = self.buffer.resolution();
        let data = self.buffer.buffer();
        let fcc = self.buffer.source_frame_format();

        convert_to_luma_buffer(fcc, resolution, data, dest)
    }
}

// ---------------------------------------------------------------------------
// IntoRgb impls — all color formats (NOT Gray)
// ---------------------------------------------------------------------------

macro_rules! impl_into_rgb {
    ($($ty:ty),+) => {
        $(
            impl IntoRgb for Frame<$ty> {
                fn into_rgb(self) -> RgbConversion {
                    RgbConversion { buffer: self.buffer }
                }
            }
        )+
    };
}

impl_into_rgb!(Yuyv, Nv12, Mjpeg, RawRgb, RawBgr);

// ---------------------------------------------------------------------------
// IntoRgba impls — all color formats (NOT Gray)
// ---------------------------------------------------------------------------

macro_rules! impl_into_rgba {
    ($($ty:ty),+) => {
        $(
            impl IntoRgba for Frame<$ty> {
                fn into_rgba(self) -> RgbaConversion {
                    RgbaConversion { buffer: self.buffer }
                }
            }
        )+
    };
}

impl_into_rgba!(Yuyv, Nv12, Mjpeg, RawRgb, RawBgr);

// ---------------------------------------------------------------------------
// IntoLuma impls — ALL formats
// ---------------------------------------------------------------------------

macro_rules! impl_into_luma {
    ($($ty:ty),+) => {
        $(
            impl IntoLuma for Frame<$ty> {
                fn into_luma(self) -> LumaConversion {
                    LumaConversion { buffer: self.buffer }
                }
            }
        )+
    };
}

impl_into_luma!(Yuyv, Nv12, Mjpeg, Gray, RawRgb, RawBgr);

// ---------------------------------------------------------------------------
// Internal conversion dispatchers
// ---------------------------------------------------------------------------

pub(crate) fn convert_to_rgb(
    fcc: FrameFormat,
    resolution: Resolution,
    data: &[u8],
) -> Result<Vec<u8>, NokhwaError> {
    match fcc {
        FrameFormat::MJPEG => mjpeg_to_rgb(data, false),
        FrameFormat::YUYV => yuyv422_to_rgb(data, false),
        FrameFormat::NV12 => nv12_to_rgb(resolution, data, false),
        FrameFormat::RAWRGB => Ok(data.to_vec()),
        FrameFormat::RAWBGR => {
            let mut rgb = vec![0u8; data.len()];
            data.chunks_exact(3).enumerate().for_each(|(idx, px)| {
                let i = idx * 3;
                rgb[i] = px[2];
                rgb[i + 1] = px[1];
                rgb[i + 2] = px[0];
            });
            Ok(rgb)
        }
        FrameFormat::GRAY => Ok(data.iter().flat_map(|&x| [x, x, x]).collect()),
    }
}

pub(crate) fn convert_to_rgb_buffer(
    fcc: FrameFormat,
    resolution: Resolution,
    data: &[u8],
    dest: &mut [u8],
) -> Result<(), NokhwaError> {
    match fcc {
        FrameFormat::MJPEG => buf_mjpeg_to_rgb(data, dest, false),
        FrameFormat::YUYV => buf_yuyv422_to_rgb(data, dest, false),
        FrameFormat::NV12 => buf_nv12_to_rgb(resolution, data, dest, false),
        FrameFormat::RAWRGB => {
            if dest.len() != data.len() {
                return Err(NokhwaError::ProcessFrameError {
                    src: fcc,
                    destination: "RGB".to_string(),
                    error: format!(
                        "destination buffer size mismatch (expected {}, got {})",
                        data.len(),
                        dest.len()
                    ),
                });
            }
            dest.copy_from_slice(data);
            Ok(())
        }
        FrameFormat::RAWBGR => buf_bgr_to_rgb(resolution, data, dest),
        FrameFormat::GRAY => {
            if dest.len() != data.len() * 3 {
                return Err(NokhwaError::ProcessFrameError {
                    src: fcc,
                    destination: "RGB".to_string(),
                    error: "Bad buffer length".to_string(),
                });
            }
            data.iter().enumerate().for_each(|(idx, &pxv)| {
                let i = idx * 3;
                dest[i] = pxv;
                dest[i + 1] = pxv;
                dest[i + 2] = pxv;
            });
            Ok(())
        }
    }
}

pub(crate) fn convert_to_rgba(
    fcc: FrameFormat,
    resolution: Resolution,
    data: &[u8],
) -> Result<Vec<u8>, NokhwaError> {
    match fcc {
        FrameFormat::MJPEG => mjpeg_to_rgb(data, true),
        FrameFormat::YUYV => yuyv422_to_rgb(data, true),
        FrameFormat::NV12 => nv12_to_rgb(resolution, data, true),
        FrameFormat::RAWRGB => Ok(data
            .chunks_exact(3)
            .flat_map(|x| [x[0], x[1], x[2], 255])
            .collect()),
        FrameFormat::RAWBGR => Ok(data
            .chunks_exact(3)
            .flat_map(|x| [x[2], x[1], x[0], 255])
            .collect()),
        FrameFormat::GRAY => Ok(data.iter().flat_map(|&x| [x, x, x, 255]).collect()),
    }
}

pub(crate) fn convert_to_rgba_buffer(
    fcc: FrameFormat,
    resolution: Resolution,
    data: &[u8],
    dest: &mut [u8],
) -> Result<(), NokhwaError> {
    match fcc {
        FrameFormat::MJPEG => buf_mjpeg_to_rgb(data, dest, true),
        FrameFormat::YUYV => buf_yuyv422_to_rgb(data, dest, true),
        FrameFormat::NV12 => buf_nv12_to_rgb(resolution, data, dest, true),
        FrameFormat::RAWRGB => {
            let expected = (data.len() / 3) * 4;
            if dest.len() != expected {
                return Err(NokhwaError::ProcessFrameError {
                    src: fcc,
                    destination: "RGBA".to_string(),
                    error: format!(
                        "destination buffer size mismatch (expected {expected}, got {})",
                        dest.len()
                    ),
                });
            }
            data.chunks_exact(3).enumerate().for_each(|(idx, px)| {
                let i = idx * 4;
                dest[i] = px[0];
                dest[i + 1] = px[1];
                dest[i + 2] = px[2];
                dest[i + 3] = 255;
            });
            Ok(())
        }
        FrameFormat::RAWBGR => {
            let expected = (data.len() / 3) * 4;
            if dest.len() != expected {
                return Err(NokhwaError::ProcessFrameError {
                    src: fcc,
                    destination: "RGBA".to_string(),
                    error: format!(
                        "destination buffer size mismatch (expected {expected}, got {})",
                        dest.len()
                    ),
                });
            }
            data.chunks_exact(3).enumerate().for_each(|(idx, px)| {
                let i = idx * 4;
                dest[i] = px[2];
                dest[i + 1] = px[1];
                dest[i + 2] = px[0];
                dest[i + 3] = 255;
            });
            Ok(())
        }
        FrameFormat::GRAY => {
            if dest.len() != data.len() * 4 {
                return Err(NokhwaError::ProcessFrameError {
                    src: fcc,
                    destination: "RGBA".to_string(),
                    error: "Bad buffer length".to_string(),
                });
            }
            data.iter().enumerate().for_each(|(idx, &pxv)| {
                let i = idx * 4;
                dest[i] = pxv;
                dest[i + 1] = pxv;
                dest[i + 2] = pxv;
                dest[i + 3] = 255;
            });
            Ok(())
        }
    }
}

/// Note: For YUYV and NV12, luma is extracted directly from the Y channel
/// (BT.601 weighted). For MJPEG, RAWRGB, and RAWBGR, a simple average
/// `(R+G+B)/3` is used rather than perceptual luminance weights.
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
pub(crate) fn convert_to_luma(
    fcc: FrameFormat,
    resolution: Resolution,
    data: &[u8],
) -> Result<Vec<u8>, NokhwaError> {
    match fcc {
        FrameFormat::GRAY => Ok(data.to_vec()),
        // Direct Y-channel extraction for YUYV and NV12
        FrameFormat::YUYV => {
            let mut dest = vec![0u8; resolution.width() as usize * resolution.height() as usize];
            buf_yuyv_extract_luma(data, &mut dest)?;
            Ok(dest)
        }
        FrameFormat::NV12 => {
            let mut dest = vec![0u8; resolution.width() as usize * resolution.height() as usize];
            buf_nv12_extract_luma(resolution, data, &mut dest)?;
            Ok(dest)
        }
        // For MJPEG, decode to RGB first then average
        FrameFormat::MJPEG => Ok(mjpeg_to_rgb(data, false)?
            .chunks_exact(3)
            .map(|x| {
                let sum = u16::from(x[0]) + u16::from(x[1]) + u16::from(x[2]);
                (sum / 3) as u8
            })
            .collect()),
        FrameFormat::RAWRGB | FrameFormat::RAWBGR => Ok(data
            .chunks_exact(3)
            .map(|px| {
                let sum = u16::from(px[0]) + u16::from(px[1]) + u16::from(px[2]);
                (sum / 3) as u8
            })
            .collect()),
    }
}

pub(crate) fn convert_to_luma_buffer(
    fcc: FrameFormat,
    resolution: Resolution,
    data: &[u8],
    dest: &mut [u8],
) -> Result<(), NokhwaError> {
    match fcc {
        FrameFormat::GRAY => {
            if dest.len() != data.len() {
                return Err(NokhwaError::ProcessFrameError {
                    src: fcc,
                    destination: "Luma".to_string(),
                    error: format!(
                        "destination buffer size mismatch (expected {}, got {})",
                        data.len(),
                        dest.len()
                    ),
                });
            }
            dest.copy_from_slice(data);
            Ok(())
        }
        FrameFormat::YUYV => buf_yuyv_extract_luma(data, dest),
        FrameFormat::NV12 => buf_nv12_extract_luma(resolution, data, dest),
        FrameFormat::RAWRGB | FrameFormat::RAWBGR => {
            let pixel_count = data.len() / 3;
            if dest.len() != pixel_count {
                return Err(NokhwaError::ProcessFrameError {
                    src: fcc,
                    destination: "Luma".to_string(),
                    error: format!(
                        "destination buffer size mismatch (expected {pixel_count}, got {})",
                        dest.len()
                    ),
                });
            }
            #[allow(clippy::cast_possible_truncation)]
            for (idx, px) in data.chunks_exact(3).enumerate() {
                dest[idx] = ((u16::from(px[0]) + u16::from(px[1]) + u16::from(px[2])) / 3) as u8;
            }
            Ok(())
        }
        FrameFormat::MJPEG => {
            let luma = convert_to_luma(fcc, resolution, data)?;
            if dest.len() < luma.len() {
                return Err(NokhwaError::ProcessFrameError {
                    src: fcc,
                    destination: "Luma".to_string(),
                    error: "Destination buffer too small".to_string(),
                });
            }
            dest[..luma.len()].copy_from_slice(&luma);
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
