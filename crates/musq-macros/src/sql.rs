// Procedural macros for sql! and sql_as!

use proc_macro::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{
    Expr, Ident, LitStr, Result as SynResult, Token,
    parse::{Parse, ParseStream},
};

fn ensure_non_empty(expr: &Expr) -> SynResult<()> {
    match expr {
        Expr::Array(arr) if arr.elems.is_empty() => {
            Err(syn::Error::new_spanned(expr, "empty list"))
        }
        Expr::Reference(r) => ensure_non_empty(&r.expr),
        Expr::Macro(m) if m.mac.path.is_ident("vec") && m.mac.tokens.is_empty() => {
            Err(syn::Error::new_spanned(expr, "empty list"))
        }
        _ => Ok(()),
    }
}

struct SqlMacroInput {
    fmt: LitStr,
    args: Punctuated<SqlArg, Token![,]>,
}

enum SqlArg {
    Positional(Expr),
    Named(Ident, Expr),
}

impl Parse for SqlMacroInput {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let fmt: LitStr = input.parse()?;
        let mut args = Punctuated::new();
        if input.is_empty() {
            return Ok(Self { fmt, args });
        }
        input.parse::<Token![,]>()?;
        while !input.is_empty() {
            if input.peek(Ident) && input.peek2(Token![=]) {
                let id: Ident = input.parse()?;
                input.parse::<Token![=]>()?;
                let expr: Expr = input.parse()?;
                args.push(SqlArg::Named(id, expr));
            } else {
                let expr: Expr = input.parse()?;
                args.push(SqlArg::Positional(expr));
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
        Ok(Self { fmt, args })
    }
}

#[derive(Debug)]
enum Segment {
    Lit(String),
    Positional,
    Named(String),
    Ident(Expr),
    Values(Expr),
    Idents(Expr),
    Insert(Expr),
    Set(Expr),
    Where(Expr),
    Upsert(Expr, Vec<String>),
    Raw(Expr),
}

struct UpsertArgs {
    values: Expr,
    exclude: Vec<String>,
}

impl Parse for UpsertArgs {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let values: Expr = input.parse()?;
        let mut exclude = Vec::new();
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let ident: Ident = input.parse()?;
            if ident != "exclude" {
                return Err(syn::Error::new_spanned(ident, "expected `exclude`"));
            }
            input.parse::<Token![:]>()?;

            // Parse comma-separated list of identifiers until end of input
            while !input.is_empty() {
                // Try parsing as a regular identifier first, then handle keywords
                let ident_string = if let Ok(ident) = input.parse::<Ident>() {
                    ident.to_string()
                } else {
                    // Handle Rust keywords by parsing as a token and converting to string
                    let lookahead = input.lookahead1();
                    if lookahead.peek(Token![type]) {
                        input.parse::<Token![type]>()?;
                        "type".to_string()
                    } else if lookahead.peek(Token![ref]) {
                        input.parse::<Token![ref]>()?;
                        "ref".to_string()
                    } else if lookahead.peek(Token![let]) {
                        input.parse::<Token![let]>()?;
                        "let".to_string()
                    } else if lookahead.peek(Token![mut]) {
                        input.parse::<Token![mut]>()?;
                        "mut".to_string()
                    } else if lookahead.peek(Token![const]) {
                        input.parse::<Token![const]>()?;
                        "const".to_string()
                    } else if lookahead.peek(Token![static]) {
                        input.parse::<Token![static]>()?;
                        "static".to_string()
                    } else if lookahead.peek(Token![fn]) {
                        input.parse::<Token![fn]>()?;
                        "fn".to_string()
                    } else if lookahead.peek(Token![struct]) {
                        input.parse::<Token![struct]>()?;
                        "struct".to_string()
                    } else if lookahead.peek(Token![enum]) {
                        input.parse::<Token![enum]>()?;
                        "enum".to_string()
                    } else if lookahead.peek(Token![impl]) {
                        input.parse::<Token![impl]>()?;
                        "impl".to_string()
                    } else if lookahead.peek(Token![trait]) {
                        input.parse::<Token![trait]>()?;
                        "trait".to_string()
                    } else if lookahead.peek(Token![mod]) {
                        input.parse::<Token![mod]>()?;
                        "mod".to_string()
                    } else if lookahead.peek(Token![use]) {
                        input.parse::<Token![use]>()?;
                        "use".to_string()
                    } else if lookahead.peek(Token![pub]) {
                        input.parse::<Token![pub]>()?;
                        "pub".to_string()
                    } else if lookahead.peek(Token![crate]) {
                        input.parse::<Token![crate]>()?;
                        "crate".to_string()
                    } else if lookahead.peek(Token![super]) {
                        input.parse::<Token![super]>()?;
                        "super".to_string()
                    } else if lookahead.peek(Token![self]) {
                        input.parse::<Token![self]>()?;
                        "self".to_string()
                    } else if lookahead.peek(Token![Self]) {
                        input.parse::<Token![Self]>()?;
                        "Self".to_string()
                    } else {
                        return Err(lookahead.error());
                    }
                };

                exclude.push(ident_string);

                if input.peek(Token![,]) {
                    input.parse::<Token![,]>()?;
                    // Allow trailing comma - if we're at the end after consuming comma, break
                    if input.is_empty() {
                        break;
                    }
                } else {
                    break;
                }
            }

