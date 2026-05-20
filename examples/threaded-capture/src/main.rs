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

use nokhwa::error::NokhwaError;
use nokhwa::format_types::{Mjpeg, Nv12, RawBgr, RawRgb, Yuyv};
use nokhwa::frame::{Frame, IntoRgba, RgbaConversion};
use nokhwa::utils::{ApiBackend, CameraIndex, FrameFormat};
use nokhwa::{
    nokhwa_initialize, open, query, Buffer, CameraRunner, OpenRequest, RunnerConfig,
};
use std::time::Duration;

fn main() -> Result<(), NokhwaError> {
    // only needs to be run on OSX
    nokhwa_initialize(|granted| {
        println!("User said {granted}");
    });

    let cameras = query(ApiBackend::Auto)?;
    for cam in &cameras {
        println!("{cam:?}");
    }

    let index = cameras
        .first()
        .map(|c| c.index().clone())
        .unwrap_or(CameraIndex::Index(0));

    let opened = open(index, OpenRequest::any())?;
    let runner = CameraRunner::spawn(opened, RunnerConfig::default())?;
    let frames = runner
        .frames()
        .ok_or_else(|| NokhwaError::general("runner has no frames channel"))?;

    for _ in 0..10 {
        let buffer = frames
            .recv_timeout(Duration::from_secs(2))
            .map_err(|e| NokhwaError::general(e.to_string()))?;
        println!(
            "callback: received buffer of {} bytes",
            buffer.buffer().len()
        );
        let image = decode_to_rgba(buffer)?.materialize()?;
        println!(
            "poll: {}x{} ({} bytes)",
            image.width(),
            image.height(),
            image.len()
        );
    }

    runner.stop()
}

/// Wrap a `Buffer` in the typed `Frame<F>` matching its own fourcc and
/// start a lazy RGBA conversion. Dispatching on the buffer's format
/// avoids the panic that `Frame::<Mjpeg>::new` raises when the backend
/// negotiates a non-MJPEG format (most Linux webcams pick YUYV).
fn decode_to_rgba(buf: Buffer) -> Result<RgbaConversion, NokhwaError> {
    match buf.source_frame_format() {
        FrameFormat::MJPEG => Ok(Frame::<Mjpeg>::try_new(buf)?.into_rgba()),
        FrameFormat::YUYV => Ok(Frame::<Yuyv>::try_new(buf)?.into_rgba()),
        FrameFormat::NV12 => Ok(Frame::<Nv12>::try_new(buf)?.into_rgba()),
        FrameFormat::RAWRGB => Ok(Frame::<RawRgb>::try_new(buf)?.into_rgba()),
        FrameFormat::RAWBGR => Ok(Frame::<RawBgr>::try_new(buf)?.into_rgba()),
        FrameFormat::GRAY => Err(NokhwaError::general(
            "threaded-capture does not support GRAY/Luma cameras",
        )),
    }
}
