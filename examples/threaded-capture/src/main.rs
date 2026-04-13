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
use nokhwa::format_types::Mjpeg;
use nokhwa::frame::{Frame, IntoRgba};
use nokhwa::utils::{ApiBackend, CameraIndex};
use nokhwa::{
    nokhwa_initialize, query, CameraRunner, open, OpenRequest, RunnerConfig,
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
        let frame: Frame<Mjpeg> = Frame::new(buffer);
        let image = frame.into_rgba().materialize()?;
        println!(
            "poll: {}x{} ({} bytes)",
            image.width(),
            image.height(),
            image.len()
        );
    }

    runner.stop()
}
