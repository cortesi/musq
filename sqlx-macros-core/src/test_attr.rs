use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::LitStr;

struct Args {
    fixtures: Vec<LitStr>,
    migrations: MigrationsOpt,
}

enum MigrationsOpt {
    InferredPath,
    ExplicitPath(LitStr),
    ExplicitMigrator(syn::Path),
    Disabled,
}

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
