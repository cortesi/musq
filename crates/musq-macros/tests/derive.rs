//! trybuild coverage for musq-macros derive output.

// This test uses trybuild to validate that derive macros work correctly on valid input
// and fail appropriately on invalid input. The .stderr files contain expected error
// messages that may need updating when the Rust compiler changes its error format.
//
// If tests fail due to error message changes, run:
//   TRYBUILD=overwrite cargo test derive
// This will update the .stderr files with current compiler output.
#[cfg(test)]
mod tests {
    #[test]
    fn derive() {
        let t = trybuild::TestCases::new();
        t.pass("tests/trybuild/pass_*.rs");
        t.compile_fail("tests/trybuild/fail_*.rs");
    }
}
