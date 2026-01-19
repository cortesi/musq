use syn::parse_str;

use crate::{core::assert_errors_with, row::expand_derive_from_row};

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
fn derive_struct_with_generics() {
    let txt = "struct Foo<T> { a: T }";
    let tokens = expand_derive_from_row(&parse_str(txt).unwrap()).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl < 'a , T > musq :: FromRow < 'a > for Foo < T >"));
}

#[test]
fn derive_struct_with_lifetime() {
    let txt = "struct Foo<'r, T> { a: &'r T }";
    let tokens = expand_derive_from_row(&parse_str(txt).unwrap()).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl < 'r , T > musq :: FromRow < 'r > for Foo < 'r , T >"));
}

#[test]
fn derive_tuple_struct_with_generics() {
    let txt = "struct Foo<T>(T);";
    let tokens = expand_derive_from_row(&parse_str(txt).unwrap()).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl < 'a , R : musq :: Row , T > musq :: FromRow < 'a > for Foo < T >"));
}

#[test]
fn derive_tuple_struct_with_lifetime() {
    let txt = "struct Foo<'r, T>(&'r T);";
    let tokens = expand_derive_from_row(&parse_str(txt).unwrap()).unwrap();
    let s = tokens.to_string();
    assert!(
        s.contains("impl < 'r , R : musq :: Row , T > musq :: FromRow < 'r > for Foo < 'r , T >")
    );
}

#[test]
fn error_on_unit_struct() {
    let txt = "struct Foo;";
    let e = expand_derive_from_row(&parse_str(txt).unwrap());
    assert_errors_with!(e, "Unsupported shape");
}
