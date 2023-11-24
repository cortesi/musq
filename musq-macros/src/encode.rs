use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{parse_quote, DeriveInput, Lifetime, LifetimeParam, Type};

use super::core;

pub fn expand_derive_encode(input: &DeriveInput) -> syn::Result<TokenStream> {
    core::expand_type_derive(input, &expand_struct, &expand_repr_enum, &expand_enum)
}

fn expand_enum(
    container: &core::TypeContainer,
    variants: &Vec<core::TypeVariant>,
) -> syn::Result<TokenStream> {
    let ident = &container.ident;
    let mut value_arms = Vec::new();

    for v in variants {
        let id = &v.ident;
        if let Some(rename) = &v.rename {
            value_arms.push(quote!(#ident :: #id => #rename,));
        } else {
            let name = container.rename_all.rename(&id.to_string());
            value_arms.push(quote!(#ident :: #id => #name,));
        }
    }

    Ok(quote!(
        #[automatically_derived]
        impl<'q> musq::encode::Encode<'q> for #ident
        where
            &'q ::std::primitive::str: musq::encode::Encode<'q>,
        {
            fn encode(
                self,
                buf: &mut musq::ArgumentBuffer<'q>,
            ) -> musq::encode::IsNull {
                let val = match self {
                    #(#value_arms)*
                };

                <&::std::primitive::str as musq::encode::Encode<'q>>::encode(val, buf)
            }
        }
    ))
}

fn expand_repr_enum(
    container: &core::TypeContainer,
    variants: &Vec<core::TypeVariant>,
    repr: &Type,
) -> syn::Result<TokenStream> {
    let ident = &container.ident;

    let mut values = Vec::new();
    for v in variants {
        let id = &v.ident;
        values.push(quote!(#ident :: #id => (#ident :: #id as #repr),));
    }

    Ok(quote!(
        #[automatically_derived]
        impl<'q> musq::encode::Encode<'q> for #ident
        where
            #repr: musq::encode::Encode<'q>,
        {
            fn encode(
                self,
                buf: &mut musq::ArgumentBuffer<'q>,
            ) -> musq::encode::IsNull {
                let value = match self {
                    #(#values)*
                };

                <#repr as musq::encode::Encode>::encode(value, buf)
            }
        }
    ))
}

fn expand_struct(
    container: &core::TypeContainer,
    field: &core::TypeField,
) -> syn::Result<TokenStream> {
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
        .push(parse_quote!(#ty: musq::encode::Encode<#lifetime>));
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    Ok(quote!(
        #[automatically_derived]
        impl #impl_generics musq::encode::Encode<#lifetime> for #ident #ty_generics
        #where_clause
        {
            fn encode(
                self,
                buf: &mut musq::ArgumentBuffer<#lifetime>,
            ) -> musq::encode::IsNull {
                <#ty as musq::encode::Encode<#lifetime>>::encode(self.0, buf)
            }
        }
    ))
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn it_derives_encode() {
        let txt = r#"enum Foo {One, Two}"#;
        expand_derive_encode(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            #[musq(rename_all = "lower_case")]
            enum Foo {One, Two}
        "#;
        expand_derive_encode(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            #[musq(repr = "i32")]
            enum Foo {One, Two}
        "#;
        expand_derive_encode(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            struct Foo(i32);
        "#;
        expand_derive_encode(&syn::parse_str(txt).unwrap()).unwrap();
    }
}
