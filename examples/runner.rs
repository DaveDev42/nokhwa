/*
 * Copyright 2022 l1npengtul <l1npengtul@protonmail.com> / The Nokhwa Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 */

#![cfg(feature = "runner")]

use nokhwa::{open, CameraRunner, OpenRequest, RunnerConfig};
use nokhwa_core::error::NokhwaError;
use nokhwa_core::types::CameraIndex;
use std::time::Duration;

fn main() -> Result<(), NokhwaError> {
    let opened = open(CameraIndex::Index(0), OpenRequest::any())?;
    let runner = CameraRunner::spawn(opened, RunnerConfig::default())?;
    if let Some(rx) = runner.frames() {
        for _ in 0..5 {
            let f = rx
                .recv_timeout(Duration::from_secs(2))
                .map_err(|e| NokhwaError::general(e.to_string()))?;
            println!("frame: {} bytes", f.buffer().len());
        }
    }
    runner.stop()
}
