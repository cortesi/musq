use darling::ast::Data;
use darling::FromDeriveInput;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, DeriveInput, Type};

use super::attrs;

pub fn expand_derive_type(input: &DeriveInput) -> syn::Result<TokenStream> {
    let attrs = attrs::ContainerAttributes::from_derive_input(input).unwrap();
    Ok(match &attrs.data {
        Data::Struct(fields) => {
            if fields.is_empty() {
                return attrs::span_err!(input, "structs with zero fields are not supported");
            }
            let unnamed = fields.iter().filter(|f| f.ident.is_none()).count();
            let named = fields.iter().filter(|f| f.ident.is_some()).count();
            if named > 1 {
                return attrs::span_err!(input, "structs with named fields are not supported");
            }
            if unnamed != 1 {
                return attrs::span_err!(input, "structs must have exactly one unnamed field");
            }
            expand_struct(&attrs, fields.iter().next().unwrap())?
        }
        Data::Enum(_) => match &attrs.repr {
            Some(t) => expand_repr_enum(&attrs, &t)?,
            None => expand_enum(&attrs)?,
        },
    })
}

/// An enum with a repr attribute defining the underlying type.
fn expand_repr_enum(
    container: &attrs::ContainerAttributes,
    repr: &Type,
) -> syn::Result<TokenStream> {
    attrs::check_repr_enum_attrs(container)?;
    let ident = &container.ident;
    Ok(quote!(
        #[automatically_derived]
        impl musqlite::Type for #ident
        where
            #repr: musqlite::Type,
        {
            fn type_info() -> musqlite::SqliteDataType {
                <#repr as musqlite::Type>::type_info()
            }

            fn compatible(ty: &musqlite::SqliteDataType) -> bool {
                <#repr as musqlite::Type>::compatible(ty)
            }
        }
    ))
}

/// A plain enum, without a repr attribute. The underlying type is `str`.
fn expand_enum(container: &attrs::ContainerAttributes) -> syn::Result<TokenStream> {
    attrs::check_enum_attrs(container)?;
    let ident = &container.ident;
    Ok(quote!(
        #[automatically_derived]
        impl musqlite::Type for #ident {
            fn type_info() -> musqlite::SqliteDataType {
                <::std::primitive::str as musqlite::Type>::type_info()
            }

            fn compatible(ty: &musqlite::SqliteDataType) -> ::std::primitive::bool {
                <&::std::primitive::str as musqlite::Type>::compatible(ty)
            }
        }
    ))
}

fn expand_struct(
    container: &attrs::ContainerAttributes,
    field: &attrs::FieldAttributes,
) -> syn::Result<TokenStream> {
    attrs::check_transparent_attrs(container)?;

    let (_, ty_generics, _) = container.generics.split_for_impl();

    let ty = &field.ty;
    let ident = &container.ident;
    let mut generics = container.generics.clone();
    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(#ty: musqlite::Type));
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    Ok(quote!(
        #[automatically_derived]
        impl #impl_generics musqlite::Type for #ident #ty_generics #where_clause {
            fn type_info() -> musqlite::SqliteDataType {
                <#ty as musqlite::Type>::type_info()
            }

            fn compatible(ty: &musqlite::SqliteDataType) -> ::std::primitive::bool {
                <#ty as musqlite::Type>::compatible(ty)
            }
        }
    ))
}

#[cfg(test)]

mod tests {
    use super::*;

    macro_rules! assert_errors_with {
        ($e:expr, $m:expr) => {
            assert!(&$e.is_err());
            let e = $e.unwrap_err();
            assert!(
                format!("{}", e).contains($m),
                "expected error containing \"{}\" got \"{}\"",
                $m,
                e
            );
        };
    }

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
            #[musqlite(rename_all = "lower_case", repr = "i32")]
            enum Foo {One, Two}
        "#;
        let e = expand_derive_type(&syn::parse_str(txt).unwrap());
        assert_errors_with!(e, "not supported for enums");

        let txt = r#"
            #[musqlite(transparent)]
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
            #[musqlite(repr = "i32")]
            enum Foo {One, Two}
        "#;
        expand_derive_type(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            #[musqlite(transparent)]
            struct Foo(i32);
        "#;
        expand_derive_type(&syn::parse_str(txt).unwrap()).unwrap();
    }
}
