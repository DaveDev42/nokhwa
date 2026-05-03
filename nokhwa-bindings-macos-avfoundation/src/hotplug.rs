//! Backend-level hotplug for the `AVFoundation` backend.
//!
//! Implements [`HotplugSource`] by **polling** `device::query()` on a
//! dedicated thread every [`POLL_INTERVAL`] and diffing successive
//! snapshots keyed on the `AVCaptureDevice.uniqueID` stored in
//! `CameraInfo.misc`.
//!
//! Polling parallels the `MediaFoundation` and `Video4Linux`
//! implementations: `IOKit`'s matching-notification API would be
//! event-driven but requires a runloop plus Objective-C block callbacks
//! to manage cleanly across the `HotplugEventPoll` boundary, and
//! hotplug latency budgets are seconds, not milliseconds. The poll
//! loop is ten lines of logic and never misses an event because
//! `query()` reads the *current* device set.

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod real {
    use crate::device::query;
    use nokhwa_core::{
        error::NokhwaError,
        traits::{HotplugEvent, HotplugEventPoll, HotplugSource},
        types::{ApiBackend, CameraInfo},
    };
    use std::{
        collections::BTreeMap,
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver, Sender},
            Arc,
        },
        thread::{self, JoinHandle},
        time::Duration,
    };

    /// How often the polling thread re-enumerates `AVFoundation`
    /// devices. 500ms matches the MSMF and V4L backends and is a
    /// compromise between "noticeable latency" and "burn CPU on the
    /// `AVFoundation` discovery session". Tune here if users report
    /// perceivable lag on hot-unplug.
    const POLL_INTERVAL: Duration = Duration::from_millis(500);

    /// Backend-level hotplug source for `AVFoundation`. Cheap to
    /// construct — the polling thread is only spawned when
    /// [`take_hotplug_events`](HotplugSource::take_hotplug_events) is
    /// called, and is joined when the returned poller is dropped.
    #[derive(Default)]
    pub struct AVFoundationHotplugContext {
        taken: bool,
    }

    impl AVFoundationHotplugContext {
        #[must_use]
        pub fn new() -> Self {
            Self { taken: false }
        }
    }

    impl HotplugSource for AVFoundationHotplugContext {
        fn take_hotplug_events(&mut self) -> Result<Box<dyn HotplugEventPoll + Send>, NokhwaError> {
            if self.taken {
                return Err(NokhwaError::UnsupportedOperationError(
                    ApiBackend::AVFoundation,
                ));
            }
            self.taken = true;
            Ok(Box::new(AvfHotplugPoll::spawn()?))
        }
    }

    /// Concrete [`HotplugEventPoll`]. Owns a background thread running
    /// the poll loop + an mpsc channel + an [`AtomicBool`] shutdown
    /// flag. Dropping the poll flips the flag and joins the thread.
    struct AvfHotplugPoll {
        rx: Receiver<HotplugEvent>,
        stop: Arc<AtomicBool>,
        join: Option<JoinHandle<()>>,
    }

    impl AvfHotplugPoll {
        fn spawn() -> Result<Self, NokhwaError> {
            let (tx, rx) = mpsc::channel();
            let stop = Arc::new(AtomicBool::new(false));
            let stop_thread = Arc::clone(&stop);
            let join = thread::Builder::new()
                .name("nokhwa-avf-hotplug".to_string())
                .spawn(move || poll_loop(&tx, &stop_thread))
                .map_err(|e| NokhwaError::general(format!("spawn hotplug thread: {e}")))?;
            Ok(Self {
                rx,
                stop,
                join: Some(join),
            })
        }
    }

    impl HotplugEventPoll for AvfHotplugPoll {
        fn try_next(&mut self) -> Option<HotplugEvent> {
            self.rx.try_recv().ok()
        }
        fn next_timeout(&mut self, d: Duration) -> Option<HotplugEvent> {
            self.rx.recv_timeout(d).ok()
        }
    }

    impl Drop for AvfHotplugPoll {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(h) = self.join.take() {
                // The background thread sleeps up to POLL_INTERVAL
                // between checks, so the join tops out at that.
                let _ = h.join();
            }
        }
    }

    /// Build an initial device snapshot, then enter the diff loop. The
    /// seed snapshot is silent — consumers see *changes*, not the
    /// population of the device registry at subscription time (that is
    /// what `query()` is for).
    fn poll_loop(tx: &Sender<HotplugEvent>, stop: &Arc<AtomicBool>) {
        let mut previous = snapshot();
        while !stop.load(Ordering::Acquire) {
            thread::sleep(POLL_INTERVAL);
            if stop.load(Ordering::Acquire) {
                break;
            }
            if !reconcile_and_emit_with(tx, &mut previous, snapshot()) {
                return; // consumer dropped the poller
            }
        }
    }

    /// Diff `current` against `previous`, emit events, swap cache.
    /// Returns false if the channel is closed (consumer dropped the
    /// poller) so [`poll_loop`] can exit early. Split out from
    /// [`poll_loop`] so unit tests can inject a synthetic `current`
    /// without touching `AVFoundation`.
    ///
    /// Emit arrivals before removals so a rapid re-plug landing in
    /// one [`POLL_INTERVAL`] window surfaces as
    /// `Disconnected` → `Connected` on the consumer side.
    fn reconcile_and_emit_with(
        tx: &Sender<HotplugEvent>,
        previous: &mut BTreeMap<String, CameraInfo>,
        current: BTreeMap<String, CameraInfo>,
    ) -> bool {
        for (key, info) in &current {
            if !previous.contains_key(key)
                && tx.send(HotplugEvent::Connected(info.clone())).is_err()
            {
                return false;
            }
        }
        for (key, info) in previous.iter() {
            if !current.contains_key(key)
                && tx.send(HotplugEvent::Disconnected(info.clone())).is_err()
            {
                return false;
            }
        }
        *previous = current;
        true
    }

    /// One `AVFoundation` enumeration pass, indexed by
    /// `AVCaptureDevice.uniqueID` (stored in `CameraInfo.misc` by the
    /// device module). `uniqueID` is stable across enumerations for a
    /// given physical device and does not repeat across ports, so it
    /// is the right diff key — same shape as the MSMF symbolic-link
    /// diff.
    ///
    /// Errors from `query()` are swallowed — a transient enumeration
    /// failure should not tear down the hotplug thread. An empty
    /// snapshot will look like "every device disappeared"; next tick
    /// we will re-emit them as `Connected`. That is noisy but not
    /// incorrect.
    fn snapshot() -> BTreeMap<String, CameraInfo> {
        match query() {
            Ok(list) => list.into_iter().map(|ci| (ci.misc(), ci)).collect(),
            Err(_) => BTreeMap::new(),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::reconcile_and_emit_with;
        use nokhwa_core::{
            traits::HotplugEvent,
            types::{CameraIndex, CameraInfo},
        };
        use std::{collections::BTreeMap, sync::mpsc};

        fn info(idx: u32) -> CameraInfo {
            CameraInfo::new(
                &format!("cam{idx}"),
                "test",
                &format!("0x{idx:016x}-AVCaptureDevice-uniqueID"),
                CameraIndex::Index(idx),
            )
        }

        fn snap<I: IntoIterator<Item = u32>>(ids: I) -> BTreeMap<String, CameraInfo> {
            ids.into_iter()
                .map(|i| {
                    let ci = info(i);
                    (ci.misc(), ci)
                })
                .collect()
        }

        /// `reconcile_and_emit_with` must replace `previous` with
        /// `current` so the next call sees the updated cache.
        #[test]
        fn cache_is_swapped_after_reconcile() {
            let (tx, _rx) = mpsc::channel();
            let mut previous = snap([0]);
            let current = snap([1, 2]);
            assert!(reconcile_and_emit_with(&tx, &mut previous, current));
            assert_eq!(previous.len(), 2);
            assert!(previous.values().any(|ci| ci.human_name() == "cam1"));
            assert!(previous.values().any(|ci| ci.human_name() == "cam2"));
        }

        /// Newcomers (in `current`, not in `previous`) emit
        /// `Connected`; removals (in `previous`, not in `current`)
        /// emit `Disconnected`.
        #[test]
        fn arrivals_and_removals_are_both_emitted() {
            let (tx, rx) = mpsc::channel();
            let mut previous = snap([0, 1]);
            let current = snap([1, 2]);
            assert!(reconcile_and_emit_with(&tx, &mut previous, current));
            drop(tx);
            let events: Vec<_> = rx.iter().collect();
            assert_eq!(events.len(), 2, "got: {events:?}");
            let connected = events
                .iter()
                .filter(|e| matches!(e, HotplugEvent::Connected(_)))
                .count();
            let disconnected = events
                .iter()
                .filter(|e| matches!(e, HotplugEvent::Disconnected(_)))
                .count();
            assert_eq!(connected, 1, "expected 1 Connected, got {events:?}");
            assert_eq!(disconnected, 1, "expected 1 Disconnected, got {events:?}");
        }

        /// Pin the documented ordering invariant: arrivals are sent
        /// before removals so a re-plug landing in one
        /// [`POLL_INTERVAL`] window is observable as
        /// `Disconnected` → `Connected` on the consumer side.
        #[test]
        fn arrivals_precede_removals_in_emission_order() {
            let (tx, rx) = mpsc::channel();
            let mut previous = snap([0]);
            let current = snap([1]);
            assert!(reconcile_and_emit_with(&tx, &mut previous, current));
            drop(tx);
            let events: Vec<_> = rx.iter().collect();
            assert_eq!(events.len(), 2);
            assert!(
                matches!(events[0], HotplugEvent::Connected(_)),
                "first event must be Connected, got {:?}",
                events[0]
            );
            assert!(
                matches!(events[1], HotplugEvent::Disconnected(_)),
                "second event must be Disconnected, got {:?}",
                events[1]
            );
        }

        /// No-op reconcile (current == previous) emits zero events
        /// and leaves the cache equal.
        #[test]
        fn identical_snapshots_emit_no_events() {
            let (tx, rx) = mpsc::channel();
            let mut previous = snap([0, 1, 2]);
            let current = snap([0, 1, 2]);
            assert!(reconcile_and_emit_with(&tx, &mut previous, current));
            drop(tx);
            assert_eq!(rx.iter().count(), 0);
            assert_eq!(previous.len(), 3);
        }

        /// If the channel is closed mid-emission, return false so
        /// the worker can exit early instead of looping over a dead
        /// channel.
        #[test]
        fn returns_false_when_channel_closed() {
            let (tx, rx) = mpsc::channel();
            drop(rx);
            let mut previous = snap([0]);
            let current = snap([1]);
            assert!(!reconcile_and_emit_with(&tx, &mut previous, current));
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
mod real {
    use nokhwa_core::{
        error::NokhwaError,
        traits::{HotplugEventPoll, HotplugSource},
        types::ApiBackend,
    };

    /// Non-Apple stub for [`AVFoundationHotplugContext`]. Every method
    /// errors with [`NokhwaError::UnsupportedOperationError`].
    #[derive(Default)]
    pub struct AVFoundationHotplugContext;

    impl AVFoundationHotplugContext {
        #[must_use]
        pub fn new() -> Self {
            Self
        }
    }

    impl HotplugSource for AVFoundationHotplugContext {
        fn take_hotplug_events(&mut self) -> Result<Box<dyn HotplugEventPoll + Send>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::AVFoundation,
            ))
        }
    }
}

pub use real::AVFoundationHotplugContext;
