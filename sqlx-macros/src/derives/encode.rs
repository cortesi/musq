use super::attributes::{
    check_strong_enum_attributes, check_struct_attributes, check_transparent_attributes,
    check_weak_enum_attributes, parse_child_attributes, parse_container_attributes,
};
use super::rename_all;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{
    parse_quote, Data, DataEnum, DataStruct, DeriveInput, Field, Fields, FieldsNamed,
    FieldsUnnamed, Lifetime, LifetimeDef, Variant,
};

pub fn expand_derive_encode(input: &DeriveInput) -> syn::Result<TokenStream> {
    let args = parse_container_attributes(&input.attrs)?;

    match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Unnamed(FieldsUnnamed { unnamed, .. }),
            ..
        }) if unnamed.len() == 1 => {
            expand_derive_encode_transparent(&input, unnamed.first().unwrap())
        }
        Data::Enum(DataEnum { variants, .. }) => match args.repr {
            Some(_) => expand_derive_encode_weak_enum(input, variants),
            None => expand_derive_encode_strong_enum(input, variants),
        },
        Data::Struct(DataStruct {
            fields: Fields::Named(FieldsNamed { named, .. }),
            ..
        }) => expand_derive_encode_struct(input, named),
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

fn expand_derive_encode_transparent(
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
    let lifetime = Lifetime::new("'q", Span::call_site());
    let mut generics = generics.clone();
    generics
        .params
        .insert(0, LifetimeDef::new(lifetime.clone()).into());

    generics
        .params
        .insert(0, parse_quote!(DB: sqlx_core::Database));
    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(#ty: sqlx_core::encode::Encode<#lifetime, DB>));
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    Ok(quote!(
        #[automatically_derived]
        impl #impl_generics sqlx_core::encode::Encode<#lifetime, DB> for #ident #ty_generics
        #where_clause
        {
            fn encode_by_ref(
                &self,
                buf: &mut <DB as sqlx_core::database::HasArguments<#lifetime>>::ArgumentBuffer,
            ) -> sqlx_core::encode::IsNull {
                <#ty as sqlx_core::encode::Encode<#lifetime, DB>>::encode_by_ref(&self.0, buf)
            }

            fn produces(&self) -> Option<DB::TypeInfo> {
                <#ty as sqlx_core::encode::Encode<#lifetime, DB>>::produces(&self.0)
            }

            fn size_hint(&self) -> usize {
                <#ty as sqlx_core::encode::Encode<#lifetime, DB>>::size_hint(&self.0)
            }
        }
    ))
}

fn expand_derive_encode_weak_enum(
    input: &DeriveInput,
    variants: &Punctuated<Variant, Comma>,
) -> syn::Result<TokenStream> {
    let attr = check_weak_enum_attributes(input, &variants)?;
    let repr = attr.repr.unwrap();
    let ident = &input.ident;

    let mut values = Vec::new();

    for v in variants {
        let id = &v.ident;
        values.push(quote!(#ident :: #id => (#ident :: #id as #repr),));
    }

    Ok(quote!(
        #[automatically_derived]
        impl<'q, DB: sqlx_core::Database> sqlx_core::encode::Encode<'q, DB> for #ident
        where
            #repr: sqlx_core::encode::Encode<'q, DB>,
        {
            fn encode_by_ref(
                &self,
                buf: &mut <DB as sqlx_core::database::HasArguments<'q>>::ArgumentBuffer,
            ) -> sqlx_core::encode::IsNull {
                let value = match self {
                    #(#values)*
                };

                <#repr as sqlx_core::encode::Encode<DB>>::encode_by_ref(&value, buf)
            }

            fn size_hint(&self) -> usize {
                <#repr as sqlx_core::encode::Encode<DB>>::size_hint(&Default::default())
            }
        }
    ))
}

fn expand_derive_encode_strong_enum(
    input: &DeriveInput,
    variants: &Punctuated<Variant, Comma>,
) -> syn::Result<TokenStream> {
    let cattr = check_strong_enum_attributes(input, &variants)?;

    let ident = &input.ident;

    let mut value_arms = Vec::new();

    for v in variants {
        let id = &v.ident;
        let attributes = parse_child_attributes(&v.attrs)?;

        if let Some(rename) = attributes.rename {
            value_arms.push(quote!(#ident :: #id => #rename,));
        } else if let Some(pattern) = cattr.rename_all {
            let name = rename_all(&*id.to_string(), pattern);

            value_arms.push(quote!(#ident :: #id => #name,));
        } else {
            let name = id.to_string();
            value_arms.push(quote!(#ident :: #id => #name,));
        }
    }

    Ok(quote!(
        #[automatically_derived]
        impl<'q, DB: sqlx_core::Database> sqlx_core::encode::Encode<'q, DB> for #ident
        where
            &'q ::std::primitive::str: sqlx_core::encode::Encode<'q, DB>,
        {
            fn encode_by_ref(
                &self,
                buf: &mut <DB as sqlx_core::database::HasArguments<'q>>::ArgumentBuffer,
            ) -> sqlx_core::encode::IsNull {
                let val = match self {
                    #(#value_arms)*
                };

                <&::std::primitive::str as sqlx_core::encode::Encode<'q, DB>>::encode(val, buf)
            }

            fn size_hint(&self) -> ::std::primitive::usize {
                let val = match self {
                    #(#value_arms)*
                };

                <&::std::primitive::str as sqlx_core::encode::Encode<'q, DB>>::size_hint(&val)
            }
        }
    ))
}

fn expand_derive_encode_struct(
    input: &DeriveInput,
    fields: &Punctuated<Field, Comma>,
) -> syn::Result<TokenStream> {
    check_struct_attributes(input, &fields)?;
    let tts = TokenStream::new();
    Ok(tts)
}
