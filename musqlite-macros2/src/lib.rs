mod core;
mod encode;
mod typ;

#[proc_macro_derive(Type, attributes(musqlite))]
pub fn derive_type(tokenstream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokenstream as syn::DeriveInput);

    fn combo(input: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
        let encode_tts = encode::expand_derive_encode(&input)?;
        let type_tts = typ::expand_derive_type(&input)?;
        let combined = proc_macro2::TokenStream::from_iter(encode_tts.into_iter().chain(type_tts));
        Ok(combined)
    }

    // let encode_tts = encode::expand_derive_encode(&input)?;
    // // let decode_tts = expand_derive_decode(input)?;
    // let type_tts = expand_derive_type(&input)?;
    // let combined = TokenStream::from_iter(encode_tts.into_iter().chain(decode_tts).chain(type_tts));

    match combo(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
