use darling::{FromDeriveInput, ast};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{DeriveInput, Expr, GenericParam, Lifetime, Stmt, parse_quote};

use super::core;

pub fn expand_derive_from_row(input: &DeriveInput) -> syn::Result<TokenStream> {
    let container = core::RowContainer::from_derive_input(input)?;
    core::check_row_attrs(&container)?;
    Ok(match &container.data {
        ast::Data::Struct(fields) => {
            // We know it's either a named struct or a tuple struct from darling restrictions.
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

fn expand_struct(
    container: &core::RowContainer,
    fields: &ast::Fields<core::RowField>,
) -> syn::Result<TokenStream> {
    let ident = &container.ident;
    let generics = &container.generics;

    let (lifetime, provided) = generics
        .lifetimes()
        .next()
        .map(|def| (def.lifetime.clone(), false))
        .unwrap_or_else(|| (Lifetime::new("'a", Span::call_site()), true));

    let (_, ty_generics, _) = generics.split_for_impl();
    let mut generics = generics.clone();
    if provided {
        let pos = generics
            .params
            .iter()
            .position(|p| !matches!(p, GenericParam::Lifetime(_)))
            .unwrap_or(generics.params.len());
        generics.params.insert(pos, parse_quote!(#lifetime));
    }

    let predicates = &mut generics.make_where_clause().predicates;

    let reads: Vec<Stmt> = fields
        .iter()
        .filter_map(|field| -> Option<Stmt> {
            let id = field.ident.as_ref()?;

            let column_name = field
                .rename
                .clone()
                .or_else(|| Some(id.to_string().trim_start_matches("r#").to_owned()))
                .map(|s| container.rename_all.rename(&s))
                .unwrap();

            let ty = &field.ty;

            if field.skip {
                return Some(parse_quote!(
                    let #id: #ty = Default::default();
                ));
            }

            let expr: Expr = if field.flatten {
                predicates.push(parse_quote!(#ty: musq::FromRow<#lifetime>));
                parse_quote!(<#ty as musq::FromRow<#lifetime>>::from_row("", row))
            } else if !field.prefix.is_empty() {
                predicates.push(parse_quote!(#ty: musq::FromRow<#lifetime>));
                let prefix = &field.prefix;
                parse_quote!(<#ty as musq::FromRow<#lifetime>>::from_row(#prefix, row))
            } else if let Some(try_from) = &field.try_from {
                predicates.push(parse_quote!(#try_from: musq::decode::Decode<#lifetime>));
                parse_quote!(
                    {
                        let column_name = format!("{}{}", prefix, #column_name);
                        let value: musq::Value = row.get_value(&column_name)?;
                        let decoded: #try_from = row.get_value(&column_name)?;
                        <#ty as ::std::convert::TryFrom::<#try_from>>::try_from(decoded).map_err(|e| musq::Error::ColumnDecode {
                            index: String::new(),
                            column_name,
                            value,
                            source: musq::error::DecodeError::Conversion(e.to_string()),
                        })
                    }
                )
            } else {
                predicates.push(parse_quote!(#ty: musq::decode::Decode<#lifetime>));
                parse_quote!(row.get_value(&format!("{}{}", prefix, #column_name)))
            };

            if field.default {
                Some(parse_quote!(
                   let #id: #ty = #expr.or_else(|e| match e {
                       musq::Error::ColumnNotFound(_) => {
                           ::std::result::Result::Ok(Default::default())
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

    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let names = fields.iter().map(|field| &field.ident);

    Ok(quote!(
        #[automatically_derived]
        impl #impl_generics musq::FromRow<#lifetime> for #ident #ty_generics #where_clause {
            fn from_row(prefix: &str, row: &#lifetime musq::Row) -> musq::Result<Self> {
                #(#reads)*

                ::std::result::Result::Ok(#ident {
                    #(#names),*
                })
            }
        }
    ))
}

fn expand_tuple_struct(
    container: &core::RowContainer,
    fields: &ast::Fields<core::RowField>,
) -> syn::Result<TokenStream> {
    let ident = &container.ident;
    let generics = &container.generics;

    let (lifetime, provided) = generics
        .lifetimes()
        .next()
        .map(|def| (def.lifetime.clone(), false))
        .unwrap_or_else(|| (Lifetime::new("'a", Span::call_site()), true));

    let (_, ty_generics, _) = generics.split_for_impl();

    let mut generics = generics.clone();
    if provided {
        let pos = generics
            .params
            .iter()
            .position(|p| !matches!(p, GenericParam::Lifetime(_)))
            .unwrap_or(generics.params.len());
        generics.params.insert(pos, parse_quote!(#lifetime));
    }

    let row_pos = generics.lifetimes().count();
    generics.params.insert(row_pos, parse_quote!(R: musq::Row));

    let predicates = &mut generics.make_where_clause().predicates;

    for field in fields.iter() {
        let ty = &field.ty;

        predicates.push(parse_quote!(#ty: musq::decode::Decode<#lifetime>));
    }

    let (impl_generics, _, where_clause) = generics.split_for_impl();

    let gets = fields
        .iter()
        .enumerate()
        .map(|(idx, _)| quote!(row.get_value_idx(#idx)?));

    Ok(quote!(
        #[automatically_derived]
        impl #impl_generics musq::FromRow<#lifetime> for #ident #ty_generics #where_clause {
            fn from_row(prefix: &str, row: &#lifetime musq::Row) -> musq::Result<Self> {
                ::std::result::Result::Ok(#ident (
                    #(#gets),*
                ))
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
        let e = expand_derive_from_row(&syn::parse_str(txt).unwrap());
        assert_errors_with!(e, "type not supported");

        let txt = r#"struct Unit;"#;
        let e = expand_derive_from_row(&syn::parse_str(txt).unwrap());
        assert_errors_with!(e, "Unsupported shape");
    }

    #[test]
    fn it_derives_row() {
        let txt = r#"
            struct Foo{
                a: i32,
                b: String
            }
        "#;
        println!(
            "{}",
            expand_derive_from_row(&syn::parse_str(txt).unwrap()).unwrap()
        );

        let txt = r#"
            struct Foo(i32, String);
        "#;
        expand_derive_from_row(&syn::parse_str(txt).unwrap()).unwrap();
    }
}
