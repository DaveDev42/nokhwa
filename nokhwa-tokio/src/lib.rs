#![deny(clippy::pedantic)]
#![warn(clippy::all)]
#![allow(clippy::module_name_repetitions)]

//! Tokio integration for [`nokhwa`].
//!
//! Exposes [`TokioCameraRunner`], an async wrapper around the sync
//! [`nokhwa::CameraRunner`]. The wrapper owns the sync runner, spawns one
//! `spawn_blocking` forwarder per active channel, and exposes
//! `tokio::sync::mpsc::Receiver`s for `.recv().await`.
//!
//! # Drop semantics
//!
//! Dropping a [`TokioCameraRunner`] inside a tokio runtime returns
//! immediately; the underlying worker thread is joined on a
//! `spawn_blocking` task so the async executor is not blocked. Outside a
//! runtime, drop joins synchronously. For explicit shutdown, use
//! [`TokioCameraRunner::stop`]`.await`.
//!
//! # Why `forwarders.abort()` is advisory
//!
//! Forwarder tasks are created via [`tokio::task::spawn_blocking`], which
//! runs on a dedicated blocking-thread pool. `spawn_blocking` tasks are
//! **not cancellable** — `JoinHandle::abort` marks them as aborted but
//! the OS thread keeps running until the current call (e.g.
//! `sync_rx.recv()`) returns. In practice that happens almost
//! immediately: [`TokioCameraRunner`] drops the sync `Receiver`s before
//! tearing down the inner runner, which causes the runner's senders to
//! disconnect, which unblocks the forwarder's `sync_rx.recv()` with
//! `Err`, and the forwarder exits. The `abort()` call is kept as a
//! belt-and-suspenders signal to the tokio scheduler.
//!
//! # Tokio features
//!
//! This crate depends on tokio with only `sync` and `rt` — the minimal set
//! needed for `mpsc` and `spawn_blocking` / `Handle::try_current`.

use nokhwa::{CameraRunner, RunnerConfig};
use nokhwa_core::buffer::Buffer;
use nokhwa_core::error::NokhwaError;
use nokhwa_core::traits::CameraEvent;
use nokhwa_core::types::{ControlValueSetter, KnownCameraControl};

use nokhwa::OpenedCamera;
use std::fmt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Capacity of each forwarder's tokio-side channel.
///
/// This is separate from the sync-side [`RunnerConfig`] capacities: the
/// forwarder bridges a `std::sync::mpsc::Receiver` to a
/// `tokio::sync::mpsc::Sender`, and the tokio channel acts as a small
/// buffer to decouple the blocking-thread from the async consumer.
///
/// `32` is large enough to absorb brief async scheduling jitter without
/// starving the forwarder, small enough that back-pressure still reaches
/// the sync-side bounded channel. Not currently configurable; file an
/// issue if you need to tune it.
const FORWARDER_CAPACITY: usize = 32;

/// Async wrapper around [`CameraRunner`].
///
/// Build one with [`TokioCameraRunner::spawn`]; drain frames with
/// `frames_mut().map(|rx| rx.recv().await)` and friends.
pub struct TokioCameraRunner {
    frames: Option<mpsc::Receiver<Buffer>>,
    pictures: Option<mpsc::Receiver<Buffer>>,
    events: Option<mpsc::Receiver<CameraEvent>>,
    inner: Option<CameraRunner>,
    forwarders: Vec<JoinHandle<()>>,
}

impl fmt::Debug for TokioCameraRunner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokioCameraRunner")
            .field("frames", &self.frames.is_some())
            .field("pictures", &self.pictures.is_some())
            .field("events", &self.events.is_some())
            .field("inner", &self.inner)
            .field("forwarders", &self.forwarders.len())
            .finish()
    }
}

/// Build a tokio channel and spawn a blocking forwarder that bridges
/// items from a `std::sync::mpsc::Receiver` to the tokio side.
fn spawn_forwarder<T: Send + 'static>(
    sync_rx: std::sync::mpsc::Receiver<T>,
    forwarders: &mut Vec<JoinHandle<()>>,
) -> mpsc::Receiver<T> {
    let (tx, rx) = mpsc::channel::<T>(FORWARDER_CAPACITY);
    forwarders.push(tokio::task::spawn_blocking(move || {
        while let Ok(item) = sync_rx.recv() {
            if tx.blocking_send(item).is_err() {
                break;
            }
        }
    }));
    rx
}

