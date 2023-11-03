use std::path::Path;

#[test]
#[ignore]
fn ui_tests() {
    let t = trybuild::TestCases::new();
    if dotenvy::var("DATABASE_URL").map_or(true, |v| {
        Path::is_relative(v.trim_start_matches("sqlite://").as_ref())
    }) {
        // this isn't `Trybuild`'s fault: https://github.com/dtolnay/trybuild/issues/69#issuecomment-620329526
        panic!("DATABASE_URL must contain an absolute path for SQLite UI tests")
    }
    t.compile_fail("tests/ui/sqlite/*.rs");
    t.compile_fail("tests/ui/*.rs");
}
