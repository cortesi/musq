mod core;
mod decode;
mod encode;
mod row;
mod typ;

#[proc_macro_derive(Type, attributes(musqlite))]
pub fn derive_type(tokenstream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokenstream as syn::DeriveInput);
    fn combo(input: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
        let encode_tts = encode::expand_derive_encode(&input)?;
        let decode_tts = decode::expand_derive_decode(&input)?;
        let type_tts = typ::expand_derive_type(&input)?;
        let combined = proc_macro2::TokenStream::from_iter(
            encode_tts.into_iter().chain(decode_tts).chain(type_tts),
        );
        Ok(combined)
    }
    match combo(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(Encode, attributes(musqlite))]
pub fn derive_encode(tokenstream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokenstream as syn::DeriveInput);
    match encode::expand_derive_encode(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(Decode, attributes(musqlite))]
pub fn derive_decode(tokenstream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokenstream as syn::DeriveInput);
    match decode::expand_derive_decode(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(FromRow, attributes(musqlite))]
pub fn derive_from_row(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match row::expand_derive_from_row(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}