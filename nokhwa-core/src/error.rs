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

use crate::types::{ApiBackend, FrameFormat};
use std::time::Duration;
use thiserror::Error;

/// All errors in `nokhwa`.
#[allow(clippy::module_name_repetitions)]
#[derive(Error, Debug, Clone)]
pub enum NokhwaError {
    #[error("Uninitialized Camera. Call `init()` first!")]
    UninitializedError,
    #[error("Could not initialize {backend}: {error}")]
    InitializeError { backend: ApiBackend, error: String },
    #[error("Could not shutdown {backend}: {error}")]
    ShutdownError { backend: ApiBackend, error: String },
    #[error("Error{}: {message}", backend.map(|b| format!(" (backend {b})")).unwrap_or_default())]
    GeneralError {
        message: String,
        backend: Option<ApiBackend>,
    },
    #[error("Could not generate required structure {structure}: {error}")]
    StructureError { structure: String, error: String },
    #[error("Could not open device {0}: {1}")]
    OpenDeviceError(String, String),
    #[error("Could not get device property {property}: {error}")]
    GetPropertyError { property: String, error: String },
    #[error("Could not set device property {property} with value {value}: {error}")]
    SetPropertyError {
        property: String,
        value: String,
        error: String,
    },
    #[error("Could not open device stream{}: {message}", backend.map(|b| format!(" (backend {b})")).unwrap_or_default())]
    OpenStreamError {
        message: String,
        backend: Option<ApiBackend>,
    },
    #[error("Could not capture frame{}: {message}", format.map(|f| format!(" (format {f:?})")).unwrap_or_default())]
    ReadFrameError {
        message: String,
        format: Option<FrameFormat>,
    },
    #[error("Could not process frame {src} to {destination}: {error}")]
    ProcessFrameError {
        src: FrameFormat,
        destination: String,
        error: String,
    },
    #[error("Could not stop stream{}: {message}", backend.map(|b| format!(" (backend {b})")).unwrap_or_default())]
    StreamShutdownError {
        message: String,
        backend: Option<ApiBackend>,
    },
    #[error("This operation is not supported by backend {0}.")]
    UnsupportedOperationError(ApiBackend),
    #[error("This operation is not implemented yet: {0}")]
    NotImplementedError(String),
    #[error("Frame capture timed out after {0:?}")]
    TimeoutError(Duration),
}

// Helper constructors for backwards compatibility — allow creating structured
// variants from a plain String, defaulting optional context fields to None.
impl NokhwaError {
    pub fn general(message: impl Into<String>) -> Self {
        Self::GeneralError {
            message: message.into(),
            backend: None,
        }
    }

    pub fn open_stream(message: impl Into<String>) -> Self {
        Self::OpenStreamError {
            message: message.into(),
            backend: None,
        }
    }

    pub fn read_frame(message: impl Into<String>) -> Self {
        Self::ReadFrameError {
            message: message.into(),
            format: None,
        }
    }

    pub fn stream_shutdown(message: impl Into<String>) -> Self {
        Self::StreamShutdownError {
            message: message.into(),
            backend: None,
        }
    }
}
