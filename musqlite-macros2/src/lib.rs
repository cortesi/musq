use proc_macro::TokenStream;

mod attrs;
mod typ;

#[proc_macro_derive(Type, attributes(musqlite))]
pub fn derive_type(tokenstream: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(tokenstream as syn::DeriveInput);
    match typ::expand_derive_type(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
