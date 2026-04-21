/*
 * Copyright 2022 l1npengtul <l1npengtul@protonmail.com> / The Nokhwa Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 */

use nokhwa::{open, OpenRequest, OpenedCamera};
use nokhwa_core::error::NokhwaError;
use nokhwa_core::types::CameraIndex;

fn main() -> Result<(), NokhwaError> {
    // Accept either a non-negative integer (local device index) or a
    // URL-like string (rtsp/http/file) from the CLI. URLs route through
    // the GStreamer backend via the session-4 dispatch — requires the
    // `input-gstreamer` feature at build time.
    let index = match std::env::args().nth(1) {
        Some(s) => match s.parse::<u32>() {
            Ok(i) => CameraIndex::Index(i),
            Err(_) => CameraIndex::String(s),
        },
        None => CameraIndex::Index(0),
    };
    let opened = open(index, OpenRequest::any())?;
    let OpenedCamera::Stream(mut cam) = opened else {
        return Err(NokhwaError::general("expected stream-capable camera"));
    };
    cam.open()?;
    for _ in 0..10 {
        let f = cam.frame()?;
        println!(
            "frame: {} bytes @ {}x{}",
            f.buffer().len(),
            f.resolution().width(),
            f.resolution().height()
        );
    }
    cam.close()
}
