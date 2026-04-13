# Migrating from nokhwa 0.13 to 0.14

0.14.0 is a small API-polish release on top of the 0.13 trait-split.
The main breaking change is cosmetic: the `CameraSession` unit struct
has been removed in favour of a free `nokhwa::open` function.

## Why

`CameraSession` was a leftover from earlier drafts of the 0.13 design.
It held no state, had no constructor, and only exposed a single
associated function (`CameraSession::open`). A free function expresses
the same thing more directly. `OpenRequest` already carries every
per-call parameter, and can grow new builder methods without breaking
callers.

## API changes

### Opening a camera

```rust
// 0.13
use nokhwa::{CameraSession, OpenRequest, OpenedCamera};
let opened = CameraSession::open(index, OpenRequest::any())?;

// 0.14
use nokhwa::{open, OpenRequest, OpenedCamera};
let opened = open(index, OpenRequest::any())?;
```

That is the entire migration. `OpenRequest`, `OpenedCamera`, and every
per-capability wrapper (`StreamCamera`, `ShutterCamera`,
`HybridCamera`) are unchanged.

The free `nokhwa::open` function acquires the device. It is distinct
from the `open()` method on `StreamCamera` / `HybridCamera`, which
starts the frame stream on an already-acquired device. Typical usage:

```rust
use nokhwa::{open, OpenRequest, OpenedCamera};

let opened = open(index, OpenRequest::any())?;   // acquire
if let OpenedCamera::Stream(mut cam) = opened {
    cam.open()?;                                  // start stream
    let _ = cam.frame()?;
    cam.close()?;                                 // stop stream (in-tree backends also close on drop)
}
```

If you import items individually, drop `CameraSession` from the import
list and add `open`:

```rust
// 0.13
use nokhwa::{CameraRunner, CameraSession, OpenRequest, RunnerConfig};

// 0.14
use nokhwa::{open, CameraRunner, OpenRequest, RunnerConfig};
```

### Bounded runner channels

0.14 also changes `CameraRunner`'s default channel behaviour (shipped
in 0.14.0 group A, PR #123). `RunnerConfig` now defaults to bounded
channels — `frames_capacity = 4`, `pictures_capacity = 8`,
`events_capacity = 32`, with `Overflow::DropNewest` as the default
policy. In 0.13 the underlying `std::sync::mpsc::channel` was
unbounded, so a slow consumer would queue without limit. In 0.14 the
slowest-moving item is silently dropped according to the configured
policy.

If you relied on the unbounded behaviour (e.g. a batch pipeline that
tolerates unbounded memory growth in exchange for never losing a
frame), set any of the three capacity fields to `0` to restore the
0.13 semantics.

### External backend crates

The `nokhwa_backend!` macro is unchanged. Backends registered via
`nokhwa_backend!(MyDevice: FrameSource, ShutterCapture, EventSource)`
continue to plug into `OpenedCamera::from_device` exactly as in 0.13.
0.14 adds an integration test that exercises the macro from outside
`src/session.rs` to guarantee the extension point stays usable for
third-party crates (e.g. a Canon EDSDK binding).
