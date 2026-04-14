/*
 * Copyright 2026 The Nokhwa Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 */

//! Live-view camera demo.
//!
//! Opens the first camera via `nokhwa::open`, spawns a `CameraRunner`
//! to pull frames on a worker thread, decodes each frame to RGB, and
//! paints it to a `minifb` window. Close the window or press Esc to
//! exit.
//!
//! Run from this directory:
//!
//! ```text
//! cargo run --release
//! ```
//!
//! The demo dispatches on the camera's negotiated `FrameFormat`, so it
//! works regardless of whether the backend picks MJPEG, YUYV, NV12, or
//! a raw RGB/BGR pixel format.

#![allow(clippy::cast_possible_truncation)]

use minifb::{Key, Window, WindowOptions};
use nokhwa::error::NokhwaError;
use nokhwa::format_types::{Mjpeg, Nv12, RawBgr, RawRgb, Yuyv};
use nokhwa::frame::{Frame, IntoRgb, RgbConversion};
use nokhwa::utils::{CameraIndex, FrameFormat};
use nokhwa::{
    nokhwa_initialize, open, Buffer, CameraRunner, OpenRequest, OpenedCamera, RunnerConfig,
};
use std::sync::mpsc;
use std::time::Duration;

fn main() {
    // `nokhwa_initialize` on macOS requests camera permission asynchronously.
    // Block the main thread on the permission result so `run()` only starts
    // after the user has (or has not) granted access — and so `main` does
    // not return while the permission dialog is still open.
    let (tx, rx) = mpsc::channel();
    nokhwa_initialize(move |granted| {
        let _ = tx.send(granted);
    });
    match rx.recv_timeout(Duration::from_secs(60)) {
        Ok(true) => {
            if let Err(e) = run() {
                eprintln!("error: {e}");
            }
        }
        Ok(false) => eprintln!("camera permission denied"),
        Err(_) => eprintln!("timed out waiting for camera permission"),
    }
}

fn run() -> Result<(), NokhwaError> {
    let opened = open(CameraIndex::Index(0), OpenRequest::any())?;
    let negotiated = match &opened {
        OpenedCamera::Stream(c) => c.negotiated_format(),
        OpenedCamera::Hybrid(c) => c.negotiated_format(),
        OpenedCamera::Shutter(_) => {
            return Err(NokhwaError::general(
                "live_view requires a streaming camera; got Shutter-only",
            ));
        }
    };

    // On every target nokhwa supports, `usize >= u32`, so widening is
    // infallible. The crate-level `cast_possible_truncation` allow
    // above covers the pedantic lint.
    let width = negotiated.resolution().width() as usize;
    let height = negotiated.resolution().height() as usize;
    let fcc = negotiated.format();
    println!(
        "opened camera: {}x{} @ {} fps ({:?})",
        width,
        height,
        negotiated.frame_rate(),
        fcc
    );

    let runner = CameraRunner::spawn(opened, RunnerConfig::default())?;
    let frames = runner
        .frames()
        .ok_or_else(|| NokhwaError::general("runner has no frames channel"))?;

    let mut window = Window::new(
        "nokhwa live_view — Esc to quit",
        width,
        height,
        WindowOptions::default(),
    )
    .map_err(|e| NokhwaError::general(format!("minifb: {e}")))?;
    window.set_target_fps(60);

    let mut pixels: Vec<u32> = vec![0; width * height];

    while window.is_open() && !window.is_key_down(Key::Escape) {
        match frames.recv_timeout(Duration::from_millis(500)) {
            Ok(buf) => {
                let rgb = decode_to_rgb(buf, fcc)?.materialize()?;
                // Cameras do not renegotiate resolution mid-stream; a
                // mismatch here would mean either a nokhwa bug or a
                // backend firmware quirk. Fail loud rather than paper
                // over it.
                assert_eq!(
                    rgb.width() as usize * rgb.height() as usize,
                    pixels.len(),
                    "frame resolution changed mid-stream"
                );
                // minifb's u32 layout is 0x00RRGGBB (top byte ignored).
                for (dst, src) in pixels.iter_mut().zip(rgb.chunks_exact(3)) {
                    *dst = (u32::from(src[0]) << 16)
                        | (u32::from(src[1]) << 8)
                        | u32::from(src[2]);
                }
                window
                    .update_with_buffer(&pixels, width, height)
                    .map_err(|e| NokhwaError::general(format!("minifb: {e}")))?;
            }
            Err(_) => {
                // No frame arrived in time — keep the window responsive.
                window.update();
            }
        }
    }

    runner.stop()
}

/// Wrap a `Buffer` in the correct typed `Frame<F>` based on the camera's
/// negotiated fourcc and start a lazy RGB conversion. Returning the
/// `RgbConversion` (instead of a materialized image) keeps the match arms
/// uniform.
fn decode_to_rgb(buf: Buffer, fcc: FrameFormat) -> Result<RgbConversion, NokhwaError> {
    match fcc {
        FrameFormat::MJPEG => Ok(Frame::<Mjpeg>::try_new(buf)?.into_rgb()),
        FrameFormat::YUYV => Ok(Frame::<Yuyv>::try_new(buf)?.into_rgb()),
        FrameFormat::NV12 => Ok(Frame::<Nv12>::try_new(buf)?.into_rgb()),
        FrameFormat::RAWRGB => Ok(Frame::<RawRgb>::try_new(buf)?.into_rgb()),
        FrameFormat::RAWBGR => Ok(Frame::<RawBgr>::try_new(buf)?.into_rgb()),
        FrameFormat::GRAY => Err(NokhwaError::general(
            "live_view does not support GRAY/Luma cameras",
        )),
    }
}
