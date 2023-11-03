use super::attributes::{
    check_strong_enum_attributes, check_struct_attributes, check_transparent_attributes,
    check_weak_enum_attributes, parse_child_attributes, parse_container_attributes,
};
use super::rename_all;
use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{
    parse_quote, Arm, Data, DataEnum, DataStruct, DeriveInput, Field, Fields, FieldsNamed,
    FieldsUnnamed, Variant,
};

pub fn expand_derive_decode(input: &DeriveInput) -> syn::Result<TokenStream> {
    let attrs = parse_container_attributes(&input.attrs)?;
    match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Unnamed(FieldsUnnamed { unnamed, .. }),
            ..
        }) if unnamed.len() == 1 => {
            expand_derive_decode_transparent(input, unnamed.first().unwrap())
        }
        Data::Enum(DataEnum { variants, .. }) => match attrs.repr {
            Some(_) => expand_derive_decode_weak_enum(input, variants),
            None => expand_derive_decode_strong_enum(input, variants),
        },
        Data::Struct(DataStruct {
            fields: Fields::Named(FieldsNamed { named, .. }),
            ..
        }) => expand_derive_decode_struct(input, named),
        Data::Union(_) => Err(syn::Error::new_spanned(input, "unions are not supported")),
        Data::Struct(DataStruct {
            fields: Fields::Unnamed(..),
            ..
        }) => Err(syn::Error::new_spanned(
            input,
            "structs with zero or more than one unnamed field are not supported",
        )),
        Data::Struct(DataStruct {
            fields: Fields::Unit,
            ..
        }) => Err(syn::Error::new_spanned(
            input,
            "unit structs are not supported",
        )),
    }
}

fn expand_derive_decode_transparent(
    input: &DeriveInput,
    field: &Field,
) -> syn::Result<TokenStream> {
    check_transparent_attributes(input, field)?;

    let ident = &input.ident;
    let ty = &field.ty;

    // extract type generics
    let generics = &input.generics;
    let (_, ty_generics, _) = generics.split_for_impl();

    // add db type for impl generics & where clause
    let mut generics = generics.clone();
    generics
        .params
        .insert(0, parse_quote!(DB: musqlite_core::Database));
    generics.params.insert(0, parse_quote!('r));
    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(#ty: musqlite_core::decode::Decode<'r>));
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    let tts = quote!(
        #[automatically_derived]
        impl #impl_generics musqlite_core::decode::Decode<'r> for #ident #ty_generics #where_clause {
            fn decode(
                value: musqlite_core::ValueRef<'r>,
            ) -> ::std::result::Result<
                Self,
                ::std::boxed::Box<
                    dyn ::std::error::Error + 'static + ::std::marker::Send + ::std::marker::Sync,
                >,
            > {
                <#ty as musqlite_core::decode::Decode<'r>>::decode(value).map(Self)
            }
        }
    );

    Ok(tts)
}

fn expand_derive_decode_weak_enum(
    input: &DeriveInput,
    variants: &Punctuated<Variant, Comma>,
) -> syn::Result<TokenStream> {
    let attr = check_weak_enum_attributes(input, &variants)?;
    let repr = attr.repr.unwrap();

    let ident = &input.ident;
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
        impl<'r> musqlite_core::decode::Decode<'r> for #ident
        where
            #repr: musqlite_core::decode::Decode<'r>,
        {
            fn decode(
                value: musqlite_core::ValueRef<'r>,
            ) -> ::std::result::Result<
                Self,
                ::std::boxed::Box<
                    dyn ::std::error::Error + 'static + ::std::marker::Send + ::std::marker::Sync,
                >,
            > {
                let value = <#repr as musqlite_core::decode::Decode<'r>>::decode(value)?;

                match value {
                    #(#arms)*
                    _ => ::std::result::Result::Err(::std::boxed::Box::new(musqlite_core::Error::Decode(
                        ::std::format!("invalid value {:?} for enum {}", value, #ident_s).into(),
                    )))
                }
            }
        }
    ))
}

fn expand_derive_decode_strong_enum(
    input: &DeriveInput,
    variants: &Punctuated<Variant, Comma>,
) -> syn::Result<TokenStream> {
    let cattr = check_strong_enum_attributes(input, &variants)?;

    let ident = &input.ident;
    let ident_s = ident.to_string();

    let value_arms = variants.iter().map(|v| -> Arm {
        let id = &v.ident;
        let attributes = parse_child_attributes(&v.attrs).unwrap();

        if let Some(rename) = attributes.rename {
            parse_quote!(#rename => ::std::result::Result::Ok(#ident :: #id),)
        } else if let Some(pattern) = cattr.rename_all {
            let name = rename_all(&*id.to_string(), pattern);

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
        impl<'r> musqlite_core::decode::Decode<'r> for #ident {
            fn decode(
                value: ::sqlite::SqliteValueRef<'r>,
            ) -> ::std::result::Result<
                Self,
                ::std::boxed::Box<
                    dyn ::std::error::Error
                        + 'static
                        + ::std::marker::Send
                        + ::std::marker::Sync,
                >,
            > {
                let value = <&'r ::std::primitive::str as musqlite_core::decode::Decode<
                    'r,
                >>::decode(value)?;

                #values
            }
        }
    ));

    Ok(tts)
}

fn expand_derive_decode_struct(
    input: &DeriveInput,
    fields: &Punctuated<Field, Comma>,
) -> syn::Result<TokenStream> {
    check_struct_attributes(input, fields)?;

    let tts = TokenStream::new();

    Ok(tts)
}
