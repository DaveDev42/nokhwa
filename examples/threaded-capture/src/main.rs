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

use nokhwa::format_types::Mjpeg;
use nokhwa::frame::{Frame, IntoRgba};
use nokhwa::{
    nokhwa_initialize, query,
    utils::{ApiBackend, RequestedFormat, RequestedFormatType},
    CallbackCamera,
};

fn main() {
    // only needs to be run on OSX
    nokhwa_initialize(|granted| {
        println!("User said {}", granted);
    });
    let cameras = query(ApiBackend::Auto).unwrap();
    cameras.iter().for_each(|cam| println!("{:?}", cam));

    let format = RequestedFormat::new::<Mjpeg>(RequestedFormatType::AbsoluteHighestFrameRate);

    let first_camera = cameras.first().unwrap();

    let mut threaded: CallbackCamera<Mjpeg> =
        CallbackCamera::new(first_camera.index().clone(), format, |buffer| {
            println!(
                "callback: received buffer of {} bytes",
                buffer.buffer().len()
            );
        })
        .unwrap();
    threaded.open_stream().unwrap();
    loop {
        let buffer = threaded.poll_frame().unwrap();
        let frame: Frame<Mjpeg> = Frame::new(buffer);
        let image = frame.into_rgba().materialize().unwrap();
        println!(
            "poll: {}x{} ({} bytes)",
            image.width(),
            image.height(),
            image.len()
        );
    }
}
