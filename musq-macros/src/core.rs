use darling::{ast, util, FromDeriveInput, FromField, FromMeta};
use heck::{ToKebabCase, ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase};
use proc_macro2::TokenStream;
use syn::{DeriveInput, Type};

macro_rules! span_err {
    ($t:expr, $err:expr) => {
        Err(syn::Error::new_spanned($t, $err))
    };
}

#[allow(unused)]
pub(crate) use span_err;

#[allow(unused)]
macro_rules! assert_errors_with {
    ($e:expr, $m:expr) => {
        assert!(&$e.is_err());
        let e = $e.unwrap_err();
        assert!(
            format!("{}", e).contains($m),
            "expected error containing \"{}\" got \"{}\"",
            $m,
            e
        );
    };
}

#[allow(unused)]
pub(crate) use assert_errors_with;

#[derive(Debug, Copy, Clone, FromMeta)]
pub enum RenameAll {
    LowerCase,
    SnakeCase,
    UpperCase,
    ScreamingSnakeCase,
    KebabCase,
    CamelCase,
    PascalCase,
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(musq))]
#[darling(supports(struct_named, struct_tuple))]
pub struct RowContainer {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    pub rename_all: Option<RenameAll>,
    pub data: ast::Data<util::Ignored, RowField>,
}

#[derive(Debug, FromField)]
#[darling(attributes(musq))]
pub struct RowField {
    pub ident: Option<syn::Ident>,
    pub ty: Type,
    pub rename: Option<String>,
    #[darling(default)]
    pub default: bool,
    #[darling(default)]
    pub flatten: bool,
    pub try_from: Option<Type>,
    #[darling(default)]
    pub skip: bool,
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(musq))]
pub struct TypeContainer {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    #[darling(default)]
    pub transparent: bool,
    pub rename_all: Option<RenameAll>,
    pub repr: Option<Type>,
    pub data: ast::Data<TypeVariant, TypeField>,
}

#[derive(darling::FromVariant, Debug)]
pub struct TypeVariant {
    pub ident: syn::Ident,
    pub fields: darling::ast::Fields<TypeField>,
    pub rename: Option<String>,
}

#[derive(Debug, FromField)]
#[darling(attributes(musq))]
pub struct TypeField {
    pub ident: Option<syn::Ident>,
    pub ty: Type,
    pub rename: Option<String>,
}

pub(crate) fn check_enum_attrs(attrs: &TypeContainer) -> syn::Result<()> {
    if attrs.transparent {
        span_err!(&attrs.ident, "transparent is not supported for enums")?;
    }
    Ok(())
}

pub(crate) fn check_repr_enum_attrs(attrs: &TypeContainer) -> syn::Result<()> {
    check_enum_attrs(attrs)?;
    if attrs.rename_all.is_some() {
        span_err!(
            &attrs.ident,
            "rename_all is not supported for enums with repr"
        )?;
    }
    if attrs.repr.is_none() {
        span_err!(&attrs.ident, "repr attribute is required")?;
    }
    Ok(())
}

pub(crate) fn check_transparent_attrs(attrs: &TypeContainer) -> syn::Result<()> {
    if !attrs.transparent {
        span_err!(&attrs.ident, "transparent is required")?;
    }
    if attrs.rename_all.is_some() {
        span_err!(
            &attrs.ident,
            "rename_all is not supported for transparent structs"
        )?;
    }
    Ok(())
}

pub(crate) fn rename_all(s: &str, pattern: RenameAll) -> String {
    match pattern {
        RenameAll::LowerCase => s.to_lowercase(),
        RenameAll::SnakeCase => s.to_snake_case(),
        RenameAll::UpperCase => s.to_uppercase(),
        RenameAll::ScreamingSnakeCase => s.to_shouty_snake_case(),
        RenameAll::KebabCase => s.to_kebab_case(),
        RenameAll::CamelCase => s.to_lower_camel_case(),
        RenameAll::PascalCase => s.to_upper_camel_case(),
    }
}

pub(crate) fn expand_type_derive(
    input: &DeriveInput,
    expand_struct: &dyn Fn(&TypeContainer, &TypeField) -> syn::Result<TokenStream>,
    expand_repr_enum: &dyn Fn(&TypeContainer, &Vec<TypeVariant>, &Type) -> syn::Result<TokenStream>,
    expand_enum: &dyn Fn(&TypeContainer, &Vec<TypeVariant>) -> syn::Result<TokenStream>,
) -> syn::Result<TokenStream> {
    let attrs = TypeContainer::from_derive_input(input)?;
    Ok(match &attrs.data {
        ast::Data::Struct(fields) => {
            if fields.is_empty() {
                return span_err!(input, "structs with zero fields are not supported");
            }
            let unnamed = fields.iter().filter(|f| f.ident.is_none()).count();
            let named = fields.iter().filter(|f| f.ident.is_some()).count();
            if named > 1 {
                return span_err!(input, "structs with named fields are not supported");
            }
            if unnamed != 1 {
                return span_err!(input, "structs must have exactly one unnamed field");
            }
            check_transparent_attrs(&attrs)?;
            expand_struct(&attrs, fields.iter().next().unwrap())?
        }
        ast::Data::Enum(v) => match &attrs.repr {
            Some(t) => {
                check_repr_enum_attrs(&attrs)?;
                expand_repr_enum(&attrs, v, &t)?
            }
            None => {
                check_enum_attrs(&attrs)?;
                expand_enum(&attrs, v)?
            }
        },
    })
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn it_parses_type_attrs() {
        let good_input = r#"
            #[musq(rename_all = "snake_case")]
            pub struct Foo {
                #[rename(bar)]
                bar: bool,
                baz: i64,
            }
        "#;
        let parsed = syn::parse_str(good_input).unwrap();
        assert!(TypeContainer::from_derive_input(&parsed).is_ok());
    }
}
