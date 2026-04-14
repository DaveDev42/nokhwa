/*
 * Copyright 2026 Dave Choi / The Nokhwa Contributors
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

use minifb::{Key, Window, WindowOptions};
use nokhwa::error::NokhwaError;
use nokhwa::format_types::Mjpeg;
use nokhwa::frame::{Frame, IntoRgb};
use nokhwa::utils::CameraIndex;
use nokhwa::{
    nokhwa_initialize, open, CameraRunner, OpenRequest, OpenedCamera, RunnerConfig,
};
use std::time::Duration;

fn main() {
    nokhwa_initialize(|granted| {
        if !granted {
            eprintln!("camera permission denied");
            return;
        }
        if let Err(e) = run() {
            eprintln!("error: {e}");
        }
    });
    // Keep the main thread alive briefly so the init callback has a chance
    // to run on macOS (where permission dialogs are async).
    std::thread::sleep(Duration::from_millis(200));
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

    let width = negotiated.resolution().width() as usize;
    let height = negotiated.resolution().height() as usize;
    println!(
        "opened camera: {}x{} @ {} fps ({:?})",
        width,
        height,
        negotiated.frame_rate(),
        negotiated.format()
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
                let rgb = Frame::<Mjpeg>::new(buf).into_rgb().materialize()?;
                let (rgb_w, rgb_h) = (rgb.width() as usize, rgb.height() as usize);
                if rgb_w * rgb_h != pixels.len() {
                    // Resolution changed mid-stream (rare); resize the buffer.
                    pixels = vec![0; rgb_w * rgb_h];
                }
                for (dst, src) in pixels.iter_mut().zip(rgb.chunks_exact(3)) {
                    *dst = (u32::from(src[0]) << 16)
                        | (u32::from(src[1]) << 8)
                        | u32::from(src[2]);
                }
                window
                    .update_with_buffer(&pixels, rgb_w, rgb_h)
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
