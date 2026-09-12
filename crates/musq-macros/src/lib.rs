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

/// Run a derive expansion and convert errors into compile errors.
fn derive(
    input: proc_macro::TokenStream,
    expand: impl FnOnce(&syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream>,
) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match expand(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Build combined encode/decode tokens for the `Codec` derive.
fn derive_codec_tokens(input: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let encode_tts = encode::expand_codec_encode(input)?;
    let decode_tts = decode::expand_derive_decode(input)?;
    Ok(proc_macro2::TokenStream::from_iter(
        encode_tts.into_iter().chain(decode_tts),
    ))
}

/// Derive JSON encode/decode implementations.
///
/// The expanded code needs the musq `json` feature.
#[proc_macro_derive(Json, attributes(musq))]
pub fn derive_json(tokenstream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive(tokenstream, json::expand_json)
}

/// Derive combined encode and decode implementations.
#[proc_macro_derive(Codec, attributes(musq))]
pub fn derive_codec(tokenstream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive(tokenstream, derive_codec_tokens)
}

/// Derive an `Encode` implementation.
#[proc_macro_derive(Encode, attributes(musq))]
pub fn derive_encode(tokenstream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive(tokenstream, encode::expand_derive_encode)
}

/// Derive a `Decode` implementation.
#[proc_macro_derive(Decode, attributes(musq))]
pub fn derive_decode(tokenstream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive(tokenstream, decode::expand_derive_decode)
}

/// Derive a `FromRow` implementation.
#[proc_macro_derive(FromRow, attributes(musq))]
pub fn derive_from_row(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive(input, row::expand_derive_from_row)
}

#[cfg(test)]
mod tests;

/// Expand a SQL query from a format string and arguments.
#[proc_macro]
pub fn sql(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    sql::sql(item)
}

/// Expand a SQL query that maps rows into a destination type.
#[proc_macro]
pub fn sql_as(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    sql::sql_as(item)
}
