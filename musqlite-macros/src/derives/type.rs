use super::attributes::{
    check_transparent_attributes, check_weak_enum_attributes, parse_container_attributes,
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{
    parse_quote, Data, DataEnum, DataStruct, DeriveInput, Field, Fields, FieldsNamed,
    FieldsUnnamed, Variant,
};

pub fn expand_derive_type(input: &DeriveInput) -> syn::Result<TokenStream> {
    let attrs = parse_container_attributes(&input.attrs)?;
    match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Unnamed(FieldsUnnamed { unnamed, .. }),
            ..
        }) if unnamed.len() == 1 => {
            expand_derive_has_sql_type_transparent(input, unnamed.first().unwrap())
        }
        Data::Enum(DataEnum { variants, .. }) => match attrs.repr {
            Some(_) => expand_derive_has_sql_type_weak_enum(input, variants),
            None => expand_derive_has_sql_type_strong_enum(input, variants),
        },
        Data::Struct(DataStruct {
            fields: Fields::Named(FieldsNamed { named, .. }),
            ..
        }) => expand_derive_has_sql_type_struct(input, named),
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

fn expand_derive_has_sql_type_transparent(
    input: &DeriveInput,
    field: &Field,
) -> syn::Result<TokenStream> {
    let attr = check_transparent_attributes(input, field)?;

    let ident = &input.ident;
    let ty = &field.ty;

    let generics = &input.generics;
    let (_, ty_generics, _) = generics.split_for_impl();

    if attr.transparent {
        let mut generics = generics.clone();

        generics
            .params
            .insert(0, parse_quote!(DB: musqlite_core::Database));
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#ty: musqlite_core::Type<DB>));
        let (impl_generics, _, where_clause) = generics.split_for_impl();

        let tokens = quote!(
            #[automatically_derived]
            impl #impl_generics musqlite_core::Type< DB > for #ident #ty_generics #where_clause {
                fn type_info() -> sqlite::TypeInfo {
                    <#ty as musqlite_core::Type<DB>>::type_info()
                }

                fn compatible(ty: &musqlite_core::sqlite::TypeInfo) -> ::std::primitive::bool {
                    <#ty as musqlite_core::Type<DB>>::compatible(ty)
                }
            }
        );

        return Ok(tokens);
    }

    Ok(TokenStream::new())
}

fn expand_derive_has_sql_type_weak_enum(
    input: &DeriveInput,
    variants: &Punctuated<Variant, Comma>,
) -> syn::Result<TokenStream> {
    let attr = check_weak_enum_attributes(input, variants)?;
    let repr = attr.repr.unwrap();
    let ident = &input.ident;
    let ts = quote!(
        #[automatically_derived]
        impl<DB: musqlite_core::Database> musqlite_core::Type<DB> for #ident
        where
            #repr: musqlite_core::Type<DB>,
        {
            fn type_info() -> musqlite_core::sqlite::TypeInfo {
                <#repr as musqlite_core::Type<DB>>::type_info()
            }

            fn compatible(ty: &musqlite_core::sqlite::TypeInfo) -> bool {
                <#repr as musqlite_core::Type<DB>>::compatible(ty)
            }
        }
    );

    Ok(ts)
}

fn expand_derive_has_sql_type_strong_enum(
    input: &DeriveInput,
    variants: &Punctuated<Variant, Comma>,
) -> syn::Result<TokenStream> {
    let ident = &input.ident;
    let mut tts = TokenStream::new();
    tts.extend(quote!(
        #[automatically_derived]
        impl Type<::Sqlite> for #ident {
            fn type_info() -> musqlite_core::sqlite::TypeInfo {
                <::std::primitive::str as musqlite_core::Type<Sqlite>>::type_info()
            }

            fn compatible(ty: &musqlite_core::sqlite::TypeInfo) -> ::std::primitive::bool {
                <&::std::primitive::str as ::types::Type<sqlite::Sqlite>>::compatible(ty)
            }
        }
    ));
    Ok(tts)
}

fn expand_derive_has_sql_type_struct(
    input: &DeriveInput,
    fields: &Punctuated<Field, Comma>,
) -> syn::Result<TokenStream> {
    let tts = TokenStream::new();
    Ok(tts)
}
