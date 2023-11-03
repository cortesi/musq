// For types with identical signatures that don't require runtime support,
// we can just arbitrarily pick one to use based on what's enabled.
//
// We'll generally lean towards Tokio's types as those are more featureful
// (including `tokio-console` support) and more widely deployed.

pub use tokio::sync::{Mutex as AsyncMutex, MutexGuard as AsyncMutexGuard};

pub struct AsyncSemaphore {
    inner: tokio::sync::Semaphore,
}

impl AsyncSemaphore {
    #[track_caller]
    pub fn new(fair: bool, permits: usize) -> Self {
        AsyncSemaphore {
            inner: {
                debug_assert!(fair, "Tokio only has fair permits");
                tokio::sync::Semaphore::new(permits)
            },
        }
    }

    pub fn permits(&self) -> usize {
        return self.inner.available_permits();
    }

    pub async fn acquire(&self, permits: u32) -> AsyncSemaphoreReleaser<'_> {
        return AsyncSemaphoreReleaser {
            inner: self
                .inner
                // Weird quirk: `tokio::sync::Semaphore` mostly uses `usize` for permit counts,
                // but `u32` for this and `try_acquire_many()`.
                .acquire_many(permits)
                .await
                .expect("BUG: we do not expose the `.close()` method"),
        };
    }

    pub fn try_acquire(&self, permits: u32) -> Option<AsyncSemaphoreReleaser<'_>> {
        return Some(AsyncSemaphoreReleaser {
            inner: self.inner.try_acquire_many(permits).ok()?,
        });
    }

    pub fn release(&self, permits: usize) {
        return self.inner.add_permits(permits);
    }
}

pub struct AsyncSemaphoreReleaser<'a> {
    // We use the semaphore from futures-intrusive as the one from async-std
    // is missing the ability to add arbitrary permits, and is not guaranteed to be fair:
    // * https://github.com/smol-rs/async-lock/issues/22
    // * https://github.com/smol-rs/async-lock/issues/23
    //
    // We're on the look-out for a replacement, however, as futures-intrusive is not maintained
    // and there are some soundness concerns (although it turns out any intrusive future is unsound
    // in MIRI due to the necessitated mutable aliasing):
    // https://github.com/launchbadge/sqlx/issues/1668
    inner: tokio::sync::SemaphorePermit<'a>,
}

impl AsyncSemaphoreReleaser<'_> {
    pub fn disarm(self) {
        {
            self.inner.forget();
            return;
        }
    }
}
