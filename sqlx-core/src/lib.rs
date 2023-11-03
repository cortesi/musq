//! Core of SQLx, the rust SQL toolkit.
//!
//! ### Note: Semver Exempt API
//! The API of this crate is not meant for general use and does *not* follow Semantic Versioning.
//! The only crate that follows Semantic Versioning in the project is the `sqlx` crate itself.
//! If you are building a custom SQLx driver, you should pin an exact version for `sqlx-core` to
//! avoid breakages:
//!
//! ```toml
//! sqlx-core = { version = "=0.6.2" }
//! ```
//!
//! And then make releases in lockstep with `sqlx-core`. We recommend all driver crates, in-tree
//! or otherwise, use the same version numbers as `sqlx-core` to avoid confusion.
#![recursion_limit = "512"]
#![warn(future_incompatible, rust_2018_idioms)]
#![allow(clippy::needless_doctest_main, clippy::type_complexity)]
// See `clippy.toml` at the workspace root
#![deny(clippy::disallowed_method)]
// Allows an API be documented as only available in some specific platforms.
// <https://doc.rust-lang.org/unstable-book/language-features/doc-cfg.html>
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod sqlite;

#[macro_use]
pub mod ext;

#[macro_use]
pub mod error;

#[macro_use]
pub mod arguments;

#[macro_use]
pub mod pool;

pub mod connection;

#[macro_use]
pub mod transaction;

#[macro_use]
pub mod encode;

#[macro_use]
pub mod decode;

#[macro_use]
pub mod types;

#[macro_use]
pub mod query;

#[macro_use]
pub mod acquire;

#[macro_use]
pub mod column;

#[macro_use]
pub mod statement;

pub mod common;
pub mod database;
pub mod describe;
pub mod executor;
pub mod from_row;
pub mod logger;
pub mod query_as;
pub mod query_builder;
pub mod query_scalar;
pub mod row;
pub mod type_info;
pub mod value;

pub use error::{Error, Result};

/// sqlx uses ahash for increased performance, at the cost of reduced DoS resistance.
pub use ahash::AHashMap as HashMap;
pub use either::Either;
pub use indexmap::IndexMap;
pub use percent_encoding;
pub use smallvec::SmallVec;
pub use url::{self, Url};

pub use bytes;
