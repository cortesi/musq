use darling::FromDeriveInput;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{DeriveInput, GenericParam, Lifetime, LifetimeParam};

use super::core;

pub fn expand_json(input: &DeriveInput) -> syn::Result<TokenStream> {
    let container = core::JsonContainer::from_derive_input(input)?;
    let (_, ty_generics, _) = container.generics.split_for_impl();

    let ident = &container.ident;
    let generics = container.generics.clone();
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    let mut decode_generics = container.generics.clone();
    let lt = Lifetime::new("'r", Span::call_site());
    let ltp = LifetimeParam::new(lt);
    decode_generics.params.push(GenericParam::from(ltp));
    let (decode_impl_generics, _, _) = decode_generics.split_for_impl();

    Ok(quote!(
        impl #impl_generics musq::encode::Encode for #ident #ty_generics #where_clause {
            fn encode(self) -> musq::Value {
                let v = serde_json::to_string(&self).expect(
                         "failed to encode value as JSON; the most likely cause is \
                         attempting to serialize a map with a non-string key type"
                );
                musq::Value::Text(v.into_bytes(), None)
            }
        }

        impl #decode_impl_generics musq::decode::Decode<'r> for #ident #ty_generics #where_clause {
            fn decode(value: &'r musq::Value) -> std::result::Result<Self, musq::DecodeError> {
                serde_json::from_str(value.text()?).map_err(|x| musq::DecodeError::Conversion(x.to_string().into()))
            }
        }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_derives_json() {
        let txt = r#"
            struct Foo{
                a: i32,
                b: String
            }
        "#;
        println!("{}", expand_json(&syn::parse_str(txt).unwrap()).unwrap());
    }
}
