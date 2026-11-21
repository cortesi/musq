use darling::FromDeriveInput;
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

use super::core;

/// Expand a `Json` derive into encode/decode implementations.
pub fn expand_json(input: &DeriveInput) -> syn::Result<TokenStream> {
    let container = core::JsonContainer::from_derive_input(input)?;
    let (impl_generics, ty_generics, where_clause) = container.generics.split_for_impl();

    let ident = &container.ident;
    let musq = core::musq_path();

    let mut decode_generics = container.generics.clone();
    let lifetime = core::add_fresh_lifetime(&mut decode_generics);
    let (decode_impl_generics, _, _) = decode_generics.split_for_impl();

    Ok(quote!(
        #[automatically_derived]
        impl #impl_generics #musq::encode::Encode for #ident #ty_generics #where_clause {
            fn encode(&self) -> ::std::result::Result<#musq::Value, #musq::error::EncodeError> {
                let v = #musq::__private::serde_json::to_string(self)
                    .map_err(|e| #musq::error::EncodeError::Conversion(
                        ::std::format!("failed to encode value as JSON: {}", e)
                    ))?;
                ::std::result::Result::Ok(#musq::Value::Text {
                    value: v.into(),
                    type_info: ::std::option::Option::None,
                })
            }
        }

        #[automatically_derived]
        impl #decode_impl_generics #musq::decode::Decode<#lifetime> for #ident #ty_generics #where_clause {
            fn decode(value: &#lifetime #musq::Value) -> ::std::result::Result<Self, #musq::DecodeError> {
                #musq::__private::serde_json::from_str(value.text()?).map_err(|x| {
                    #musq::DecodeError::Conversion(::std::string::ToString::to_string(&x))
                })
            }
        }
    ))
}
