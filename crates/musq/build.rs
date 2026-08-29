//! Build-time SQLite linkage policy checks.

use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

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

    let header = sqlite3_header_path();
    println!("cargo:rerun-if-changed={}", header.display());
    let version = bundled_sqlite_version(&header);
    println!("cargo:rustc-env=BUNDLED_SQLITE_VERSION={version}");
}

/// Path to the `sqlite3.h` header exported by `libsqlite3-sys`.
fn sqlite3_header_path() -> PathBuf {
    let include = env::var("DEP_SQLITE3_INCLUDE").expect(
        "DEP_SQLITE3_INCLUDE is missing; musq requires libsqlite3-sys with the bundled feature",
    );
    Path::new(&include).join("sqlite3.h")
}

/// Read `SQLITE_VERSION` from the amalgamation header.
fn bundled_sqlite_version(header: &Path) -> String {
    let text = fs::read_to_string(header).unwrap_or_else(|error| {
        panic!("failed to read {}: {error}", header.display());
    });
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("#define SQLITE_VERSION") else {
            continue;
        };
        if rest.starts_with('_') {
            continue;
        }
        let rest = rest.trim();
        if let Some(version) = rest.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            return version.to_string();
        }
    }
    panic!("SQLITE_VERSION not found in {}", header.display());
}

/// Return whether an environment variable has a meaningful override value.
fn env_var_enabled(name: &str) -> bool {
    env::var_os(name).is_some_and(|value| {
        !value.is_empty() && value != OsStr::new("0") && value != OsStr::new("false")
    })
}