            // Ensure we have at least one exclude column
            if exclude.is_empty() {
                return Err(syn::Error::new(
                    input.span(),
                    "expected at least one column identifier after 'exclude:'",
                ));
            }
        }
        Ok(Self { values, exclude })
    }
}

fn parse_fmt(s: &str) -> SynResult<Vec<Segment>> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if matches!(chars.peek(), Some('{')) {
                    chars.next();
                    cur.push('{');
                    continue;
                }
                if !cur.is_empty() {
                    out.push(Segment::Lit(cur.clone()));
                    cur.clear();
                }
                let mut ph = String::new();
                let mut closed = false;
                for ch in chars.by_ref() {
                    if ch == '}' {
                        closed = true;
                        break;
                    }
                    ph.push(ch);
                }
                if !closed {
                    return Err(syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "unmatched `{`",
                    ));
                }
                let (kind, expr) = if let Some(idx) = ph.find(':') {
                    let (k, e) = ph.split_at(idx);
                    (k.trim(), Some(e[1..].trim()))
                } else {
                    (ph.trim(), None)
                };
                match (kind, expr) {
                    ("", None) => out.push(Segment::Positional),
                    (
                        "ident" | "values" | "idents" | "insert" | "set" | "where" | "upsert"
                        | "raw",
                        None,
                    ) => {
                        return Err(syn::Error::new(
                            proc_macro2::Span::call_site(),
                            "malformed placeholder",
                        ));
                    }
                    (name, None) => out.push(Segment::Named(name.to_string())),
                    ("ident", Some(e)) => out.push(Segment::Ident(syn::parse_str(e)?)),
                    ("values", Some(e)) => {
                        let expr = syn::parse_str::<Expr>(e)?;
                        ensure_non_empty(&expr)?;
                        out.push(Segment::Values(expr));
                    }
                    ("insert", Some(e)) => {
                        let expr = syn::parse_str::<Expr>(e)?;
                        ensure_non_empty(&expr)?;
                        out.push(Segment::Insert(expr));
                    }
                    ("set", Some(e)) => {
                        let expr = syn::parse_str::<Expr>(e)?;
                        ensure_non_empty(&expr)?;
                        out.push(Segment::Set(expr));
                    }
                    ("where", Some(e)) => {
                        let expr = syn::parse_str::<Expr>(e)?;
                        out.push(Segment::Where(expr));
                    }
                    ("upsert", Some(e)) => {
                        let args = syn::parse_str::<UpsertArgs>(e)?;
                        ensure_non_empty(&args.values)?;
                        out.push(Segment::Upsert(args.values, args.exclude));
                    }
                    ("idents", Some(e)) => {
                        let expr = syn::parse_str::<Expr>(e)?;
                        ensure_non_empty(&expr)?;
                        out.push(Segment::Idents(expr));
                    }
                    ("raw", Some(e)) => out.push(Segment::Raw(syn::parse_str(e)?)),
                    _ => {
                        return Err(syn::Error::new(
                            proc_macro2::Span::call_site(),
                            "malformed placeholder",
                        ));
                    }
                }
            }
            '}' => {
                if matches!(chars.peek(), Some('}')) {
                    chars.next();
                    cur.push('}');
                } else {
                    return Err(syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "unmatched `}`",
                    ));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(Segment::Lit(cur));
    }
    Ok(out)
}

