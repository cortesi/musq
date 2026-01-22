//! trybuild coverage for musq's proc-macro API.

#[cfg(test)]
mod tests {
    #[test]
    fn trybuild() {
        let t = trybuild::TestCases::new();
        t.pass("tests/trybuild/pass_*.rs");
        t.compile_fail("tests/trybuild/fail_*.rs");
    }
}
