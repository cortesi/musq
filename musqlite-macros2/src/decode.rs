use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, Arm, DeriveInput, Type};

use super::core;

pub fn expand_derive_decode(input: &DeriveInput) -> syn::Result<TokenStream> {
    core::expand_type_derive(input, &expand_struct, &expand_repr_enum, &expand_enum)
}

fn expand_struct(
    container: &core::ContainerAttributes,
    field: &core::FieldAttributes,
) -> syn::Result<TokenStream> {
    let ident = &container.ident;
    let ty = &field.ty;

    // extract type generics
    let generics = &container.generics;
    let (_, ty_generics, _) = generics.split_for_impl();

    // add db type for impl generics & where clause
    let mut generics = generics.clone();
    generics
        .params
        .insert(0, parse_quote!(DB: musqlite::Database));
    generics.params.insert(0, parse_quote!('r));
    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(#ty: musqlite::decode::Decode<'r>));
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    let tts = quote!(
        #[automatically_derived]
        impl #impl_generics musqlite::decode::Decode<'r> for #ident #ty_generics #where_clause {
            fn decode(
                value: musqlite::ValueRef<'r>,
            ) -> ::std::result::Result<
                Self,
                ::std::boxed::Box<
                    dyn ::std::error::Error + 'static + ::std::marker::Send + ::std::marker::Sync,
                >,
            > {
                <#ty as musqlite::decode::Decode<'r>>::decode(value).map(Self)
            }
        }
    );

    Ok(tts)
}

fn expand_repr_enum(
    container: &core::ContainerAttributes,
    variants: &Vec<core::Variant>,
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
        impl<'r> musqlite::decode::Decode<'r> for #ident
        where
            #repr: musqlite::decode::Decode<'r>,
        {
            fn decode(
                value: musqlite::ValueRef<'r>,
            ) -> ::std::result::Result<
                Self,
                ::std::boxed::Box<
                    dyn ::std::error::Error + 'static + ::std::marker::Send + ::std::marker::Sync,
                >,
            > {
                let value = <#repr as musqlite::decode::Decode<'r>>::decode(value)?;

                match value {
                    #(#arms)*
                    _ => ::std::result::Result::Err(::std::boxed::Box::new(musqlite::Error::Decode(
                        ::std::format!("invalid value {:?} for enum {}", value, #ident_s).into(),
                    )))
                }
            }
        }
    ))
}

fn expand_enum(
    container: &core::ContainerAttributes,
    variants: &Vec<core::Variant>,
) -> syn::Result<TokenStream> {
    let ident = &container.ident;
    let ident_s = ident.to_string();

    let value_arms = variants.iter().map(|v| -> Arm {
        let id = &v.ident;
        if let Some(rename) = &v.rename {
            parse_quote!(#rename => ::std::result::Result::Ok(#ident :: #id),)
        } else if let Some(pattern) = container.rename_all {
            let name = core::rename_all(&id.to_string(), pattern);

            parse_quote!(#name => ::std::result::Result::Ok(#ident :: #id),)
        } else {
            let name = id.to_string();
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
        impl<'r> musqlite::decode::Decode<'r> for #ident {
            fn decode(
                value: ::musqlite::ValueRef<'r>,
            ) -> ::std::result::Result<
                Self,
                ::std::boxed::Box<
                    dyn ::std::error::Error
                        + 'static
                        + ::std::marker::Send
                        + ::std::marker::Sync,
                >,
            > {
                let value = <&'r ::std::primitive::str as musqlite::decode::Decode<
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
            #[musqlite(rename_all = "lower_case")]
            enum Foo {One, Two}
        "#;
        expand_derive_decode(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            #[musqlite(repr = "i32")]
            enum Foo {One, Two}
        "#;
        expand_derive_decode(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            #[musqlite(transparent)]
            struct Foo(i32);
        "#;
        expand_derive_decode(&syn::parse_str(txt).unwrap()).unwrap();
    }
}
