use darling::{ast, FromDeriveInput};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{parse_quote, DeriveInput, Expr, Lifetime, Stmt};

use super::core;

pub fn expand_derive_from_row(input: &DeriveInput) -> syn::Result<TokenStream> {
    let container = core::RowContainer::from_derive_input(input)?;
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
        generics.params.insert(0, parse_quote!(#lifetime));
    }

    let predicates = &mut generics.make_where_clause().predicates;
    predicates.push(parse_quote!(&#lifetime ::std::primitive::str: musq::ColumnIndex<musq::Row>));

    let reads: Vec<Stmt> = fields
            .iter()
            .filter_map(|field| -> Option<Stmt> {
                let id = &field.ident.as_ref()?;
                let ty = &field.ty;

                if field.skip {
                    return Some(parse_quote!(
                        let #id: #ty = Default::default();
                    ));
                }

                let expr: Expr = match (field.flatten, &field.try_from) {
                    (true, None) => {
                        predicates.push(parse_quote!(#ty: musq::FromRow<#lifetime>));
                        parse_quote!(<#ty as musq::FromRow<#lifetime>>::from_row(row))
                    }
                    (false, None) => {
                        predicates
                            .push(parse_quote!(#ty: musq::decode::Decode<#lifetime>));
                        predicates.push(parse_quote!(#ty: musq::types::Type));

                        let id_s = field
                            .rename.clone()
                            .or_else(|| Some(id.to_string().trim_start_matches("r#").to_owned()))
                            .map(|s| match container.rename_all {
                                Some(pattern) => core::rename_all(&s, pattern),
                                None => s,
                            })
                            .unwrap();
                        parse_quote!(row.get_value(#id_s))
                    }
                    (true,Some(try_from)) => {
                        predicates.push(parse_quote!(#try_from: musq::FromRow<#lifetime>));
                        parse_quote!(<#try_from as musq::FromRow<#lifetime>>::from_row(row).and_then(|v| <#ty as ::std::convert::TryFrom::<#try_from>>::try_from(v).map_err(|e| musq::Error::ColumnNotFound("FromRow: try_from failed".to_string()))))
                    }
                    (false,Some(try_from)) => {
                        predicates
                            .push(parse_quote!(#try_from: musq::decode::Decode<#lifetime>));
                        predicates.push(parse_quote!(#try_from: musq::types::Type));

                        let id_s = field
                            .rename.clone()
                            .or_else(|| Some(id.to_string().trim_start_matches("r#").to_owned()))
                            .map(|s| match container.rename_all {
                                Some(pattern) => core::rename_all(&s, pattern),
                                None => s,
                            })
                            .unwrap();
                        parse_quote!(row.get_value(#id_s).and_then(|v| <#ty as ::std::convert::TryFrom::<#try_from>>::try_from(v).map_err(|e| musq::Error::ColumnNotFound("FromRow: try_from failed".to_string()))))
                    }
                };

                if field.default {
                    Some(parse_quote!(let #id: #ty = #expr.or_else(|e| match e {
                    musq::Error::ColumnNotFound(_) => {
                        ::std::result::Result::Ok(Default::default())
                    },
                    e => ::std::result::Result::Err(e)
                })?;))
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
            fn from_row(row: &#lifetime musq::Row) -> musq::Result<Self> {
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
    generics.params.insert(0, parse_quote!(R: musq::Row));

    if provided {
        generics.params.insert(0, parse_quote!(#lifetime));
    }

    let predicates = &mut generics.make_where_clause().predicates;

    predicates.push(parse_quote!(
        ::std::primitive::usize: musq::ColumnIndex<musq::Row>
    ));

    for field in fields.iter() {
        let ty = &field.ty;

        predicates.push(parse_quote!(#ty: musq::decode::Decode<#lifetime>));
        predicates.push(parse_quote!(#ty: musq::types::Type));
    }

    let (impl_generics, _, where_clause) = generics.split_for_impl();

    let gets = fields
        .iter()
        .enumerate()
        .map(|(idx, _)| quote!(row.get_value(#idx)?));

    Ok(quote!(
        #[automatically_derived]
        impl #impl_generics musq::FromRow<#lifetime> for #ident #ty_generics #where_clause {
            fn from_row(row: &#lifetime musq::Row) -> musq::Result<Self> {
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
        expand_derive_from_row(&syn::parse_str(txt).unwrap()).unwrap();

        let txt = r#"
            struct Foo(i32, String);
        "#;
        expand_derive_from_row(&syn::parse_str(txt).unwrap()).unwrap();
    }
}
