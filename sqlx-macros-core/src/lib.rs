//! Support crate for SQLx's proc macros.
//!
//! ### Note: Semver Exempt API
//! The API of this crate is not meant for general use and does *not* follow Semantic Versioning.
//! The only crate that follows Semantic Versioning in the project is the `sqlx` crate itself.
//! If you are building a custom SQLx driver, you should pin an exact version of this and
//! `sqlx-core` to avoid breakages:
//!
//! ```toml
//! sqlx-core = "=0.6.2"
//! sqlx-macros-core = "=0.6.2"
//! ```
//!
//! And then make releases in lockstep with `sqlx-core`. We recommend all driver crates, in-tree
//! or otherwise, use the same version numbers as `sqlx-core` to avoid confusion.

#![cfg_attr(
    any(sqlx_macros_unstable, procmacro2_semver_exempt),
    feature(track_path)
)]

pub type Error = Box<dyn std::error::Error>;

pub type Result<T> = std::result::Result<T, Error>;

mod common;
mod database;

pub mod derives;

// The compiler gives misleading help messages about `#[cfg(test)]` when this is just named `test`.
pub mod test_attr;

pub fn block_on<F>(f: F) -> F::Output
where
    F: std::future::Future,
{
    {
        use once_cell::sync::Lazy;
        use tokio::runtime::{self, Runtime};

        // We need a single, persistent Tokio runtime since we're caching connections,
        // otherwise we'll get "IO driver has terminated" errors.
        static TOKIO_RT: Lazy<Runtime> = Lazy::new(|| {
            runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to start Tokio runtime")
        });

        return TOKIO_RT.block_on(f);
    }
}
