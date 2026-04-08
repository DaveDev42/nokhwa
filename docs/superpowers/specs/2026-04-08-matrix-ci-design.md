# Matrix CI Design

Cross-platform CI pipeline for the nokhwa webcam capture library.

## Goals

1. Build verification across all supported platforms (Linux, Windows, macOS)
2. Unit tests for platform-independent core logic
3. Real camera integration tests on self-hosted macOS runner
4. Lint and format enforcement

## Workflows

### lint.yml

- **Trigger**: PR, main push
- **Runner**: `ubuntu-latest`
- **Steps**:
  - `cargo fmt --all -- --check`
  - `cargo clippy --features docs-only,docs-nolink -- -D warnings`
- **Purpose**: Enforce formatting and lint rules. Uses `docs-only,docs-nolink` to include all platform code without linking.

### build-matrix.yml

- **Trigger**: PR, main push
- **Runner**: GitHub-hosted (Linux, Windows) + self-hosted macOS (`macos-camera`)
- **Matrix**:

| OS | Runner | Feature |
|---|---|---|
| Linux | `ubuntu-latest` | `input-v4l` |
| Windows | `windows-latest` | `input-msmf` |
| macOS | `macos-camera` (self-hosted) | `input-avfoundation` |

- **Steps**: Install nightly Rust toolchain, install system deps (Linux: `libv4l-dev`), `cargo build --features <feature>`
- **Purpose**: Verify that each platform's native backend compiles successfully.

### test-core.yml

- **Trigger**: PR, main push
- **Runner**: `ubuntu-latest`
- **Steps**:
  1. `cargo test -p nokhwa-core` — unit tests for core types
  2. `cargo build --features output-threaded` — build verification
  3. `cargo build --features output-wgpu` — build verification
  4. `cargo build --features decoding` — build verification
- **Purpose**: Test platform-independent logic and verify independent feature builds.

### device-test.yml

- **Trigger**: main push only
- **Runner**: `macos-camera` (self-hosted, physical webcam attached)
- **Steps**: `cargo test --features device-test,input-avfoundation,output-threaded`
- **Purpose**: Integration tests with real camera hardware.

## New Code

### nokhwa-core unit tests

Location: `nokhwa-core/src/*.rs` (inline `#[cfg(test)]` modules)

Test targets:
- `Buffer` — creation, metadata access, format queries
- `FrameFormat` — enum variants, display/debug
- `Resolution` — construction, comparison, ordering
- `CameraFormat` — creation from components, format negotiation helpers
- `NokhwaError` — error variant construction, Display impl, From conversions

### Device integration tests

Location: `tests/device_tests.rs` (workspace root)

Gated by `#[cfg(feature = "device-test")]` — only runs when `device-test` feature is enabled.

Test targets:
- `query_devices()` — enumerate available cameras, verify non-empty list
- Camera open/close — open default camera, verify stream lifecycle
- Frame capture — start stream, capture frame, verify non-empty buffer with valid dimensions
- Format query and change — list supported formats, switch format, verify new format applied
- `CallbackCamera` — register callback, verify frames received via channel

Each test includes a skip helper that gracefully skips if no camera is detected (safety net for misconfigured runners).

### New feature flag

Add `device-test` to root `Cargo.toml`:
```toml
[features]
device-test = []
```

This flag gates integration test compilation so `cargo test` without the flag skips device tests entirely.

## Self-hosted Runner Setup

### Requirements

- macOS machine with physical webcam (built-in FaceTime camera works)
- Xcode Command Line Tools installed
- Nightly Rust toolchain installed
- GitHub Actions runner agent registered with label `macos-camera`

### Camera TCC Permission

The runner process needs camera access permission. Options:
1. Run the runner interactively once, trigger a camera test, approve the TCC prompt manually
2. Disable SIP and insert TCC approval directly into the database

Option 1 is recommended — simpler and only needs to be done once per machine.

### Runner Registration

```bash
# Download and configure the runner (follow GitHub's instructions for the repo)
./config.sh --url https://github.com/DaveDev42/nokhwa --token <TOKEN> --labels macos-camera
./run.sh  # or install as a service with ./svc.sh install
```
