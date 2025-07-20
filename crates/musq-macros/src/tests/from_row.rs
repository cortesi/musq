use crate::core::assert_errors_with;
use crate::row::expand_derive_from_row;
use syn::parse_str;

#[test]
fn derive_struct() {
    let txt = "struct Foo { a: i32, b: String }";
    let tokens = expand_derive_from_row(&parse_str(txt).unwrap()).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl < 'a > musq :: FromRow < 'a > for Foo"));
}

#[test]
fn derive_tuple_struct() {
    let txt = "struct Foo(i32, String);";
    let tokens = expand_derive_from_row(&parse_str(txt).unwrap()).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl < 'a , R : musq :: Row > musq :: FromRow < 'a > for Foo"));
}

#[test]
fn error_on_unit_struct() {
    let txt = "struct Foo;";
    let e = expand_derive_from_row(&parse_str(txt).unwrap());
    assert_errors_with!(e, "Unsupported shape");
}
