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

#![cfg(feature = "wgpu-types")]

use crate::{
    error::NokhwaError,
    types::{FrameFormat, Resolution},
};
use wgpu::{Extent3d, Texture as WgpuTexture, TextureFormat};

/// A GPU texture containing raw camera frame data in its native pixel format,
/// without conversion to RGBA. Consumers can use GPU shaders to decode the
/// data according to the [`FrameFormat`] and [`TextureFormat`] provided.
#[derive(Debug)]
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
