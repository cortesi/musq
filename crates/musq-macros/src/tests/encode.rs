derive_expansion_tests!(
    encode::expand_derive_encode,
    enum_impl = "impl :: musq :: encode :: Encode for Foo",
    enum_generic_impl = "impl < T > :: musq :: encode :: Encode for Foo < T >",
    struct_impl = "impl :: musq :: encode :: Encode for Foo",
    struct_body = "self . 0",
);

use syn::parse_str;

use crate::encode::expand_derive_encode;

#[test]
fn derive_enum_variant_rename_and_raw_ident() {
    let input = parse_str(
        r#"
        #[musq(rename_all = "lower_case")]
        enum Foo {
            #[musq(rename = "custom_one")]
            One,
            r#type,
        }
        "#,
    )
    .unwrap();
    let tokens = expand_derive_encode(&input).unwrap();
    let s = tokens.to_string();
    assert!(s.contains("\"custom_one\""), "got: {s}");
    assert!(!s.contains("\"one\""), "got: {s}");
    assert!(s.contains("\"type\""), "got: {s}");
    assert!(!s.contains("\"r#type\""), "got: {s}");
}
