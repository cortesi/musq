use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Expr, Ident, LitStr, Result as SynResult, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

enum MacroArg {
    Pos(Expr),
    Named(Ident, Expr),
}

impl Parse for MacroArg {
    fn parse(input: ParseStream) -> SynResult<Self> {
        if input.peek(Ident) && input.peek2(Token![=]) {
            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let expr: Expr = input.parse()?;
            Ok(MacroArg::Named(ident, expr))
        } else {
            let expr: Expr = input.parse()?;
            Ok(MacroArg::Pos(expr))
        }
    }
}

struct MacroInput {
    fmt: LitStr,
    _comma: Option<Token![,]>,
    args: syn::punctuated::Punctuated<MacroArg, Token![,]>,
}

impl Parse for MacroInput {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let fmt: LitStr = input.parse()?;
        let mut comma = None;
        if input.peek(Token![,]) {
            comma = Some(input.parse()?);
        }
        let args = syn::punctuated::Punctuated::parse_terminated(input)?;
        Ok(MacroInput {
            fmt,
            _comma: comma,
            args,
        })
    }
}

pub fn expand_sql(input: TokenStream, as_map: bool) -> TokenStream {
    let MacroInput { fmt, args, .. } = parse_macro_input!(input as MacroInput);
    let mut positionals: Vec<Expr> = Vec::new();
    let mut named: std::collections::HashMap<String, Expr> = std::collections::HashMap::new();
    for arg in args {
        match arg {
            MacroArg::Pos(expr) => positionals.push(expr),
            MacroArg::Named(id, expr) => {
                named.insert(id.to_string(), expr);
            }
        }
    }
    let fmt_string = fmt.value();
    let segments = match parse_format_string(&fmt_string) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };
    let builder_ident = format_ident!("__builder");
    let mut quoted = quote! {
        let mut #builder_ident = musq::QueryBuilder::new();
    };
    let mut pos_i = 0usize;
    for seg in segments {
        match seg {
            Segment::Lit(s) => {
                quoted.extend(quote! { #builder_ident.push_sql(#s); });
            }
            Segment::Positional => {
                if pos_i >= positionals.len() {
                    return syn::Error::new_spanned(
                        &fmt,
                        "not enough arguments for positional placeholder",
                    )
                    .to_compile_error()
                    .into();
                }
                let expr = &positionals[pos_i];
                quoted.extend(quote! { #builder_ident.push_bind(#expr); });
                pos_i += 1;
            }
            Segment::Named(name) => {
                let expr = named
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| syn::parse_str::<Expr>(&name).unwrap());
                let name_str = name;
                quoted.extend(quote! { #builder_ident.push_bind_named(#name_str, #expr); });
            }
            Segment::Ident(expr) => {
                quoted.extend(quote! { #builder_ident.push_identifier(#expr); });
            }
            Segment::Values(expr) => {
                quoted.extend(quote! { #builder_ident.push_values(#expr); });
            }
            Segment::Idents(expr) => {
                quoted.extend(quote! { #builder_ident.push_idents(#expr); });
            }
            Segment::Raw(expr) => {
                quoted.extend(quote! { #builder_ident.push_raw(#expr); });
            }
        }
    }
    if pos_i < positionals.len() {
        return syn::Error::new_spanned(&fmt, "too many positional arguments")
            .to_compile_error()
            .into();
    }
    let build_tokens = if as_map {
        quote! { #builder_ident.build_map() }
    } else {
        quote! { #builder_ident.build() }
    };
    let output = quote!({
        let query = { #quoted #build_tokens };
        musq::Result::<_>::Ok(query)
    });
    output.into()
}

enum Segment {
    Lit(String),
    Positional,
    Named(String),
    Ident(Expr),
    Values(Expr),
    Idents(Expr),
    Raw(Expr),
}

fn parse_format_string(s: &str) -> SynResult<Vec<Segment>> {
    let mut chars = s.chars().peekable();
    let mut segments = Vec::new();
    let mut current = String::new();
    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
                current.push('{');
                continue;
            }
            if !current.is_empty() {
                segments.push(Segment::Lit(current.clone()));
                current.clear();
            }
            let mut inner = String::new();
            for ch in &mut chars {
                if ch == '}' {
                    break;
                }
                inner.push(ch);
            }
            if inner.is_empty() {
                segments.push(Segment::Positional);
            } else if let Some(rest) = inner.strip_prefix("ident:") {
                let expr: Expr = syn::parse_str(rest.trim()).unwrap();
                segments.push(Segment::Ident(expr));
            } else if let Some(rest) = inner.strip_prefix("values:") {
                let expr: Expr = syn::parse_str(rest.trim()).unwrap();
                segments.push(Segment::Values(expr));
            } else if let Some(rest) = inner.strip_prefix("idents:") {
                let expr: Expr = syn::parse_str(rest.trim()).unwrap();
                segments.push(Segment::Idents(expr));
            } else if let Some(rest) = inner.strip_prefix("raw:") {
                let expr: Expr = syn::parse_str(rest.trim()).unwrap();
                segments.push(Segment::Raw(expr));
            } else {
                let name = inner.trim().to_string();
                segments.push(Segment::Named(name));
            }
        } else if c == '}' {
            if chars.peek() == Some(&'}') {
                chars.next();
                current.push('}');
            } else {
                return Err(syn::Error::new_spanned(
                    LitStr::new(s, proc_macro2::Span::call_site()),
                    "unmatched `}`",
                ));
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        segments.push(Segment::Lit(current));
    }
    Ok(segments)
}
