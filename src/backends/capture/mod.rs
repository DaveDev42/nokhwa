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
pub use nokhwa_bindings_linux_v4l::V4LCaptureDevice;

// NOTE: `V4LCaptureDevice<'a>` carries a lifetime parameter tied to a
// `MutexGuard<Device>` borrowed inside `open()`. Instantiating the type with
// `'static` (as would be required to store a boxed `V4LCaptureDevice<'static>`
// behind `dyn AnyDevice`) cannot be proved sound without an `unsafe` transmute
// of the stream handle. That rework is tracked for 0.13.1 — see TODO.md and
// CHANGELOG.md. For 0.13.0, the V4L path through `CameraSession::open` is
// intentionally stubbed out in `session.rs`; users may still construct
// `V4LCaptureDevice` directly via the `nokhwa-bindings-linux-v4l` crate.
#[cfg(all(feature = "input-v4l", target_os = "linux"))]
crate::nokhwa_backend!(nokhwa_bindings_linux_v4l::V4LCaptureDevice<'static>: FrameSource);
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
pub use nokhwa_bindings_macos_avfoundation::AVFoundationCaptureDevice;

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
crate::nokhwa_backend!(nokhwa_bindings_macos_avfoundation::AVFoundationCaptureDevice: FrameSource);
#[cfg(any(
    all(feature = "input-msmf", target_os = "windows"),
    all(feature = "docs-only", feature = "docs-nolink", feature = "input-msmf")
))]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "input-msmf")))]
pub use nokhwa_bindings_windows_msmf::MediaFoundationCaptureDevice;

#[cfg(any(
    all(feature = "input-msmf", target_os = "windows"),
    all(feature = "docs-only", feature = "docs-nolink", feature = "input-msmf")
))]
crate::nokhwa_backend!(
    nokhwa_bindings_windows_msmf::MediaFoundationCaptureDevice: FrameSource
);
// input-opencv backend is pending migration to the 0.13.0 trait split.
// See TODO.md (T21/T22). The opencv_backend.rs file is preserved on disk
// as dead code until the migration lands.
// #[cfg(feature = "input-opencv")]
// mod opencv_backend;
// #[cfg(feature = "input-opencv")]
// #[cfg_attr(feature = "docs-features", doc(cfg(feature = "input-opencv")))]
// pub use opencv_backend::OpenCvCaptureDevice;
