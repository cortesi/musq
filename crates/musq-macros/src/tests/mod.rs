/// Generate the shared derive-expansion tests for one encode or decode
/// expander.
macro_rules! derive_expansion_tests {
    (
        $module:ident :: $func:ident,
        enum_impl = $enum_impl:literal,
        enum_generic_impl = $enum_generic_impl:literal,
        struct_impl = $struct_impl:literal,
        struct_body = $struct_body:literal $(,)?
    ) => {
        #[test]
        fn derive_enum() {
            let input = syn::parse_str("enum Foo { One, Two }").unwrap();
            let tokens = crate::$module::$func(&input).unwrap();
            assert!(tokens.to_string().contains($enum_impl));
        }

        #[test]
        fn derive_enum_generic() {
            let input = syn::parse_str("enum Foo<T> { One(T), Two }").unwrap();
            let tokens = crate::$module::$func(&input).unwrap();
            assert!(tokens.to_string().contains($enum_generic_impl));
        }

        #[test]
        fn derive_enum_with_repr() {
            let input = syn::parse_str("#[musq(repr = \"i32\")] enum Foo { One, Two }").unwrap();
            let tokens = crate::$module::$func(&input).unwrap();
            let s = tokens.to_string();
            assert!(s.contains($enum_impl));
            assert!(s.contains("as i32"));
        }

        #[test]
        fn derive_struct() {
            let input = syn::parse_str("struct Foo(i32);").unwrap();
            let tokens = crate::$module::$func(&input).unwrap();
            let s = tokens.to_string();
            assert!(s.contains($struct_impl));
            assert!(s.contains($struct_body));
        }

        #[test]
        fn error_on_named_struct() {
            let input = syn::parse_str("struct Foo { a: i32 }").unwrap();
            let e = crate::$module::$func(&input);
            crate::core::assert_errors_with!(e, "structs must have exactly one unnamed field");
        }
    };
}

mod decode;
mod encode;
mod from_row;
mod json;
