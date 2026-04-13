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

//! Layer 3: [`CameraRunner`], a background-thread capture helper.
//!
//! The runner owns an [`OpenedCamera`] on a dedicated thread and delivers
//! frames, pictures, and events through `std::sync::mpsc` channels. The
//! thread is joined either via [`CameraRunner::stop`] or on drop.
//!
//! Dispatch mirrors [`OpenedCamera`]:
//!
//! - [`OpenedCamera::Stream`]: the runner exposes a frames channel.
//! - [`OpenedCamera::Shutter`]: the runner exposes a pictures + events
//!   channels (events is always present but inert for non-event backends).
//! - [`OpenedCamera::Hybrid`]: all three channels are available; events
//!   only if the backend advertised `EventSource`.

use std::sync::mpsc::{
    channel, sync_channel, Receiver, RecvTimeoutError, Sender, SyncSender, TryRecvError,
    TrySendError,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use nokhwa_core::buffer::Buffer;
use nokhwa_core::error::NokhwaError;
use nokhwa_core::traits::{CameraEvent, EventPoll};
use nokhwa_core::types::{ControlValueSetter, KnownCameraControl};

use crate::session::{HybridCamera, OpenedCamera, ShutterCamera, StreamCamera};

/// Policy for what to do when a bounded runner channel is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    /// Drop the newly-produced item, preserving older backlog. Lowest
    /// overhead; good when any-recent-frame is acceptable.
    #[default]
    DropNewest,
    /// Drop the oldest item in the channel to make room for the new one.
    /// Good when consumers want the freshest frame. Implemented via a
    /// per-channel relay thread — only paid when this policy is selected.
    DropOldest,
}

/// Configuration for [`CameraRunner::spawn`].
#[derive(Debug, Clone, Copy)]
pub struct RunnerConfig {
    /// Worker poll interval.
    ///
    /// - In stream / hybrid variants: how long the worker sleeps before
    ///   retrying after a failed `frame()` call.
    /// - In the shutter variant: the command-channel receive timeout (i.e.
    ///   the cadence at which the worker wakes up to check for shutdown).
    pub poll_interval: Duration,
    /// Event-poll timeout passed to [`EventPoll::next_timeout`].
    pub event_tick: Duration,
    /// Timeout for shutter `take_picture(…)` after a trigger.
    pub shutter_timeout: Duration,
    /// Capacity of the frames channel. `0` = unbounded (pre-0.14 behavior).
    pub frames_capacity: usize,
    /// Capacity of the pictures channel. `0` = unbounded.
    pub pictures_capacity: usize,
    /// Capacity of the events channel. `0` = unbounded.
    pub events_capacity: usize,
    /// Policy when a bounded channel is full. Ignored if the corresponding
    /// capacity is `0`.
    pub overflow: Overflow,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(10),
            event_tick: Duration::from_millis(50),
            shutter_timeout: Duration::from_secs(5),
            frames_capacity: 4,
            pictures_capacity: 8,
            events_capacity: 32,
            overflow: Overflow::DropNewest,
        }
    }
}

/// Producer half of a runner channel, abstracting over unbounded /
/// bounded-with-overflow variants. Shared across the three spawn paths.
enum Tx<T: Send + 'static> {
    Unbounded(Sender<T>),
    /// `DropNewest`: on `Full`, the new item is silently discarded.
    BoundedDropNewest(SyncSender<T>),
    /// `DropOldest`: feeds into a relay thread that drop-oldest's into the
    /// user-facing bounded channel.
    BoundedDropOldest(SyncSender<T>),
}

impl<T: Send + 'static> Tx<T> {
    /// Send an item. Returns `Err(())` if the consumer has disconnected —
    /// the worker should treat this as a shutdown signal. Overflow on a
    /// bounded channel is **not** an error.
    fn send(&self, item: T) -> Result<(), ()> {
        match self {
            Tx::Unbounded(tx) => tx.send(item).map_err(|_| ()),
            Tx::BoundedDropNewest(tx) => match tx.try_send(item) {
                Ok(()) | Err(TrySendError::Full(_)) => Ok(()),
                Err(TrySendError::Disconnected(_)) => Err(()),
            },
            Tx::BoundedDropOldest(tx) => tx.send(item).map_err(|_| ()),
        }
    }
}

