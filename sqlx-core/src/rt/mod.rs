use std::future::Future;

#[track_caller]
pub fn test_block_on<F: Future>(f: F) -> F::Output {
    return tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to start Tokio runtime")
        .block_on(f);
}
