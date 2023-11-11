use darling::ast::Data;
use darling::FromDeriveInput;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{parse_quote, DeriveInput, Lifetime, LifetimeParam, Type};

use super::core;

pub fn expand_derive_encode(input: &DeriveInput) -> syn::Result<TokenStream> {
    let attrs = core::ContainerAttributes::from_derive_input(input).unwrap();

    Ok(match &attrs.data {
        Data::Struct(fields) => {
            if fields.is_empty() {
                return core::span_err!(input, "structs with zero fields are not supported");
            }
            let unnamed = fields.iter().filter(|f| f.ident.is_none()).count();
            let named = fields.iter().filter(|f| f.ident.is_some()).count();
            if named > 1 {
                return core::span_err!(input, "structs with named fields are not supported");
            }
            if unnamed != 1 {
                return core::span_err!(input, "structs must have exactly one unnamed field");
            }
            expand_struct(&attrs, fields.iter().next().unwrap())?
        }
        Data::Enum(v) => match &attrs.repr {
            Some(t) => expand_repr_enum(&attrs, v, &t)?,
            None => expand_enum(&attrs, v)?,
        },
    })
}

fn expand_enum(
    container: &core::ContainerAttributes,
    variants: &Vec<core::Variant>,
) -> syn::Result<TokenStream> {
    core::check_enum_attrs(container)?;
    let ident = &container.ident;
    let mut value_arms = Vec::new();

    for v in variants {
        let id = &v.ident;
        if let Some(rename) = &v.rename {
            value_arms.push(quote!(#ident :: #id => #rename,));
        } else if let Some(pattern) = container.rename_all {
            let name = core::rename_all(&id.to_string(), pattern);
            value_arms.push(quote!(#ident :: #id => #name,));
        } else {
            let name = id.to_string();
            value_arms.push(quote!(#ident :: #id => #name,));
        }
    }

    Ok(quote!(
        #[automatically_derived]
        impl<'q> musqlite::encode::Encode<'q> for #ident
        where
            &'q ::std::primitive::str: musqlite::encode::Encode<'q>,
        {
            fn encode_by_ref(
                &self,
                buf: &mut musqlite::ArgumentBuffer<'q>,
            ) -> musqlite::encode::IsNull {
                let val = match self {
                    #(#value_arms)*
                };

                <&::std::primitive::str as musqlite::encode::Encode<'q>>::encode(val, buf)
            }
        }
    ))
}

fn expand_repr_enum(
    container: &core::ContainerAttributes,
    variants: &Vec<core::Variant>,
    repr: &Type,
) -> syn::Result<TokenStream> {
    core::check_repr_enum_attrs(container)?;
    let ident = &container.ident;

    let mut values = Vec::new();
    for v in variants {
        let id = &v.ident;
        values.push(quote!(#ident :: #id => (#ident :: #id as #repr),));
    }

    Ok(quote!(
        #[automatically_derived]
        impl<'q> musqlite::encode::Encode<'q> for #ident
        where
            #repr: musqlite::encode::Encode<'q>,
        {
            fn encode_by_ref(
                &self,
                buf: &mut musqlite::ArgumentBuffer<'q>,
            ) -> musqlite::encode::IsNull {
                let value = match self {
                    #(#values)*
                };

                <#repr as musqlite::encode::Encode>::encode_by_ref(&value, buf)
            }
        }
    ))
}

fn expand_struct(
    container: &core::ContainerAttributes,
    field: &core::FieldAttributes,
) -> syn::Result<TokenStream> {
    core::check_transparent_attrs(container)?;

    let ident = &container.ident;
    let ty = &field.ty;

    // extract type generics
    let generics = &container.generics;
    let (_, ty_generics, _) = generics.split_for_impl();

    // add db type for impl generics & where clause
    let lifetime = Lifetime::new("'q", Span::call_site());
    let mut generics = generics.clone();
    generics
        .params
        .insert(0, LifetimeParam::new(lifetime.clone()).into());

    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(#ty: musqlite::encode::Encode<#lifetime>));
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    Ok(quote!(
        #[automatically_derived]
        impl #impl_generics musqlite::encode::Encode<#lifetime> for #ident #ty_generics
        #where_clause
        {
            fn encode_by_ref(
                &self,
                buf: &mut musqlite::ArgumentBuffer<#lifetime>,
            ) -> musqlite::encode::IsNull {
                <#ty as musqlite::encode::Encode<#lifetime>>::encode_by_ref(&self.0, buf)
            }

            fn produces(&self) -> Option<musqlite::SqliteDataType> {
                <#ty as musqlite::encode::Encode<#lifetime>>::produces(&self.0)
            }
        }
    ))
}

#[cfg(test)]

mod tests {
    use super::core::assert_errors_with;
    use super::*;

    #[test]
    fn it_derives_encode() {
        let txt = r#"enum Foo {One, Two}"#;
        expand_derive_encode(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            #[musqlite(rename_all = "lower_case")]
            enum Foo {One, Two}
        "#;
        expand_derive_encode(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            #[musqlite(repr = "i32")]
            enum Foo {One, Two}
        "#;
        expand_derive_encode(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            #[musqlite(transparent)]
            struct Foo(i32);
        "#;
        expand_derive_encode(&syn::parse_str(txt).unwrap()).unwrap();
    }
}
