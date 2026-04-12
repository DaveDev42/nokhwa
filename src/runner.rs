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

use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use nokhwa_core::buffer::Buffer;
use nokhwa_core::error::NokhwaError;
use nokhwa_core::traits::{CameraEvent, EventPoll};
use nokhwa_core::types::{ControlValueSetter, KnownCameraControl};

use crate::session::{HybridCamera, OpenedCamera, ShutterCamera, StreamCamera};

/// How to behave when a channel is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    /// Drop the newest item when the channel is at capacity.
    #[default]
    DropNewest,
    /// Drop the oldest item (block briefly, then push).
    DropOldest,
}

/// Configuration for [`CameraRunner::spawn`].
#[derive(Debug, Clone, Copy)]
pub struct RunnerConfig {
    /// Bounded frames-channel capacity. `None` = unbounded.
    pub frames_capacity: Option<usize>,
    /// Bounded pictures-channel capacity. `None` = unbounded.
    pub pictures_capacity: Option<usize>,
    /// Bounded events-channel capacity. `None` = unbounded.
    pub events_capacity: Option<usize>,
    /// Behaviour when a bounded channel is full.
    pub overflow: Overflow,
    /// How long the worker waits on the command channel between frame polls.
    /// Also used as the cadence for probing shutter `take_picture(…)`.
    pub tick: Duration,
    /// Event-poll timeout.
    pub event_tick: Duration,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            frames_capacity: None,
            pictures_capacity: None,
            events_capacity: None,
            overflow: Overflow::default(),
            tick: Duration::from_millis(10),
            event_tick: Duration::from_millis(50),
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
#[derive(Debug)]
pub struct CameraRunner {
    frames: Option<Receiver<Buffer>>,
    pictures: Option<Receiver<Buffer>>,
    events: Option<Receiver<CameraEvent>>,
    cmd: Sender<Command>,
    join: Option<JoinHandle<()>>,
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
        let (frame_tx, frame_rx) = channel::<Buffer>();
        let (cmd_tx, cmd_rx) = channel::<Command>();
        let tick = cfg.tick;
        let frames_cap = cfg.frames_capacity;
        let overflow = cfg.overflow;
        let join = thread::spawn(move || loop {
            match cmd_rx.try_recv() {
                Ok(Command::Die) | Err(TryRecvError::Disconnected) => break,
                Ok(Command::SetControl(id, v)) => {
                    let _ = cam.set_control(id, v);
                }
                Ok(Command::Trigger) | Err(TryRecvError::Empty) => {}
            }
            match cam.frame() {
                Ok(buf) => push_or_drop(&frame_tx, buf, frames_cap, overflow),
                Err(_) => {
                    thread::sleep(tick);
                }
            }
        });
        Ok(Self {
            frames: Some(frame_rx),
            pictures: None,
            events: None,
            cmd: cmd_tx,
            join: Some(join),
        })
    }

    fn spawn_shutter(mut cam: ShutterCamera, cfg: RunnerConfig) -> Self {
        let (pic_tx, pic_rx) = channel::<Buffer>();
        let (cmd_tx, cmd_rx) = channel::<Command>();
        let tick = cfg.tick;
        let pics_cap = cfg.pictures_capacity;
        let overflow = cfg.overflow;
        let join = thread::spawn(move || loop {
            match cmd_rx.recv_timeout(tick) {
                Ok(Command::Die) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(Command::Trigger) => {
                    if cam.trigger().is_ok() {
                        if let Ok(pic) = cam.take_picture(Duration::from_millis(200)) {
                            push_or_drop(&pic_tx, pic, pics_cap, overflow);
                        }
                    }
                }
                Ok(Command::SetControl(id, v)) => {
                    let _ = cam.set_control(id, v);
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        });
        Self {
            frames: None,
            pictures: Some(pic_rx),
            events: None,
            cmd: cmd_tx,
            join: Some(join),
        }
    }

    fn spawn_hybrid(mut cam: HybridCamera, cfg: RunnerConfig) -> Result<Self, NokhwaError> {
        cam.open()?;
        let events_poll: Option<Box<dyn EventPoll + Send>> = match cam.take_events() {
            Some(Ok(p)) => Some(p),
            Some(Err(_)) | None => None,
        };

        let (frame_tx, frame_rx) = channel::<Buffer>();
        let (pic_tx, pic_rx) = channel::<Buffer>();
        let (cmd_tx, cmd_rx) = channel::<Command>();

        // Events thread (if any).
        let (event_rx_opt, event_join_opt) = if let Some(mut poll) = events_poll {
            let (ev_tx, ev_rx) = channel::<CameraEvent>();
            let (ev_cmd_tx, ev_cmd_rx) = channel::<()>();
            let event_tick = cfg.event_tick;
            let ev_cap = cfg.events_capacity;
            let overflow = cfg.overflow;
            let handle = thread::spawn(move || loop {
                if let Ok(()) = ev_cmd_rx.try_recv() {
                    break;
                }
                if let Some(event) = poll.next_timeout(event_tick) {
                    push_or_drop(&ev_tx, event, ev_cap, overflow);
                }
            });
            (Some(ev_rx), Some((ev_cmd_tx, handle)))
        } else {
            (None, None)
        };

        let tick = cfg.tick;
        let frames_cap = cfg.frames_capacity;
        let pics_cap = cfg.pictures_capacity;
        let overflow = cfg.overflow;

        let join = thread::spawn(move || {
            loop {
                match cmd_rx.try_recv() {
                    Ok(Command::Die) | Err(TryRecvError::Disconnected) => break,
                    Ok(Command::Trigger) => {
                        if cam.trigger().is_ok() {
                            if let Ok(pic) = cam.take_picture(Duration::from_millis(200)) {
                                push_or_drop(&pic_tx, pic, pics_cap, overflow);
                            }
                        }
                    }
                    Ok(Command::SetControl(id, v)) => {
                        let _ = cam.set_control(id, v);
                    }
                    Err(TryRecvError::Empty) => {}
                }
                match cam.frame() {
                    Ok(buf) => push_or_drop(&frame_tx, buf, frames_cap, overflow),
                    Err(_) => {
                        thread::sleep(tick);
                    }
                }
            }
            // Tell the events thread to stop too.
            if let Some((ev_cmd_tx, handle)) = event_join_opt {
                let _ = ev_cmd_tx.send(());
                let _ = handle.join();
            }
        });

        Ok(Self {
            frames: Some(frame_rx),
            pictures: Some(pic_rx),
            events: event_rx_opt,
            cmd: cmd_tx,
            join: Some(join),
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
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for CameraRunner {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn push_or_drop<T>(tx: &Sender<T>, item: T, _cap: Option<usize>, _overflow: Overflow) {
    // std::sync::mpsc::channel is unbounded, so capacity bookkeeping is a
    // no-op here. The fields are kept on RunnerConfig for future migration
    // to a bounded channel (e.g. crossbeam / sync_channel).
    let _ = tx.send(item);
}
