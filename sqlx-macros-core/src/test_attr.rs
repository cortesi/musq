use proc_macro2::TokenStream;
use quote::quote;

pub fn expand(args: syn::AttributeArgs, input: syn::ItemFn) -> crate::Result<TokenStream> {
    let ret = &input.sig.output;
    let name = &input.sig.ident;
    let body = &input.block;
    let attrs = &input.attrs;
    Ok(quote! {
        #[::core::prelude::v1::test]
        #(#attrs)*
        fn #name() #ret {
            ::sqlx::test_block_on(async { #body })
        }
    })
}