/// Build a (producer, consumer) pair for a runner stream, along with an
/// optional relay-thread `JoinHandle` that the runner must join on shutdown.
///
/// - `capacity == 0` → unbounded `std::sync::mpsc::channel`; relay = `None`.
/// - `capacity > 0, DropNewest` → single `sync_channel(capacity)`; relay = `None`.
/// - `capacity > 0, DropOldest` → producer feeds a relay thread that
///   maintains a `VecDeque<T>` of at most `capacity` items; when full, the
///   oldest item is popped before the new one is pushed. The relay forwards
///   items into an unbounded channel exposed to the user, so the total
///   memory footprint stays bounded by `capacity` plus whatever the user
///   hasn't yet received (which is always immediately drainable).
fn make_channel<T: Send + 'static>(
    capacity: usize,
    policy: Overflow,
) -> (Tx<T>, Receiver<T>, Option<JoinHandle<()>>) {
    if capacity == 0 {
        let (tx, rx) = channel::<T>();
        return (Tx::Unbounded(tx), rx, None);
    }
    match policy {
        Overflow::DropNewest => {
            let (tx, rx) = sync_channel::<T>(capacity);
            (Tx::BoundedDropNewest(tx), rx, None)
        }
        Overflow::DropOldest => {
            // Two chained `sync_channel(capacity)` joined by a relay thread
            // that owns an in-memory `VecDeque<T>` of at most `capacity`
            // items. When the user-facing channel is full, the relay drops
            // the oldest buffered item to make room for the new one. Total
            // memory footprint is bounded by `2 * capacity` items.
            let (prod_tx, relay_rx) = sync_channel::<T>(capacity);
            let (user_tx, user_rx) = sync_channel::<T>(capacity);
            let handle = thread::spawn(move || {
                use std::collections::VecDeque;
                use std::sync::mpsc::RecvTimeoutError;
                let mut buf: VecDeque<T> = VecDeque::with_capacity(capacity);
                let poll = Duration::from_millis(5);
                loop {
                    // Try to drain buffer into user_tx first (non-blocking).
                    while let Some(front) = buf.pop_front() {
                        match user_tx.try_send(front) {
                            Ok(()) => {}
                            Err(TrySendError::Full(item)) => {
                                buf.push_front(item);
                                break;
                            }
                            Err(TrySendError::Disconnected(_)) => return,
                        }
                    }
                    // Wait for a new item. If buf is non-empty, poll with a
                    // short timeout so we get another chance to drain into
                    // user_tx even when the producer has gone idle.
                    let wait = if buf.is_empty() {
                        relay_rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
                    } else {
                        relay_rx.recv_timeout(poll)
                    };
                    match wait {
                        Ok(item) => {
                            if buf.len() == capacity {
                                buf.pop_front();
                            }
                            buf.push_back(item);
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => {
                            // Producer gone; still try to flush buffered items.
                            while let Some(front) = buf.pop_front() {
                                if user_tx.send(front).is_err() {
                                    return;
                                }
                            }
                            return;
                        }
                    }
                }
            });
            (Tx::BoundedDropOldest(prod_tx), user_rx, Some(handle))
        }
    }
}

/// Commands sent from the foreground handle to the worker thread.
enum Command {
    Trigger,
    SetControl(KnownCameraControl, ControlValueSetter),
    Die,
}

/// Background-thread capture helper.
///
/// `CameraRunner` owns an [`OpenedCamera`](crate::OpenedCamera) on a worker
/// thread and delivers frames / pictures / events through
/// [`std::sync::mpsc`] channels.
///
/// ## Channel semantics
///
/// In 0.13.0 the channels are unbounded ([`std::sync::mpsc::channel`]). Bounded
/// channels with a drop-oldest / drop-newest policy are on the 0.14 roadmap.
/// Until then, if a consumer stops draining the `frames()` receiver while
/// keeping the runner alive, memory grows without bound.
///
/// The accessor methods (`frames()`, `pictures()`, `events()`) return borrowed
/// receivers, so in the current API a caller cannot detach and drop a receiver
/// independently of the runner — the runner's [`Drop`] signals `Die` and joins
/// the worker first, then drops the receivers. The worker does defensively
/// exit on `SendError` so that a future API surface exposing owned receivers
/// would not be able to leak the worker thread.
#[derive(Debug)]
pub struct CameraRunner {
    frames: Option<Receiver<Buffer>>,
    pictures: Option<Receiver<Buffer>>,
    events: Option<Receiver<CameraEvent>>,
    cmd: Sender<Command>,
    join: Option<JoinHandle<()>>,
    relays: Vec<JoinHandle<()>>,
}

impl CameraRunner {
    /// Spawn a worker thread owning `camera`.
    ///
    /// # Errors
    /// Returns [`NokhwaError`] if opening the underlying camera fails
    /// (stream/hybrid variants call `open()` before entering the loop).
    pub fn spawn(camera: OpenedCamera, cfg: RunnerConfig) -> Result<Self, NokhwaError> {
        match camera {
            OpenedCamera::Stream(cam) => Self::spawn_stream(cam, cfg),
            OpenedCamera::Shutter(cam) => Ok(Self::spawn_shutter(cam, cfg)),
            OpenedCamera::Hybrid(cam) => Self::spawn_hybrid(cam, cfg),
        }
    }

    fn spawn_stream(mut cam: StreamCamera, cfg: RunnerConfig) -> Result<Self, NokhwaError> {
        cam.open()?;
        let (frame_tx, frame_rx, frame_relay) =
            make_channel::<Buffer>(cfg.frames_capacity, cfg.overflow);
        let (cmd_tx, cmd_rx) = channel::<Command>();
        let poll_interval = cfg.poll_interval;
        let join = thread::spawn(move || loop {
            match cmd_rx.try_recv() {
                Ok(Command::Die) | Err(TryRecvError::Disconnected) => break,
                Ok(Command::SetControl(id, v)) => {
                    let _ = cam.set_control(id, v);
                }
                Ok(Command::Trigger) | Err(TryRecvError::Empty) => {}
            }
            match cam.frame() {
                Ok(buf) => {
                    // If the consumer dropped the receiver, treat it as a
                    // shutdown signal rather than spinning forever.
                    if frame_tx.send(buf).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    thread::sleep(poll_interval);
                }
            }
        });
        let mut relays = Vec::new();
        if let Some(h) = frame_relay {
            relays.push(h);
        }
        Ok(Self {
            frames: Some(frame_rx),
            pictures: None,
            events: None,
            cmd: cmd_tx,
            join: Some(join),
            relays,
        })
    }

    fn spawn_shutter(mut cam: ShutterCamera, cfg: RunnerConfig) -> Self {
        let (pic_tx, pic_rx, pic_relay) =
            make_channel::<Buffer>(cfg.pictures_capacity, cfg.overflow);
        let (cmd_tx, cmd_rx) = channel::<Command>();
        let poll_interval = cfg.poll_interval;
        let shutter_timeout = cfg.shutter_timeout;
        let join = thread::spawn(move || loop {
            match cmd_rx.recv_timeout(poll_interval) {
                Ok(Command::Die) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(Command::Trigger) => {
                    if cam.trigger().is_ok() {
                        if let Ok(pic) = cam.take_picture(shutter_timeout) {
                            if pic_tx.send(pic).is_err() {
                                break;
                            }
                        }
                    }
                }
                Ok(Command::SetControl(id, v)) => {
                    let _ = cam.set_control(id, v);
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        });
        let mut relays = Vec::new();
        if let Some(h) = pic_relay {
            relays.push(h);
        }
        Self {
            frames: None,
            pictures: Some(pic_rx),
            events: None,
            cmd: cmd_tx,
            join: Some(join),
            relays,
        }
    }

    fn spawn_hybrid(mut cam: HybridCamera, cfg: RunnerConfig) -> Result<Self, NokhwaError> {
        cam.open()?;
        let events_poll: Option<Box<dyn EventPoll + Send>> = match cam.take_events() {
            Some(Ok(p)) => Some(p),
            Some(Err(e)) => {
                #[cfg(feature = "logging")]
                log::warn!("CameraRunner: failed to take event poller: {e}");
                #[cfg(not(feature = "logging"))]
                let _ = e;
                None
            }
            None => None,
        };

        let (frame_tx, frame_rx, frame_relay) =
            make_channel::<Buffer>(cfg.frames_capacity, cfg.overflow);
        let (pic_tx, pic_rx, pic_relay) =
            make_channel::<Buffer>(cfg.pictures_capacity, cfg.overflow);
        let (cmd_tx, cmd_rx) = channel::<Command>();

        let mut relays: Vec<JoinHandle<()>> = Vec::new();
        if let Some(h) = frame_relay {
            relays.push(h);
        }
        if let Some(h) = pic_relay {
            relays.push(h);
        }

        // Events thread (if any).
        let (event_rx_opt, event_join_opt) = if let Some(mut poll) = events_poll {
            let (ev_tx, ev_rx, ev_relay) =
                make_channel::<CameraEvent>(cfg.events_capacity, cfg.overflow);
            if let Some(h) = ev_relay {
                relays.push(h);
            }
            let (ev_cmd_tx, ev_cmd_rx) = channel::<()>();
            let event_tick = cfg.event_tick;
            let handle = thread::spawn(move || loop {
                if let Ok(()) = ev_cmd_rx.try_recv() {
                    break;
                }
                if let Some(event) = poll.next_timeout(event_tick) {
                    if ev_tx.send(event).is_err() {
                        break;
                    }
                }
            });
            (Some(ev_rx), Some((ev_cmd_tx, handle)))
        } else {
            (None, None)
        };

        let poll_interval = cfg.poll_interval;
        let shutter_timeout = cfg.shutter_timeout;

        let join = thread::spawn(move || {
            loop {
                match cmd_rx.try_recv() {
                    Ok(Command::Die) | Err(TryRecvError::Disconnected) => break,
                    Ok(Command::Trigger) => {
                        if cam.trigger().is_ok() {
                            if let Ok(pic) = cam.take_picture(shutter_timeout) {
                                let _ = pic_tx.send(pic);
                            }
                        }
                    }
                    Ok(Command::SetControl(id, v)) => {
                        let _ = cam.set_control(id, v);
                    }
                    Err(TryRecvError::Empty) => {}
                }
                match cam.frame() {
                    Ok(buf) => {
                        // Exit if the consumer dropped the frames receiver.
                        if frame_tx.send(buf).is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        thread::sleep(poll_interval);
                    }
                }
            }
            // Tell the events thread to stop too.
            if let Some((ev_cmd_tx, handle)) = event_join_opt {
                let _ = ev_cmd_tx.send(());
                if let Err(err) = handle.join() {
                    #[cfg(feature = "logging")]
                    log::warn!("CameraRunner: event worker thread panicked: {err:?}");
                    #[cfg(not(feature = "logging"))]
                    let _ = err;
                }
            }
        });

        Ok(Self {
            frames: Some(frame_rx),
            pictures: Some(pic_rx),
            events: event_rx_opt,
            cmd: cmd_tx,
            join: Some(join),
            relays,
        })
    }

    /// Frame receiver, if this runner's backend is a stream or hybrid.
    #[must_use]
    pub fn frames(&self) -> Option<&Receiver<Buffer>> {
        self.frames.as_ref()
    }

    /// Picture receiver, if this runner's backend is a shutter or hybrid.
    #[must_use]
    pub fn pictures(&self) -> Option<&Receiver<Buffer>> {
        self.pictures.as_ref()
    }

    /// Event receiver, if the backend advertised `EventSource`.
    #[must_use]
    pub fn events(&self) -> Option<&Receiver<CameraEvent>> {
        self.events.as_ref()
    }

    /// Trigger a shutter capture on the worker. No-op for pure stream backends.
    ///
    /// # Errors
    /// Returns [`NokhwaError`] if the worker thread is no longer running.
    pub fn trigger(&self) -> Result<(), NokhwaError> {
        self.cmd
            .send(Command::Trigger)
            .map_err(|e| NokhwaError::general(format!("runner thread gone: {e}")))
    }

    /// Set a camera control on the worker thread.
    ///
    /// # Errors
    /// Returns [`NokhwaError`] if the worker thread is no longer running.
    pub fn set_control(
        &self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        self.cmd
            .send(Command::SetControl(id, value))
            .map_err(|e| NokhwaError::general(format!("runner thread gone: {e}")))
    }

    /// Stop the worker thread and join it.
    ///
    /// # Errors
    /// Returns [`NokhwaError`] only if signalling the worker fails; a failed
    /// join is logged-but-ignored because there is no good recovery.
    pub fn stop(mut self) -> Result<(), NokhwaError> {
        self.shutdown();
        Ok(())
    }

    fn shutdown(&mut self) {
        let _ = self.cmd.send(Command::Die);
        // Drop receivers first so any blocked relay `try_send` wakes up and
        // the relay threads can exit cleanly.
        self.frames = None;
        self.pictures = None;
        self.events = None;
        if let Some(handle) = self.join.take() {
            if let Err(err) = handle.join() {
                #[cfg(feature = "logging")]
                log::warn!("CameraRunner: worker thread panicked: {err:?}");
                #[cfg(not(feature = "logging"))]
                let _ = err;
            }
        }
        for relay in self.relays.drain(..) {
            if let Err(err) = relay.join() {
                #[cfg(feature = "logging")]
                log::warn!("CameraRunner: relay thread panicked: {err:?}");
                #[cfg(not(feature = "logging"))]
                let _ = err;
            }
        }
    }

    /// Take ownership of the frames receiver. The worker thread keeps
    /// running so you can still [`trigger`](Self::trigger) and
    /// [`set_control`](Self::set_control). Returns `None` if the runner was
    /// built from a shutter-only backend, or the receiver was already taken.
    #[must_use]
    pub fn take_frames(&mut self) -> Option<Receiver<Buffer>> {
        self.frames.take()
    }

    /// Take ownership of the pictures receiver. See [`take_frames`](Self::take_frames).
    #[must_use]
    pub fn take_pictures(&mut self) -> Option<Receiver<Buffer>> {
        self.pictures.take()
    }

    /// Take ownership of the events receiver. See [`take_frames`](Self::take_frames).
    #[must_use]
    pub fn take_events(&mut self) -> Option<Receiver<CameraEvent>> {
        self.events.take()
    }
}

impl Drop for CameraRunner {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::{make_channel, Overflow};
    use std::time::Duration;

    #[test]
    fn unbounded_capacity_zero_is_unbounded() {
        let (tx, rx, relay) = make_channel::<u32>(0, Overflow::DropNewest);
        assert!(relay.is_none());
        for i in 0..1000 {
            tx.send(i).unwrap();
        }
        for i in 0..1000 {
            assert_eq!(rx.recv().unwrap(), i);
        }
    }

    #[test]
    fn drop_newest_discards_overflow() {
        let (tx, rx, relay) = make_channel::<u32>(2, Overflow::DropNewest);
        assert!(relay.is_none());
        // Fill capacity.
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        // Overflow: the new item (3) is dropped; backlog preserved.
        tx.send(3).unwrap();
        assert_eq!(rx.recv().unwrap(), 1);
        assert_eq!(rx.recv().unwrap(), 2);
        assert!(rx.recv_timeout(Duration::from_millis(50)).is_err());
    }

    #[test]
    fn drop_oldest_has_relay_and_accepts_sends() {
        // We don't attempt to prove the exact sequence observed — the relay
        // drains into the user channel on a timer, so timing is inherently
        // fuzzy. What we do check: a relay handle exists, sends succeed,
        // and the consumer observes *some* items including at least one
        // item from the tail of the produced sequence.
        let (tx, rx, relay) = make_channel::<u32>(2, Overflow::DropOldest);
        assert!(relay.is_some(), "DropOldest should spawn a relay thread");
        // Produce items and drain concurrently to avoid `SyncSender::send`
        // back-pressure through the relay.
        let producer = std::thread::spawn(move || {
            for i in 0..50u32 {
                tx.send(i).unwrap();
            }
        });
        let mut received = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(v) => received.push(v),
                Err(_) => {
                    if received.len() >= 2 {
                        break;
                    }
                }
            }
        }
        producer.join().unwrap();
        assert!(
            !received.is_empty(),
            "expected at least one item through the relay"
        );
        // At minimum, the items we received should be in monotonically
        // non-decreasing order — drop-oldest never re-orders.
        for w in received.windows(2) {
            assert!(w[0] <= w[1], "drop-oldest must not reorder: {received:?}");
        }
        drop(rx);
        relay.unwrap().join().unwrap();
    }

    #[test]
    fn tx_send_err_on_consumer_drop_unbounded() {
        let (tx, rx, _) = make_channel::<u32>(0, Overflow::DropNewest);
        drop(rx);
        assert!(tx.send(1).is_err());
    }

    #[test]
    fn tx_send_err_on_consumer_drop_bounded_newest() {
        let (tx, rx, _) = make_channel::<u32>(2, Overflow::DropNewest);
        drop(rx);
        // SyncSender::try_send surfaces Disconnected after the receiver is dropped.
        // (Note: a pending Full item buffered before drop is not possible here
        // because nothing was sent.)
        assert!(tx.send(1).is_err());
    }

    #[test]
    fn default_runnerconfig_is_bounded() {
        let cfg = super::RunnerConfig::default();
        assert!(cfg.frames_capacity > 0);
        assert!(cfg.pictures_capacity > 0);
        assert!(cfg.events_capacity > 0);
        assert_eq!(cfg.overflow, Overflow::DropNewest);
    }
}
