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

If you import items individually, drop `CameraSession` from the import
list and add `open`:

```rust
// 0.13
use nokhwa::{CameraRunner, CameraSession, OpenRequest, RunnerConfig};

// 0.14
use nokhwa::{open, CameraRunner, OpenRequest, RunnerConfig};
```

### External backend crates

The `nokhwa_backend!` macro is unchanged. Backends registered via
`nokhwa_backend!(MyDevice: FrameSource, ShutterCapture, EventSource)`
continue to plug into `OpenedCamera::from_device` exactly as in 0.13.
0.14 adds an integration test that exercises the macro from outside
`src/session.rs` to guarantee the extension point stays usable for
third-party crates (e.g. a Canon EDSDK binding).
