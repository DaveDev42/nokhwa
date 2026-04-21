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

//! Backend-level hotplug contexts. Each `HotplugSource`-implementing
//! type here wraps a platform's device-change notification plumbing:
//! create one, call [`HotplugSource::take_hotplug_events`][tho] once,
//! and poll the returned handle.
//!
//! [tho]: nokhwa_core::traits::HotplugSource::take_hotplug_events

#[cfg(all(feature = "input-msmf", target_os = "windows"))]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "input-msmf")))]
pub use nokhwa_bindings_windows_msmf::MediaFoundationHotplugContext;

#[cfg(all(feature = "input-v4l", target_os = "linux"))]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "input-v4l")))]
pub use nokhwa_bindings_linux_v4l::V4LHotplugContext;
