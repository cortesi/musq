//! Build-time SQLite linkage policy checks.

use std::{env, ffi::OsStr};

/// Environment variables that can redirect libsqlite3-sys to an external SQLite.
const UNSUPPORTED_SQLITE_LINK_ENV_VARS: &[&str] = &[
    "LIBSQLITE3_SYS_USE_PKG_CONFIG",
    "SQLITE3_LIB_DIR",
    "SQLITE3_INCLUDE_DIR",
    "SQLITE3_STATIC",
];

fn main() {
    for name in UNSUPPORTED_SQLITE_LINK_ENV_VARS {
        println!("cargo:rerun-if-env-changed={name}");
        assert!(
            !env_var_enabled(name),
            "musq supports only the bundled SQLite release from libsqlite3-sys; \
             unset {name} to build with the bundled library"
        );
    }
}

/// Return whether an environment variable has a meaningful override value.
fn env_var_enabled(name: &str) -> bool {
    env::var_os(name).is_some_and(|value| {
        !value.is_empty() && value != OsStr::new("0") && value != OsStr::new("false")
    })
}
