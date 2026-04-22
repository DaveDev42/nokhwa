# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Nokhwa (녹화, "to record") is a cross-platform Rust webcam capture library. It provides capability-split wrappers (`StreamCamera` / `ShutterCamera` / `HybridCamera`) dispatched from the top-level `open()` function, with platform-specific backends: V4L2 on Linux, Media Foundation on Windows, AVFoundation on macOS, plus a cross-platform GStreamer backend that also handles RTSP / HTTP / file URL sources via `uridecodebin`.

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

The crate requires **Rust nightly** (configured via flake.nix) because the `docs-features` feature uses `#![feature(doc_cfg)]`. Regular builds work on stable Rust.

## Workspace Structure

This is a Cargo workspace with these crates:

- **`nokhwa`** (root) — User-facing entrypoint. Exposes `open()`, `query()`, `StreamCamera` / `ShutterCamera` / `HybridCamera`, `CameraRunner` (under `runner` feature), and re-exports from `nokhwa-core`.
- **`nokhwa-core`** — Core types, traits, error types, buffer, pixel format decoders. No platform-specific code. Provides the `opencv-mat` feature for `cv::Mat` interop (independent of the removed OpenCV capture backend).
- **`nokhwa-bindings-macos-avfoundation`** — AVFoundation FFI bindings via `objc2`
- **`nokhwa-bindings-linux-v4l`** — V4L2 bindings via the `v4l` crate
- **`nokhwa-bindings-windows-msmf`** — Media Foundation bindings via the `windows` crate
- **`nokhwa-bindings-gstreamer`** — Cross-platform GStreamer backend via `gstreamer-rs`. Handles local capture (session 2), controls on Linux (session 3), and URL sources like `rtsp://` / `http://` / `file://` (session 5) through `uridecodebin`.
- **`nokhwa-tokio`** — Tokio-flavoured wrappers for the async surface.

## Architecture

### Layered abstraction

1. **Capability traits in `nokhwa-core::traits`** — `CameraDevice` (metadata + controls), `FrameSource` (streaming), `ShutterCapture` (still-capture), `EventSource` (hotplug / device events), `HotplugSource` (backend-wide plug/unplug notifications). Backends implement whichever subset they support; the `nokhwa_backend!` macro in `src/session.rs` exposes capability bits for runtime dispatch.
2. **Platform binding crates** (`nokhwa-bindings-*`) — Each implements the relevant capability traits for its platform's API. GStreamer handles both local devices and URL sources.
3. **`src/backends/capture/`** — Re-exports and registers backend structs via `nokhwa_backend!`. Conditional compilation via `cfg` features selects which backends are compiled in.
4. **`src/session.rs` → `open()` / `OpenedCamera` / `StreamCamera` / `ShutterCamera` / `HybridCamera`** — User-facing surface. `open()` routes `CameraIndex::Index` to the native backend and URL-like `CameraIndex::String` to GStreamer via `uridecodebin`. `OpenedCamera` is dispatched by capability bits into one of the three wrapper types.
5. **`src/runner.rs` → `CameraRunner`** — Wraps a `StreamCamera` with a background thread and event channels (feature `runner`).

### Feature flags drive compilation

Almost everything is behind feature flags. A build **must** enable at least one `input-*` feature to be functional. The `input-native` meta-feature selects the right backend for the current OS. Key combinations:
- `input-native` = `input-avfoundation` + `input-v4l` + `input-msmf`
- `input-gstreamer` = cross-platform capture + RTSP / HTTP / file URL sources via GStreamer. Requires a system GStreamer install (`libgstreamer1.0-dev` + `libgstreamer-plugins-base1.0-dev` + `gstreamer1.0-libav` on Ubuntu; the **Complete** installer variant on Windows / macOS).
- `mjpeg` (default) = MJPEG decoding via `mozjpeg`
- `output-wgpu` = Direct frame-to-wgpu-texture copy
- `runner` = `CameraRunner` background-thread helper
- `nokhwa-core/opencv-mat` (optional, independent) = `cv::Mat` interop helpers on `Frame<_>`. Enable this if you want to hand frames into OpenCV for downstream image processing.

