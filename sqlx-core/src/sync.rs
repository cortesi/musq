// For types with identical signatures that don't require runtime support,
// we can just arbitrarily pick one to use based on what's enabled.
//
// We'll generally lean towards Tokio's types as those are more featureful
// (including `tokio-console` support) and more widely deployed.

pub use tokio::sync::{Mutex as AsyncMutex, MutexGuard as AsyncMutexGuard};
