//! Live hotplug probe for the MediaFoundation backend.
//!
//! Run with:
//!
//! ```text
//! cargo run --features input-msmf --example hotplug_probe
//! ```
//!
//! Listens for fifteen seconds on the MSMF hotplug source, printing
//! every `Connected` / `Disconnected` event that arrives. Unplug and
//! re-plug a webcam while the probe is running to verify the poll loop
//! is picking up device-change signals.

#[cfg(all(feature = "input-msmf", target_os = "windows"))]
fn main() {
    use nokhwa::backends::hotplug::MediaFoundationHotplugContext;
    use nokhwa_core::traits::HotplugSource;
    use std::time::Duration;

    let mut ctx = MediaFoundationHotplugContext::new();
    let mut poll = ctx.take_hotplug_events().expect("take_hotplug_events");
    println!("Listening for 15 seconds — unplug + replug a webcam to see events.");
    for i in 0..30 {
        if let Some(evt) = poll.next_timeout(Duration::from_millis(500)) {
            println!("  [{:5}ms] {evt:?}", i * 500);
        }
    }
    println!("Done.");
}

#[cfg(not(all(feature = "input-msmf", target_os = "windows")))]
fn main() {
    eprintln!(
        "This example requires the `input-msmf` feature on a Windows host.\n\
         Try: cargo run --features input-msmf --example hotplug_probe"
    );
}
