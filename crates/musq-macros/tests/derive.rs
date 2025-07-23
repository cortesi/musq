#[test]
fn derive() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/pass_*.rs");
    t.compile_fail("tests/trybuild/fail_*.rs");
}
