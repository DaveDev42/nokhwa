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

#[cfg(all(feature = "input-v4l", target_os = "linux"))]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "input-v4l")))]
pub use nokhwa_bindings_linux::V4LCaptureDevice;
#[cfg(any(
    all(
        feature = "input-avfoundation",
        any(target_os = "macos", target_os = "ios")
    ),
    all(
        feature = "docs-only",
        feature = "docs-nolink",
        feature = "input-avfoundation"
    )
))]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "input-avfoundation")))]
pub use nokhwa_bindings_macos::AVFoundationCaptureDevice;
#[cfg(any(
    all(feature = "input-msmf", target_os = "windows"),
    all(feature = "docs-only", feature = "docs-nolink", feature = "input-msmf")
))]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "input-msmf")))]
pub use nokhwa_bindings_windows::MediaFoundationCaptureDevice;
#[cfg(feature = "input-opencv")]
mod opencv_backend;
#[cfg(feature = "input-opencv")]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "input-opencv")))]
pub use opencv_backend::OpenCvCaptureDevice;
