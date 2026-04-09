# Release Checklist

## Pre-release

- [ ] All CI workflows pass (lint, build-matrix, test-core)
- [ ] `cargo fmt --all -- --check` passes locally
- [ ] `cargo clippy --features input-avfoundation` (macOS) / `input-v4l` (Linux) / `input-msmf` (Windows) — no errors
- [ ] `cargo test -p nokhwa-core` — all tests pass
- [ ] Run example programs with platform-native backend, verify no errors
- [ ] `cargo doc --features docs-only,docs-nolink,docs-features` — no broken links

## Release

- [ ] Update version numbers in all Cargo.toml files (root, nokhwa-core, bindings crates)
- [ ] Update CHANGELOG.md with new version entry
- [ ] Commit version bump and changelog
- [ ] Create git tag: `git tag -s v0.x.y`
- [ ] Publish in order: `./publish.sh` (core → bindings → root)

## Post-release

- [ ] Verify crates.io pages are correct
- [ ] Update README if API changed
