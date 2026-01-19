//! Procedural macros for the musq crate.

/// Shared helpers used by macro expansion modules.
mod core;
/// Derive support for decoding types from rows.
mod decode;
/// Derive support for encoding types into SQLite values.
mod encode;
/// Derive support for JSON column handling.
mod json;
/// Derive support for row mapping.
mod row;
/// Compile-time SQL helpers.
mod sql;

/// Build combined encode/decode tokens for the `Codec` derive.
fn derive_codec_tokens(input: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let encode_tts = encode::expand_derive_encode(input)?;
    let decode_tts = decode::expand_derive_decode(input)?;
    Ok(proc_macro2::TokenStream::from_iter(
        encode_tts.into_iter().chain(decode_tts),
    ))
}

#[proc_macro_derive(Json, attributes(musq))]
/// Derive JSON encode/decode implementations.
pub fn derive_json(tokenstream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokenstream as syn::DeriveInput);
    match json::expand_json(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(Codec, attributes(musq))]
/// Derive combined encode and decode implementations.
pub fn derive_codec(tokenstream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokenstream as syn::DeriveInput);
    match derive_codec_tokens(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(Encode, attributes(musq))]
/// Derive an `Encode` implementation.
pub fn derive_encode(tokenstream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokenstream as syn::DeriveInput);
    match encode::expand_derive_encode(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(Decode, attributes(musq))]
/// Derive a `Decode` implementation.
pub fn derive_decode(tokenstream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokenstream as syn::DeriveInput);
    match decode::expand_derive_decode(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(FromRow, attributes(musq))]
/// Derive a `FromRow` implementation.
pub fn derive_from_row(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match row::expand_derive_from_row(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(test)]
mod tests;

#[proc_macro]
/// Expand a SQL query from a format string and arguments.
pub fn sql(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    sql::sql(item)
}

#[proc_macro]
/// Expand a SQL query that maps rows into a destination type.
pub fn sql_as(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    sql::sql_as(item)
}
