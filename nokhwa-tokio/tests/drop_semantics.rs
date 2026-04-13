// Link-check for `nokhwa-tokio` — forces the crate to be compiled into a
// test binary that boots a tokio runtime, catching any accidental
// dependency on multi-thread features or non-Send types. True drop
// semantics need a cross-platform fake `OpenedCamera`, which does not
// exist yet; cover that via the `tokio_runner` example against a real
// camera.

#[tokio::test(flavor = "current_thread")]
async fn library_links_under_tokio_runtime() {
    let _ = tokio::task::spawn_blocking(|| 1 + 1).await.unwrap();
}