fn build_sql(
    tokens: Vec<Segment>,
    input: SqlMacroInput,
    as_query_as: bool,
) -> SynResult<proc_macro2::TokenStream> {
    let mut positional = Vec::new();
    let mut named = std::collections::HashMap::new();
    for arg in input.args {
        match arg {
            SqlArg::Positional(e) => positional.push(e),
            SqlArg::Named(id, e) => {
                named.insert(id.to_string(), e);
            }
        }
    }
    let mut sql_parts = Vec::<proc_macro2::TokenStream>::new();
    let mut pos_index = 0usize;
    for seg in tokens {
        match seg {
            Segment::Lit(l) => sql_parts.push(quote! { _builder.push_sql(#l); }),
            Segment::Positional => {
                let expr = positional.get(pos_index).cloned().ok_or_else(|| {
                    syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "missing positional argument",
                    )
                })?;
                pos_index += 1;
                sql_parts.push(quote! { _builder.push_bind(#expr)?; });
            }
            Segment::Named(name) => {
                let name_lit = syn::LitStr::new(&name, proc_macro2::Span::call_site());
                if let Some(expr) = named.remove(&name) {
                    sql_parts.push(quote! { _builder.push_bind_named(#name_lit, #expr)?; });
                } else {
                    let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
                    sql_parts.push(quote! { _builder.push_bind_named(#name_lit, #ident)?; });
                }
            }
            Segment::Ident(expr) => {
                sql_parts.push(quote! { _builder.push_sql(&musq::quote_identifier(&(#expr))); })
            }
            Segment::Values(expr) => sql_parts.push(quote! { _builder.push_values(#expr)?; }),
            Segment::Idents(expr) => sql_parts.push(quote! { _builder.push_idents(#expr)?; }),
            Segment::Insert(expr) => sql_parts.push(quote! { _builder.push_insert(&(#expr))?; }),
            Segment::Set(expr) => sql_parts.push(quote! { _builder.push_set(&(#expr))?; }),
            Segment::Where(expr) => sql_parts.push(quote! { _builder.push_where(&(#expr))?; }),
            Segment::Upsert(expr, exclude) => {
                let lits: Vec<syn::LitStr> = exclude
                    .iter()
                    .map(|s| syn::LitStr::new(s, proc_macro2::Span::call_site()))
                    .collect();
                sql_parts.push(quote! { _builder.push_upsert(&(#expr), &[ #( #lits ),* ])?; })
            }
            Segment::Raw(expr) => sql_parts.push(quote! { _builder.push_raw(#expr); }),
        }
    }
    if pos_index != positional.len() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "unused positional arguments",
        ));
    }
    if !named.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "unused named arguments",
        ));
    }
    let builder_inits = quote! { let mut _builder = musq::QueryBuilder::new(); };
    let collects = quote! { #(#sql_parts)* };
    let build = if as_query_as {
        quote! { _builder.build().try_map(|row| musq::FromRow::from_row("", &row)) }
    } else {
        quote! { _builder.build() }
    };
    Ok(quote! {{
        #builder_inits
        (|| -> musq::Result<_> { #collects Ok(#build) })()
    }})
}

pub fn expand_sql(input: TokenStream, as_query_as: bool) -> TokenStream {
    let input = syn::parse_macro_input!(input as SqlMacroInput);
    let fmt = &input.fmt;
    match parse_fmt(&fmt.value()).and_then(|tokens| build_sql(tokens, input, as_query_as)) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

pub fn sql(input: TokenStream) -> TokenStream {
    expand_sql(input, false)
}

pub fn sql_as(input: TokenStream) -> TokenStream {
    expand_sql(input, true)
}
