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

#![allow(clippy::not_unsafe_ptr_arg_deref)]

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[macro_use]
extern crate objc;

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod internal;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use crate::internal::*;
