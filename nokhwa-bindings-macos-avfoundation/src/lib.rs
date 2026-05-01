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

// hello, future peng here
// whatever is written here will induce horrors uncomprehendable.
// save yourselves. write apple code in swift and bind it to rust.

// <some change so we can call this 0.10.4>

#![deny(clippy::pedantic)]
#![warn(clippy::all)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod callback;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod capture;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod device;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod ffi;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod session;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod types;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod util;

mod hotplug;
pub use hotplug::AVFoundationHotplugContext;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use callback::{
    current_authorization_status, request_permission_with_callback, AVCaptureVideoCallback,
};
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use device::{
    get_raw_device_info, query, AVCaptureDeviceFormatWrapper, AVCaptureDeviceWrapper,
    AVFrameRateRangeWrapper,
};
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use ffi::*;

// Re-export typed AVFoundation types for downstream use
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use capture::AVFoundationCaptureDevice;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use objc2::rc::Retained;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use objc2_av_foundation::{AVCaptureDeviceInput, AVCaptureSession, AVCaptureVideoDataOutput};
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use types::{AVAuthorizationStatus, AVCaptureDeviceTypeLocal, AVMediaTypeLocal};
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use util::{CompressionData, DataPipe};
