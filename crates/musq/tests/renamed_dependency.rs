//! Compile-test coverage for proc-macro crate-path resolution.

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, process::Command};

    #[test]
    fn proc_macros_compile_with_renamed_musq_dependency() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/renamed-dependency/Cargo.toml");
        let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/renamed-dependency-fixture");

        let status = Command::new(env!("CARGO"))
            .arg("check")
            .arg("--manifest-path")
            .arg(manifest)
            .arg("--target-dir")
            .arg(target_dir)
            .status()
            .expect("failed to run cargo check for renamed dependency fixture");

        assert!(
            status.success(),
            "renamed dependency fixture failed to compile"
        );
    }
}
