use darling::{FromDeriveInput, ast};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Expr, Stmt, parse_quote};

use super::core;

/// Return the database column name for a named row field.
fn column_name(container: &core::RowContainer, field: &core::RowField) -> String {
    match &field.rename {
        Some(name) => name.clone(),
        None => {
            let id = field.ident.as_ref().expect("named field");
            container
                .rename_all
                .rename(id.to_string().trim_start_matches("r#"))
        }
    }
}

/// Expand a `FromRow` derive into the corresponding implementation.
pub fn expand_derive_from_row(input: &DeriveInput) -> syn::Result<TokenStream> {
    let container = core::RowContainer::from_derive_input(input)?;
    core::check_row_attrs(&container)?;
    Ok(match &container.data {
        ast::Data::Struct(fields) => {
            // We know it's either a named struct or a tuple struct from darling
            // restrictions.
            let unnamed = fields.iter().filter(|f| f.ident.is_none()).count();
            let named = fields.iter().filter(|f| f.ident.is_some()).count();
            if unnamed > 0 {
                expand_tuple_struct(&container, fields)?
            } else if named > 0 {
                expand_struct(&container, fields)?
            } else {
                return Err(syn::Error::new_spanned(input, "type not supported"));
            }
        }
        _ => return Err(syn::Error::new_spanned(input, "type not supported")),
    })
}

