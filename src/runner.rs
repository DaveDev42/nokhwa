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
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(10),
            event_tick: Duration::from_millis(50),
            shutter_timeout: Duration::from_secs(5),
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
        let poll_interval = cfg.poll_interval;
        let shutter_timeout = cfg.shutter_timeout;
        let join = thread::spawn(move || loop {
            match cmd_rx.recv_timeout(poll_interval) {
                Ok(Command::Die) | Err(RecvTimeoutError::Disconnected) => break,
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
            Some(Err(e)) => {
                #[cfg(feature = "logging")]
                log::warn!("CameraRunner: failed to take event poller: {e}");
                #[cfg(not(feature = "logging"))]
                let _ = e;
                None
            }
            None => None,
        };

        let (frame_tx, frame_rx) = channel::<Buffer>();
        let (pic_tx, pic_rx) = channel::<Buffer>();
        let (cmd_tx, cmd_rx) = channel::<Command>();

        // Events thread (if any).
        let (event_rx_opt, event_join_opt) = if let Some(mut poll) = events_poll {
            let (ev_tx, ev_rx) = channel::<CameraEvent>();
            let (ev_cmd_tx, ev_cmd_rx) = channel::<()>();
            let event_tick = cfg.event_tick;
            let handle = thread::spawn(move || loop {
                if let Ok(()) = ev_cmd_rx.try_recv() {
                    break;
                }
                if let Some(event) = poll.next_timeout(event_tick) {
                    let _ = ev_tx.send(event);
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
            if let Err(err) = handle.join() {
                #[cfg(feature = "logging")]
                log::warn!("CameraRunner: worker thread panicked: {err:?}");
                #[cfg(not(feature = "logging"))]
                let _ = err;
            }
        }
    }
}

impl Drop for CameraRunner {
    fn drop(&mut self) {
        self.shutdown();
    }
}
