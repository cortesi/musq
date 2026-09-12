derive_expansion_tests!(
    decode::expand_derive_decode,
    enum_impl = "impl < 'r > :: musq :: decode :: Decode < 'r > for Foo",
    enum_generic_impl = "impl < 'r , T > :: musq :: decode :: Decode < 'r > for Foo < T >",
    struct_impl = "impl < 'r > :: musq :: decode :: Decode < 'r > for Foo",
    struct_body = "map (Self)",
);

use syn::parse_str;

use crate::decode::expand_derive_decode;

#[test]
fn derive_struct_generic() {
    let input = parse_str("struct Foo<T>(T);").unwrap();
    let tokens = expand_derive_decode(&input).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl < 'r , T > :: musq :: decode :: Decode < 'r > for Foo < T >"));
}

#[test]
fn derive_struct_user_lifetime() {
    let input = parse_str("struct Foo<'r>(&'r str);").unwrap();
    let tokens = expand_derive_decode(&input).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl < 'r , 'r_1 > :: musq :: decode :: Decode < 'r_1 > for Foo < 'r >"));
}

#[test]
fn derive_enum_rename_all() {
    let input = parse_str("#[musq(rename_all = \"lower_case\")] enum Foo { One, Two }").unwrap();
    let tokens = expand_derive_decode(&input).unwrap();
    assert!(tokens.to_string().contains("\"one\""));
}

#[test]
fn derive_struct_try_from() {
    let input = parse_str("#[musq(try_from = \"String\")] struct Foo(String);").unwrap();
    let tokens = expand_derive_decode(&input).unwrap();
    assert!(tokens.to_string().contains("TryFrom"));
}
