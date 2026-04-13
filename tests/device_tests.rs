//! Integration tests requiring a physical camera device.
//! Only compiled when `device-test` feature is enabled.
//!
//! NOTE: The 0.12-era tests in this file referenced the removed
//! `Camera` / `CallbackCamera` types. They are pending migration to the
//! 0.14 `nokhwa::open` / `OpenedCamera` API — see TODO.md.

#![cfg(feature = "device-test")]

#[test]
fn device_tests_pending_migration_to_0_14_api() {
    // TODO: port the original `device_tests.rs` suite to the new
    // `nokhwa::open` / `OpenedCamera` API.
}
