use crate::core::assert_errors_with;
use crate::decode::expand_derive_decode;
use syn::parse_str;

#[test]
fn derive_enum() {
    let input = parse_str("enum Foo { One, Two }").unwrap();
    let tokens = expand_derive_decode(&input).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl < 'r > musq :: decode :: Decode < 'r > for Foo"));
}

#[test]
fn derive_enum_with_repr() {
    let input = parse_str("#[musq(repr = \"i32\")] enum Foo { One, Two }").unwrap();
    let tokens = expand_derive_decode(&input).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl < 'r > musq :: decode :: Decode < 'r > for Foo"));
    assert!(s.contains("as i32"));
}

#[test]
fn derive_struct() {
    let input = parse_str("struct Foo(i32);").unwrap();
    let tokens = expand_derive_decode(&input).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl < 'r > musq :: decode :: Decode < 'r > for Foo"));
    assert!(s.contains("map (Self)"));
}

#[test]
fn error_on_named_struct() {
    let input = parse_str("struct Foo { a: i32 }").unwrap();
    let e = expand_derive_decode(&input);
    assert_errors_with!(e, "structs must have exactly one unnamed field");
}