impl TokioCameraRunner {
    /// Build a sync [`CameraRunner`] and wire forwarder tasks for each
    /// available channel.
    ///
    /// # Errors
    /// Returns [`NokhwaError`] if the underlying [`CameraRunner::spawn`]
    /// fails.
    pub fn spawn(camera: OpenedCamera, cfg: RunnerConfig) -> Result<Self, NokhwaError> {
        let mut runner = CameraRunner::spawn(camera, cfg)?;
        let mut forwarders: Vec<JoinHandle<()>> = Vec::new();

        let frames = runner
            .take_frames()
            .map(|sync_rx| spawn_forwarder(sync_rx, &mut forwarders));
        let pictures = runner
            .take_pictures()
            .map(|sync_rx| spawn_forwarder(sync_rx, &mut forwarders));
        let events = runner
            .take_events()
            .map(|sync_rx| spawn_forwarder(sync_rx, &mut forwarders));

        Ok(Self {
            frames,
            pictures,
            events,
            inner: Some(runner),
            forwarders,
        })
    }

    /// Mutable access to the frames receiver. Call `recv().await` on it.
    pub fn frames_mut(&mut self) -> Option<&mut mpsc::Receiver<Buffer>> {
        self.frames.as_mut()
    }

    /// Mutable access to the pictures receiver.
    pub fn pictures_mut(&mut self) -> Option<&mut mpsc::Receiver<Buffer>> {
        self.pictures.as_mut()
    }

    /// Mutable access to the events receiver.
    pub fn events_mut(&mut self) -> Option<&mut mpsc::Receiver<CameraEvent>> {
        self.events.as_mut()
    }

    /// Trigger a shutter capture on the underlying sync runner.
    ///
    /// # Errors
    /// See [`CameraRunner::trigger`].
    pub fn trigger(&self) -> Result<(), NokhwaError> {
        self.inner
            .as_ref()
            .ok_or_else(|| NokhwaError::general("runner already stopped"))?
            .trigger()
    }

    /// Set a camera control on the underlying sync runner.
    ///
    /// # Errors
    /// See [`CameraRunner::set_control`].
    pub fn set_control(
        &self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        self.inner
            .as_ref()
            .ok_or_else(|| NokhwaError::general("runner already stopped"))?
            .set_control(id, value)
    }

    /// Stop forwarders and join the underlying sync runner on a
    /// `spawn_blocking` task.
    ///
    /// # Errors
    /// Returns [`NokhwaError`] only if the `spawn_blocking` task panics.
    pub async fn stop(mut self) -> Result<(), NokhwaError> {
        // `abort()` on spawn_blocking tasks is advisory — see the
        // crate-level note. Forwarders actually exit when their
        // `sync_rx.recv()` returns Err, which happens as soon as the
        // sync runner's `Drop` closes its senders below.
        for f in self.forwarders.drain(..) {
            f.abort();
        }
        // Drop the async receivers so forwarders observe the closed tx
        // and exit cleanly (their blocking_send returns an error).
        self.frames = None;
        self.pictures = None;
        self.events = None;
        if let Some(inner) = self.inner.take() {
            tokio::task::spawn_blocking(move || drop(inner))
                .await
                .map_err(|e| NokhwaError::general(format!("runner join failed: {e}")))?;
        }
        Ok(())
    }
}

impl Drop for TokioCameraRunner {
    fn drop(&mut self) {
        // See note on `stop()`: `abort()` is advisory for spawn_blocking.
        for f in self.forwarders.drain(..) {
            f.abort();
        }
        self.frames = None;
        self.pictures = None;
        self.events = None;
        if let Some(inner) = self.inner.take() {
            match tokio::runtime::Handle::try_current() {
                Ok(h) => {
                    h.spawn_blocking(move || drop(inner));
                }
                Err(_) => {
                    // No tokio runtime: the calling thread is not an
                    // executor task, so a synchronous join is safe.
                    drop(inner);
                }
            }
        }
    }
}
