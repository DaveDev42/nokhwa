/*
 * Copyright 2022 l1npengtul <l1npengtul@protonmail.com> / The Nokhwa Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 */

use nokhwa::{CameraSession, OpenRequest, OpenedCamera};
use nokhwa_core::error::NokhwaError;
use nokhwa_core::types::CameraIndex;

fn main() -> Result<(), NokhwaError> {
    let opened = CameraSession::open(CameraIndex::Index(0), OpenRequest::any())?;
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
