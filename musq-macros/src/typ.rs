use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, DeriveInput, Type};

use super::core;

pub fn expand_derive_type(input: &DeriveInput) -> syn::Result<TokenStream> {
    core::expand_type_derive(input, &expand_struct, &expand_repr_enum, &expand_enum)
}

/// An enum with a repr attribute defining the underlying type.
fn expand_repr_enum(
    container: &core::TypeContainer,
    _: &Vec<core::TypeVariant>,
    repr: &Type,
) -> syn::Result<TokenStream> {
    let ident = &container.ident;
    Ok(quote!(
        #[automatically_derived]
        impl musq::Type for #ident
        where
            #repr: musq::Type,
        {
            fn type_info() -> musq::SqliteDataType {
                <#repr as musq::Type>::type_info()
            }

            fn compatible(ty: &musq::SqliteDataType) -> bool {
                <#repr as musq::Type>::compatible(ty)
            }
        }
    ))
}

/// A plain enum, without a repr attribute. The underlying type is `str`.
fn expand_enum(
    container: &core::TypeContainer,
    _: &Vec<core::TypeVariant>,
) -> syn::Result<TokenStream> {
    let ident = &container.ident;
    Ok(quote!(
        #[automatically_derived]
        impl musq::Type for #ident {
            fn type_info() -> musq::SqliteDataType {
                <::std::primitive::str as musq::Type>::type_info()
            }

            fn compatible(ty: &musq::SqliteDataType) -> ::std::primitive::bool {
                <&::std::primitive::str as musq::Type>::compatible(ty)
            }
        }
    ))
}

fn expand_struct(
    container: &core::TypeContainer,
    field: &core::TypeField,
) -> syn::Result<TokenStream> {
    let (_, ty_generics, _) = container.generics.split_for_impl();

    let ty = &field.ty;
    let ident = &container.ident;
    let mut generics = container.generics.clone();
    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(#ty: musq::Type));
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    Ok(quote!(
        #[automatically_derived]
        impl #impl_generics musq::Type for #ident #ty_generics #where_clause {
            fn type_info() -> musq::SqliteDataType {
                <#ty as musq::Type>::type_info()
            }

            fn compatible(ty: &musq::SqliteDataType) -> ::std::primitive::bool {
                <#ty as musq::Type>::compatible(ty)
            }
        }
    ))
}

#[cfg(test)]

mod tests {
    use super::core::assert_errors_with;
    use super::*;

    #[test]
    fn it_errors_on_invalid() {
        let txt = r#"struct Empty {}"#;
        let e = expand_derive_type(&syn::parse_str(txt).unwrap());
        assert_errors_with!(e, "zero fields");

        let txt = r#"struct Unnamed(i32, i32);"#;
        let e = expand_derive_type(&syn::parse_str(txt).unwrap());
        assert_errors_with!(e, "unnamed field");

        let txt = r#"struct Unit;"#;
        let e = expand_derive_type(&syn::parse_str(txt).unwrap());
        assert_errors_with!(e, "zero fields");

        let txt = r#"
            #[musq(rename_all = "lower_case", repr = "i32")]
            enum Foo {One, Two}
        "#;
        let e = expand_derive_type(&syn::parse_str(txt).unwrap());
        assert_errors_with!(e, "not supported for enums");

        let txt = r#"
            #[musq(transparent)]
            enum Foo {One, Two}
        "#;
        let e = expand_derive_type(&syn::parse_str(txt).unwrap());
        assert_errors_with!(e, "not supported for enums");
    }

    #[test]
    fn it_derives_type() {
        let txt = r#"enum Foo {One, Two}"#;
        expand_derive_type(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            #[musq(repr = "i32")]
            enum Foo {One, Two}
        "#;
        expand_derive_type(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            #[musq(transparent)]
            struct Foo(i32);
        "#;
        expand_derive_type(&syn::parse_str(txt).unwrap()).unwrap();
    }
}