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

#[test]
fn derive_struct_with_deserialize_with() {
    let txt = r#"
        struct Foo {
            a: i32,
            #[musq(deserialize_with = "custom_deserializer")]
            b: CustomType,
        }
    "#;
    let tokens = expand_derive_from_row(&parse_str(txt).unwrap()).unwrap();
    let s = tokens.to_string();
    // Check that it calls the custom function
    assert!(s.contains("custom_deserializer (prefix , row)"));
    // Check that no Decode bound is added for CustomType
    assert!(!s.contains("CustomType : musq :: decode :: Decode"));
}

#[test]
fn deserialize_with_with_module_path() {
    let txt = r#"
        struct Foo {
            #[musq(deserialize_with = "my_mod::submod::deserializer")]
            data: Data,
        }
    "#;
    let tokens = expand_derive_from_row(&parse_str(txt).unwrap()).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("my_mod :: submod :: deserializer (prefix , row)"));
}

#[test]
fn error_deserialize_with_and_flatten() {
    let txt = r#"
        struct Foo {
            #[musq(flatten, deserialize_with = "custom")]
            a: Bar,
        }
    "#;
    let e = expand_derive_from_row(&parse_str(txt).unwrap());
    assert_errors_with!(e, "`flatten` cannot be combined with `deserialize_with`");
}

#[test]
fn error_deserialize_with_and_try_from() {
    let txt = r#"
        struct Foo {
            #[musq(deserialize_with = "custom", try_from = "i32")]
            a: Bar,
        }
    "#;
    let e = expand_derive_from_row(&parse_str(txt).unwrap());
    assert_errors_with!(e, "`deserialize_with` cannot be combined with `try_from`");
}

#[test]
fn error_deserialize_with_and_skip() {
    let txt = r#"
        struct Foo {
            #[musq(deserialize_with = "custom", skip)]
            a: Bar,
        }
    "#;
    let e = expand_derive_from_row(&parse_str(txt).unwrap());
    assert_errors_with!(e, "`deserialize_with` cannot be combined with `skip`");
}

#[test]
fn error_deserialize_with_and_default() {
    let txt = r#"
        struct Foo {
            #[musq(deserialize_with = "custom", default)]
            a: Bar,
        }
    "#;
    let e = expand_derive_from_row(&parse_str(txt).unwrap());
    assert_errors_with!(e, "`deserialize_with` cannot be combined with `default`");
}