### Frame pipeline

`StreamCamera::frame()` → `Buffer` → `Buffer::typed::<F>()` → `Frame<F>` (typed handle). From there: `frame.into_rgb().materialize()` → `image::ImageBuffer`. Format marker types (`Mjpeg`, `Yuyv`, etc.) live in `nokhwa-core/src/format_types.rs`. Conversion traits (`IntoRgb`, `IntoRgba`, `IntoLuma`) in `nokhwa-core/src/frame.rs` are selectively implemented per format (e.g. `Frame<Gray>` cannot convert to RGB).

## Git & GitHub Rules

- This is a **fork** of `l1npengtul/nokhwa`. Our remote is `origin` (`DaveDev42/nokhwa`).
- **NEVER create PRs against the upstream repository (`l1npengtul/nokhwa`).** Always use `--repo DaveDev42/nokhwa` with `gh pr create`.
- `main` branch is protected via GitHub Rulesets (require PR, require signed commits). Do not attempt direct push.
- When using `gh` commands, always specify `--repo DaveDev42/nokhwa` to avoid accidentally targeting upstream.
- **NEVER commit planning artifacts, spec documents, or skill-generated files (e.g. `docs/superpowers/`, `docs/plans/`) to the repository.** Keep all planning work in local context only.
- **Keep `CHANGELOG.md` up to date.** When merging feature or fix PRs, add an entry under the current unreleased version section. Group entries by: Features, Performance, Refactoring, Bug Fixes, Infrastructure, Cleanup.
- **Always keep `TODO.md` current.** After every PR merge or task completion, immediately update TODO.md: mark completed items as done and remove them, add newly discovered issues, and re-prioritize as needed. TODO.md must always reflect the true current state of the project.
- **When delegating tasks to worktrees (gw),** always include in the prompt: "Update TODO.md to mark the relevant item as completed and include the change in your commit." Each worktree PR must include the TODO.md update alongside the code changes.

## Commit & Release Convention

- **Default to patch version bumps.** Unless the user explicitly asks for a major or minor bump, every change (including API-breaking ones in 0.x) must ship as a patch release. release-please drives version bumps from conventional-commit prefixes.
- **Never use `feat!`, `fix!`, or a `BREAKING CHANGE:` footer** in PR titles, squash-merge messages, or commit messages. These escalate release-please to major bumps automatically (e.g. 0.x → 1.0.0). Use plain `feat:` / `fix:` / `refactor:` / `chore:` instead, and describe breaking changes in the PR body and `CHANGELOG.md` / `MIGRATING-*.md` rather than the commit prefix.
- **Manual major/minor bump**: when a major/minor release is explicitly requested, push a commit to `main` with a `Release-As: x.y.z` footer (release-please auto-detects it), or temporarily set `release-as` in `release-please-config.json` via a chore PR, then remove it in a follow-up chore PR after the release ships.
- **Squash-merge messages are authoritative** for release-please. Before merging a PR, verify the squash commit title uses an allowed prefix. If the PR title contains `feat!` / `fix!`, edit the squash message at merge time.

## Versioning

All workspace crates use a **unified version number** (e.g. `0.11.0`). When bumping versions, update ALL `Cargo.toml` files (root, nokhwa-core, nokhwa-bindings-macos, nokhwa-bindings-linux, nokhwa-bindings-windows) to the same version, including cross-reference `version` fields in `[dependencies.nokhwa-*]`.

## Code Style

- `#![deny(clippy::pedantic)]` and `#![warn(clippy::all)]` are enforced in both `nokhwa` and `nokhwa-core`
- `clippy::module_name_repetitions` is allowed
- Run `cargo fmt` before committing
- Minimize use of `unsafe`
- **Apache 2.0 license** — This project is licensed under Apache-2.0 by the original author (`l1npengtul`). Always respect this license: preserve license headers in source files, include the license in distributions, and never relicense or change the license terms.