/// Expand a named-struct `FromRow` implementation.
fn expand_struct(
    container: &core::RowContainer,
    fields: &ast::Fields<core::RowField>,
) -> syn::Result<TokenStream> {
    let ident = &container.ident;
    let generics = &container.generics;
    let musq = core::musq_path();

    let (_, ty_generics, _) = generics.split_for_impl();
    let mut generics = generics.clone();
    let lifetime = match core::first_lifetime(&generics) {
        Some(lifetime) => lifetime,
        None => core::add_fresh_lifetime(&mut generics),
    };

    let predicates = &mut generics.make_where_clause().predicates;

    let reads: Vec<Stmt> = fields
        .iter()
        .filter_map(|field| -> Option<Stmt> {
            let id = field.ident.as_ref()?;

            let column_name = column_name(container, field);

            let ty = &field.ty;

            if field.skip {
                return Some(parse_quote!(
                    let #id: #ty = ::std::default::Default::default();
                ));
            }

            let expr: Expr = if field.flatten {
                predicates.push(parse_quote!(#ty: #musq::FromRow<#lifetime>));
                predicates.push(parse_quote!(#ty: #musq::AllNull<#lifetime>));
                if field.prefix.is_empty() {
                    parse_quote!(<#ty as #musq::FromRow<#lifetime>>::from_row("", row))
                } else {
                    let prefix = &field.prefix;
                    parse_quote!(<#ty as #musq::FromRow<#lifetime>>::from_row(#prefix, row))
                }
            } else if let Some(try_from) = &field.try_from {
                predicates.push(parse_quote!(#try_from: #musq::decode::Decode<#lifetime>));
                parse_quote!(
                    {
                        let column_name = ::std::format!("{}{}", prefix, #column_name);
                        let decoded: #try_from = row.get_value(&column_name)?;
                        <#ty as ::std::convert::TryFrom::<#try_from>>::try_from(decoded).map_err(|e| #musq::Error::ColumnDecode {
                            index: ::std::string::String::new(),
                            column_name: column_name.clone(),
                            value: row
                                .get_value::<#musq::Value>(&column_name)
                                .unwrap_or(#musq::Value::Null { type_info: None }),
                            source: #musq::error::DecodeError::Conversion(::std::string::ToString::to_string(&e)),
                        })
                    }
                )
            } else if let Some(fn_path) = &field.deserialize_with {
                // Custom deserialization function. The function must have signature:
                // fn(prefix: &str, row: &musq::Row) -> musq::Result<T>
                parse_quote!(#fn_path(prefix, row))
            } else {
                predicates.push(parse_quote!(#ty: #musq::decode::Decode<#lifetime>));
                parse_quote!(row.get_value(&::std::format!("{}{}", prefix, #column_name)))
            };

            if field.default {
                Some(parse_quote!(
                   let #id: #ty = #expr.or_else(|e| match e {
                       #musq::Error::ColumnNotFound(_) => {
                           ::std::result::Result::Ok(::std::default::Default::default())
                       },
                       e => ::std::result::Result::Err(e)
                   })?;
                ))
            } else {
                Some(parse_quote!(
                    let #id: #ty = #expr?;
                ))
            }
        })
        .collect();

    let null_checks: Vec<Expr> = fields
        .iter()
        .filter_map(|field| -> Option<Expr> {
            field.ident.as_ref()?;

            let column_name = column_name(container, field);

            let ty = &field.ty;

            if field.skip {
                return None;
            }

            let expr: Expr = if field.flatten {
                predicates.push(parse_quote!(#ty: #musq::AllNull<#lifetime>));
                if field.prefix.is_empty() {
                    parse_quote!(<#ty as #musq::AllNull<#lifetime>>::all_null("", row)?)
                } else {
                    let prefix = &field.prefix;
                    parse_quote!(<#ty as #musq::AllNull<#lifetime>>::all_null(#prefix, row)?)
                }
            } else if let Some(try_from) = &field.try_from {
                predicates.push(parse_quote!(#try_from: #musq::decode::Decode<#lifetime>));
                parse_quote!({
                    match row.get_value::<Option<#try_from>>(&::std::format!("{}{}", prefix, #column_name)) {
                        ::std::result::Result::Ok(v) => v.is_none(),
                        ::std::result::Result::Err(#musq::Error::ColumnNotFound(_)) => true,
                        ::std::result::Result::Err(e) => return ::std::result::Result::Err(e),
                    }
                })
            } else if field.deserialize_with.is_some() {
                // Custom deserialization fields are never considered null since
                // we don't know what the function does.
                parse_quote!(false)
            } else {
                predicates.push(parse_quote!(#ty: #musq::decode::Decode<#lifetime>));
                parse_quote!({
                    match row.get_value::<Option<#ty>>(&::std::format!("{}{}", prefix, #column_name)) {
                        ::std::result::Result::Ok(v) => v.is_none(),
                        ::std::result::Result::Err(#musq::Error::ColumnNotFound(_)) => true,
                        ::std::result::Result::Err(e) => return ::std::result::Result::Err(e),
                    }
                })
            };

            Some(expr)
        })
        .collect();

    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let names = fields.iter().map(|field| &field.ident);

    Ok(quote!(
        #[automatically_derived]
        impl #impl_generics #musq::FromRow<#lifetime> for #ident #ty_generics #where_clause {
            fn from_row(prefix: &str, row: &#lifetime #musq::Row) -> #musq::Result<Self> {
                #(#reads)*

                ::std::result::Result::Ok(#ident {
                    #(#names),*
                })
            }
        }

        #[automatically_derived]
        impl #impl_generics #musq::AllNull<#lifetime> for #ident #ty_generics #where_clause {
            fn all_null(prefix: &str, row: &#lifetime #musq::Row) -> #musq::Result<bool> {
                ::std::result::Result::Ok(true #(&& (#null_checks))* )
            }
        }
    ))
}

/// Expand a tuple-struct `FromRow` implementation.
fn expand_tuple_struct(
    container: &core::RowContainer,
    fields: &ast::Fields<core::RowField>,
) -> syn::Result<TokenStream> {
    let ident = &container.ident;
    let generics = &container.generics;
    let musq = core::musq_path();

    let (_, ty_generics, _) = generics.split_for_impl();

    let mut generics = generics.clone();
    let lifetime = match core::first_lifetime(&generics) {
        Some(lifetime) => lifetime,
        None => core::add_fresh_lifetime(&mut generics),
    };

    let predicates = &mut generics.make_where_clause().predicates;

    for field in fields.iter() {
        let ty = &field.ty;

        predicates.push(parse_quote!(#ty: #musq::decode::Decode<#lifetime>));
    }

    let (impl_generics, _, where_clause) = generics.split_for_impl();

    let gets = fields
        .iter()
        .enumerate()
        .map(|(idx, _)| quote!(row.get_value_idx(#idx)?));

    let null_gets = fields.iter().enumerate().map(|(idx, field)| {
        let ty = &field.ty;
        quote!(row.get_value_idx::<Option<#ty>>(#idx)?.is_none())
    });

    Ok(quote!(
        #[automatically_derived]
        impl #impl_generics #musq::FromRow<#lifetime> for #ident #ty_generics #where_clause {
            fn from_row(prefix: &str, row: &#lifetime #musq::Row) -> #musq::Result<Self> {
                let _ = prefix;
                ::std::result::Result::Ok(#ident (
                    #(#gets),*
                ))
            }
        }

        #[automatically_derived]
        impl #impl_generics #musq::AllNull<#lifetime> for #ident #ty_generics #where_clause {
            fn all_null(prefix: &str, row: &#lifetime #musq::Row) -> #musq::Result<bool> {
                let _ = prefix;
                ::std::result::Result::Ok(true #(&& (#null_gets))* )
            }
        }
    ))
}
