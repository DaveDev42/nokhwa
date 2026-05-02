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

#[cfg(not(all(
    feature = "input-avfoundation",
    any(target_os = "macos", target_os = "ios")
)))]
fn init_avfoundation(callback: impl Fn(bool) + Send + 'static) {
    callback(true);
}

#[cfg(all(
    feature = "input-avfoundation",
    any(target_os = "macos", target_os = "ios")
))]
fn init_avfoundation(callback: impl Fn(bool) + Send + Sync + 'static) {
    use nokhwa_bindings_macos_avfoundation::request_permission_with_callback;

    request_permission_with_callback(callback);
}

#[cfg(not(all(
    feature = "input-avfoundation",
    any(target_os = "macos", target_os = "ios")
)))]
fn status_avfoundation() -> bool {
    true
}

#[cfg(all(
    feature = "input-avfoundation",
    any(target_os = "macos", target_os = "ios")
))]
fn status_avfoundation() -> bool {
    use nokhwa_bindings_macos_avfoundation::{current_authorization_status, AVAuthorizationStatus};

    matches!(
        current_authorization_status(),
        AVAuthorizationStatus::Authorized
    )
}

// todo: make this work on browser code
/// Initialize `nokhwa`
/// It is your responsibility to call this function before anything else, but only on `MacOS`.
///
/// The `on_complete` is called after initialization (a.k.a User granted permission). The callback's argument
/// is weather the initialization was successful or not
pub fn nokhwa_initialize(on_complete: impl Fn(bool) + Send + Sync + 'static) {
    init_avfoundation(on_complete);
}

/// Check the status if `nokhwa`
/// True if the initialization is successful (ready-to-use)
#[must_use]
pub fn nokhwa_check() -> bool {
    status_avfoundation()
}

#[cfg(test)]
#[cfg(not(all(
    feature = "input-avfoundation",
    any(target_os = "macos", target_os = "ios")
)))]
mod tests {
    //! On platforms / feature combos where `AVFoundation` is *not* available
    //! the init module degrades to no-op stubs. These tests pin that
    //! degradation so a refactor can't accidentally turn the stubs into
    //! "not-yet-supported" errors or deferred work — non-`AVFoundation`
    //! callers must observe synchronous, success-immediate behaviour.

    use super::{nokhwa_check, nokhwa_initialize};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    #[test]
    fn nokhwa_check_returns_true_without_avfoundation() {
        assert!(nokhwa_check());
    }

    #[test]
    fn nokhwa_initialize_invokes_callback_synchronously_with_true() {
        let fired = Arc::new(AtomicBool::new(false));
        let result = Arc::new(AtomicBool::new(false));
        {
            let fired = Arc::clone(&fired);
            let result = Arc::clone(&result);
            nokhwa_initialize(move |ok| {
                result.store(ok, Ordering::SeqCst);
                fired.store(true, Ordering::SeqCst);
            });
        }
        // Stub is synchronous: the callback must have already fired
        // by the time `nokhwa_initialize` returned.
        assert!(
            fired.load(Ordering::SeqCst),
            "non-AVFoundation init stub must call its callback synchronously"
        );
        assert!(
            result.load(Ordering::SeqCst),
            "non-AVFoundation init stub must report success"
        );
    }
}
