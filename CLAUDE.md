# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Nokhwa (녹화, "to record") is a cross-platform Rust webcam capture library. It provides a unified `Camera` API that abstracts over platform-specific backends (V4L2 on Linux, Media Foundation on Windows, AVFoundation on macOS, OpenCV cross-platform).

## Build Commands

```bash
# Build on macOS (native backend)
cargo build --features input-avfoundation

# Build on Linux (native backend)
cargo build --features input-v4l

# Build on Windows (native backend)
cargo build --features input-msmf

# Build with all native backends + extras (for docs/CI)
cargo build --features docs-only,docs-nolink,docs-features

# Check without linking (useful for cross-platform code review)
cargo check --features docs-only,docs-nolink

# Lint
cargo clippy --features input-avfoundation  # (use your platform's input-* feature)

# Format
cargo fmt --all

# Enable pre-commit hook (runs fmt + clippy before each commit)
git config core.hooksPath .githooks
```

The crate uses nightly Rust (configured via flake.nix). The `docs-features` feature requires nightly (`doc_cfg`).

## Workspace Structure

This is a Cargo workspace with these crates:

- **`nokhwa`** (root) — Main crate exposing `Camera`, `CallbackCamera`, query functions, and re-exports from `nokhwa-core`
- **`nokhwa-core`** — Core types, traits, error types, buffer, pixel format decoders. No platform-specific code.
- **`nokhwa-bindings-macos`** — AVFoundation FFI bindings (Objective-C interop)
- **`nokhwa-bindings-linux`** — V4L2 bindings via `v4l` crate
- **`nokhwa-bindings-windows`** — Media Foundation bindings via `windows` crate

## Architecture

### Layered abstraction

1. **`nokhwa-core::traits::CaptureBackendTrait`** — The central trait all backends implement. Defines open/close stream, frame capture, format negotiation, camera controls.
2. **Platform binding crates** (`nokhwa-bindings-*`) — Each implements `CaptureBackendTrait` for its platform's API.
3. **`src/backends/capture/`** — Re-exports and wraps backend structs. Conditional compilation via `cfg` features selects the right backend per platform.
4. **`src/camera.rs` → `Camera`** — User-facing struct holding a `Box<dyn CaptureBackendTrait>`. Delegates all calls to the selected backend.
5. **`src/threaded.rs` → `CallbackCamera`** — Wraps `Camera` with a background thread and callback support (feature `output-threaded`).

### Feature flags drive compilation

Almost everything is behind feature flags. A build **must** enable at least one `input-*` feature to be functional. The `input-native` meta-feature selects the right backend for the current OS. Key combinations:
- `input-native` = `input-avfoundation` + `input-v4l` + `input-msmf`
- `decoding` (default) = MJPEG decoding via `mozjpeg`
- `output-wgpu` = Direct frame-to-wgpu-texture copy
- `output-threaded` = `CallbackCamera` with `parking_lot`

### Frame pipeline

`CaptureBackendTrait::frame()` → `nokhwa_core::buffer::Buffer` (raw bytes + format metadata) → `Buffer::decode_image::<FormatDecoder>()` → `image::ImageBuffer`. Pixel format decoders implement `FormatDecoder` trait in `nokhwa-core/src/pixel_format.rs`.

## Code Style

- `#![deny(clippy::pedantic)]` and `#![warn(clippy::all)]` are enforced in both `nokhwa` and `nokhwa-core`
- `clippy::module_name_repetitions` is allowed
- Run `cargo fmt` before committing
- Minimize use of `unsafe`
- Apache 2.0 license
