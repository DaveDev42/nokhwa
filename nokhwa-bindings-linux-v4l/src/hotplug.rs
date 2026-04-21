//! Backend-level hotplug for the `Video4Linux` backend.
//!
//! Implements [`HotplugSource`] by **polling** `query()` on a dedicated
//! thread every [`POLL_INTERVAL`] and diffing successive snapshots on
//! the v4l node index.
//!
//! Polling parallels the `MediaFoundation` implementation in
//! `nokhwa-bindings-windows-msmf::hotplug`: `inotify` on `/dev/` would
//! be event-driven but requires a non-trivial read loop + shutdown
//! plumbing, and hotplug latency budgets are seconds, not milliseconds.
//! The poll loop is ten lines of logic and never misses an event
//! because `query()` reads the *current* device set.

#[cfg(target_os = "linux")]
mod real {
    use crate::internal::query;
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

    /// How often the polling thread re-enumerates `Video4Linux`
    /// devices. 500ms matches the MSMF backend and is a compromise
    /// between "noticeable latency" and "burn CPU churning through
    /// `/dev/video*`". Tune here if users report perceivable lag on
    /// hot-unplug.
    const POLL_INTERVAL: Duration = Duration::from_millis(500);

    /// Backend-level hotplug source for `Video4Linux`. Cheap to
    /// construct — the polling thread is only spawned when
    /// [`take_hotplug_events`](HotplugSource::take_hotplug_events) is
    /// called, and is joined when the returned poller is dropped.
    #[derive(Default)]
    pub struct V4LHotplugContext {
        taken: bool,
    }

    impl V4LHotplugContext {
        #[must_use]
        pub fn new() -> Self {
            Self { taken: false }
        }
    }

    impl HotplugSource for V4LHotplugContext {
        fn take_hotplug_events(&mut self) -> Result<Box<dyn HotplugEventPoll + Send>, NokhwaError> {
            if self.taken {
                return Err(NokhwaError::UnsupportedOperationError(
                    ApiBackend::Video4Linux,
                ));
            }
            self.taken = true;
            Ok(Box::new(V4LHotplugPoll::spawn()?))
        }
    }

    /// Concrete [`HotplugEventPoll`]. Owns a background thread running
    /// the poll loop + an mpsc channel + an [`AtomicBool`] shutdown
    /// flag. Dropping the poll flips the flag and joins the thread.
    struct V4LHotplugPoll {
        rx: Receiver<HotplugEvent>,
        stop: Arc<AtomicBool>,
        join: Option<JoinHandle<()>>,
    }

    impl V4LHotplugPoll {
        fn spawn() -> Result<Self, NokhwaError> {
            let (tx, rx) = mpsc::channel();
            let stop = Arc::new(AtomicBool::new(false));
            let stop_thread = Arc::clone(&stop);
            let join = thread::Builder::new()
                .name("nokhwa-v4l-hotplug".to_string())
                .spawn(move || poll_loop(&tx, &stop_thread))
                .map_err(|e| NokhwaError::general(format!("spawn hotplug thread: {e}")))?;
            Ok(Self {
                rx,
                stop,
                join: Some(join),
            })
        }
    }

    impl HotplugEventPoll for V4LHotplugPoll {
        fn try_next(&mut self) -> Option<HotplugEvent> {
            self.rx.try_recv().ok()
        }
        fn next_timeout(&mut self, d: Duration) -> Option<HotplugEvent> {
            self.rx.recv_timeout(d).ok()
        }
    }

    impl Drop for V4LHotplugPoll {
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
            let current = snapshot();

            // Emit arrivals before removals so a rapid re-plug can be
            // observed as Disconnected → Connected on the consumer
            // side even if both changes land in one poll window.
            for (key, info) in &current {
                if !previous.contains_key(key)
                    && tx.send(HotplugEvent::Connected(info.clone())).is_err()
                {
                    return; // consumer dropped the poller
                }
            }
            for (key, info) in &previous {
                if !current.contains_key(key)
                    && tx.send(HotplugEvent::Disconnected(info.clone())).is_err()
                {
                    return;
                }
            }
            previous = current;
        }
    }

    /// One `Video4Linux` enumeration pass, indexed by
    /// [`CameraIndex::to_string`]. The v4l node index is the stable
    /// identifier surfaced by `enum_devices` and maps 1:1 to
    /// `/dev/videoN`. Re-plugging a device can reuse the same index —
    /// consumers will then see `Disconnected(N)` followed by
    /// `Connected(N)` which is the right semantic.
    ///
    /// Errors from `query()` are swallowed — a transient enumeration
    /// failure should not tear down the hotplug thread. An empty
    /// snapshot will look like "every device disappeared"; next tick
    /// we will re-emit them as `Connected`. That is noisy but not
    /// incorrect.
    fn snapshot() -> BTreeMap<String, CameraInfo> {
        match query() {
            Ok(list) => list
                .into_iter()
                .map(|ci| (ci.index().to_string(), ci))
                .collect(),
            Err(_) => BTreeMap::new(),
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod real {
    use nokhwa_core::{
        error::NokhwaError,
        traits::{HotplugEventPoll, HotplugSource},
        types::ApiBackend,
    };

    /// Non-Linux stub for [`V4LHotplugContext`]. Every method errors
    /// with [`NokhwaError::UnsupportedOperationError`].
    #[derive(Default)]
    pub struct V4LHotplugContext;

    impl V4LHotplugContext {
        #[must_use]
        pub fn new() -> Self {
            Self
        }
    }

    impl HotplugSource for V4LHotplugContext {
        fn take_hotplug_events(&mut self) -> Result<Box<dyn HotplugEventPoll + Send>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::Video4Linux,
            ))
        }
    }
}

pub use real::V4LHotplugContext;
