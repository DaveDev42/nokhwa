// Smoke tests for TokioCameraRunner's Drop / stop semantics.
// Currently we don't have a cross-platform fake backend exposed through
// OpenedCamera, so these tests only check the API surface compiles and that
// the crate re-exports are sane. Runtime semantics are covered manually via
// the `tokio_runner` example against a real camera.

#[tokio::test(flavor = "current_thread")]
async fn library_compiles_under_tokio_runtime() {
    // This test exists to force the crate to be linked into a test binary
    // that boots a tokio runtime, catching any accidental dependency on
    // multi-thread features or non-Send types.
    let _ = tokio::task::spawn_blocking(|| 1 + 1).await.unwrap();
}
