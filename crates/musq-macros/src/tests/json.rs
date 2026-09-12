use syn::parse_str;

use crate::json::expand_json;

#[test]
fn derive_json_struct() {
    let txt = "struct Foo { a: i32, b: String }";
    let tokens = expand_json(&parse_str(txt).unwrap()).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl :: musq :: encode :: Encode for Foo"));
    assert!(s.contains("impl < 'r > :: musq :: decode :: Decode < 'r > for Foo"));
}

#[test]
fn derive_json_enum() {
    let txt = "enum Foo { Unit, Tuple(i32), Named { value: String } }";
    let tokens = expand_json(&parse_str(txt).unwrap()).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl :: musq :: encode :: Encode for Foo"));
    assert!(s.contains("impl < 'r > :: musq :: decode :: Decode < 'r > for Foo"));
}

#[test]
fn derive_json_generic() {
    let txt = "struct Foo<T> { val: T }";
    let tokens = expand_json(&parse_str(txt).unwrap()).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl < 'r , T > :: musq :: decode :: Decode < 'r > for Foo < T >"));
}

#[test]
fn derive_json_user_lifetime() {
    let txt = "struct Foo<'r> { val: &'r str }";
    let tokens = expand_json(&parse_str(txt).unwrap()).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl < 'r , 'r_1 > :: musq :: decode :: Decode < 'r_1 > for Foo < 'r >"));
}
