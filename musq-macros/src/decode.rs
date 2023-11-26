use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, Arm, DeriveInput, Type};

use super::core;

pub fn expand_derive_decode(input: &DeriveInput) -> syn::Result<TokenStream> {
    core::expand_type_derive(input, &expand_struct, &expand_repr_enum, &expand_enum)
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
    let mut generics = generics.clone();
    generics.params.insert(0, parse_quote!('r));
    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(#ty: musq::decode::Decode<'r>));
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    let tts = quote!(
        #[automatically_derived]
        impl #impl_generics musq::decode::Decode<'r> for #ident #ty_generics #where_clause {
            fn decode(
                value: &'r musq::Value,
            ) -> ::std::result::Result<
                Self,
                musq::DecodeError,
            > {
                <#ty as musq::decode::Decode<'r>>::decode(value).map(Self)
            }
        }
    );

    Ok(tts)
}

fn expand_repr_enum(
    container: &core::TypeContainer,
    variants: &[core::TypeVariant],
    repr: &Type,
) -> syn::Result<TokenStream> {
    let ident = &container.ident;
    let ident_s = ident.to_string();

    let arms = variants
        .iter()
        .map(|v| {
            let id = &v.ident;
            parse_quote! {
                _ if (#ident::#id as #repr) == value => ::std::result::Result::Ok(#ident::#id),
            }
        })
        .collect::<Vec<Arm>>();

    Ok(quote!(
        #[automatically_derived]
        impl<'r> musq::decode::Decode<'r> for #ident
        where
            #repr: musq::decode::Decode<'r>,
        {
            fn decode(
                value: &'r musq::Value,
            ) -> ::std::result::Result<
                Self,
                musq::DecodeError,
            > {
                let value = <#repr as musq::decode::Decode<'r>>::decode(value)?;
                match value {
                    #(#arms)*
                    _ => Err(musq::DecodeError::Conversion(
                        ::std::format!("invalid value {:?} for enum {}", value, #ident_s).into(),
                    ))
                }
            }
        }
    ))
}

fn expand_enum(
    container: &core::TypeContainer,
    variants: &[core::TypeVariant],
) -> syn::Result<TokenStream> {
    let ident = &container.ident;
    let ident_s = ident.to_string();

    let value_arms = variants.iter().map(|v| -> Arm {
        let id = &v.ident;
        if let Some(rename) = &v.rename {
            parse_quote!(#rename => ::std::result::Result::Ok(#ident :: #id),)
        } else {
            let name = container.rename_all.rename(&id.to_string());

            parse_quote!(#name => ::std::result::Result::Ok(#ident :: #id),)
        }
    });

    let values = quote! {
        match value {
            #(#value_arms)*
            _ => Err(format!("invalid value {:?} for enum {}", value, #ident_s).into())
        }
    };

    let mut tts = TokenStream::new();

    tts.extend(quote!(
        #[automatically_derived]
        impl<'r> musq::decode::Decode<'r> for #ident {
            fn decode(
                value: &'r ::musq::Value,
            ) -> ::std::result::Result<
                Self,
                musq::DecodeError,
            > {
                let value = <&'r ::std::primitive::str as musq::decode::Decode<
                    'r,
                >>::decode(value)?;

                #values
            }
        }
    ));

    Ok(tts)
}

mod tests {
    // Rust spuriously detects this particular import as unused?? Remove once this is fixed.
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn it_derives_decode() {
        let txt = r#"enum Foo {One, Two}"#;
        expand_derive_decode(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            #[musq(rename_all = "lower_case")]
            enum Foo {One, Two}
        "#;
        expand_derive_decode(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            #[musq(repr = "i32")]
            enum Foo {One, Two}
        "#;
        expand_derive_decode(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            struct Foo(i32);
        "#;
        expand_derive_decode(&syn::parse_str(txt).unwrap()).unwrap();
    }
}
