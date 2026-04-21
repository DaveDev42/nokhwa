//! Live hotplug probe.
//!
//! Run with:
//!
//! ```text
//! cargo run --features input-msmf         --example hotplug_probe   # Windows
//! cargo run --features input-v4l          --example hotplug_probe   # Linux
//! cargo run --features input-avfoundation --example hotplug_probe   # macOS
//! ```
//!
//! Listens for fifteen seconds on the native hotplug source, printing
//! every `Connected` / `Disconnected` event that arrives. Unplug and
//! re-plug a webcam (or `modprobe -r v4l2loopback` + re-add on Linux)
//! while the probe is running to verify the poll loop is picking up
//! device-change signals.

use std::time::Duration;

#[cfg(all(feature = "input-msmf", target_os = "windows"))]
fn backend_context() -> Box<dyn nokhwa_core::traits::HotplugSource> {
    Box::new(nokhwa::backends::hotplug::MediaFoundationHotplugContext::new())
}

#[cfg(all(feature = "input-v4l", target_os = "linux"))]
fn backend_context() -> Box<dyn nokhwa_core::traits::HotplugSource> {
    Box::new(nokhwa::backends::hotplug::V4LHotplugContext::new())
}

#[cfg(all(
    feature = "input-avfoundation",
    any(target_os = "macos", target_os = "ios")
))]
fn backend_context() -> Box<dyn nokhwa_core::traits::HotplugSource> {
    Box::new(nokhwa::backends::hotplug::AVFoundationHotplugContext::new())
}

#[cfg(any(
    all(feature = "input-msmf", target_os = "windows"),
    all(feature = "input-v4l", target_os = "linux"),
    all(
        feature = "input-avfoundation",
        any(target_os = "macos", target_os = "ios")
    ),
))]
fn main() {
    let mut ctx = backend_context();
    let mut poll = ctx.take_hotplug_events().expect("take_hotplug_events");
    println!("Listening for 15 seconds — unplug + replug a camera to see events.");
    for i in 0..30 {
        if let Some(evt) = poll.next_timeout(Duration::from_millis(500)) {
            println!("  [{:5}ms] {evt:?}", i * 500);
        }
    }
    println!("Done.");
}

#[cfg(not(any(
    all(feature = "input-msmf", target_os = "windows"),
    all(feature = "input-v4l", target_os = "linux"),
    all(
        feature = "input-avfoundation",
        any(target_os = "macos", target_os = "ios")
    ),
)))]
fn main() {
    eprintln!(
        "This example requires a hotplug-capable backend for the current OS.\n\
         Try one of:\n\
           cargo run --features input-msmf         --example hotplug_probe   # Windows\n\
           cargo run --features input-v4l          --example hotplug_probe   # Linux\n\
           cargo run --features input-avfoundation --example hotplug_probe   # macOS"
    );
}
