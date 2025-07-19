use crate::json::expand_json;
use syn::parse_str;

#[test]
fn derive_json_struct() {
    let txt = "struct Foo { a: i32, b: String }";
    let tokens = expand_json(&parse_str(txt).unwrap()).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("impl musq :: encode :: Encode for Foo"));
    assert!(s.contains("impl < 'r > musq :: decode :: Decode < 'r > for Foo"));
}
