use darling::{ast, FromDeriveInput, FromField, FromMeta};
use heck::{ToKebabCase, ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase};
use syn::Type;

macro_rules! span_err {
    ($t:expr, $err:expr) => {
        Err(syn::Error::new_spanned($t, $err))
    };
}

pub(crate) use span_err;

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
#[darling(attributes(musqlite))]
pub struct ContainerAttributes {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    #[darling(default)]
    pub transparent: bool,
    pub rename_all: Option<RenameAll>,
    pub repr: Option<Type>,
    pub data: ast::Data<Variant, FieldAttributes>,
}

#[derive(darling::FromVariant, Debug)]
pub struct Variant {
    pub ident: syn::Ident,
    pub fields: darling::ast::Fields<FieldAttributes>,
    pub rename: Option<String>,
}

#[derive(Debug, FromField)]
#[darling(attributes(musqlite))]
pub struct VariantAttributes {
    pub ty: Type,
}

#[derive(Debug, FromField)]
#[darling(attributes(musqlite))]
pub struct FieldAttributes {
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

pub(crate) fn check_enum_attrs(attrs: &ContainerAttributes) -> syn::Result<()> {
    if attrs.transparent {
        span_err!(&attrs.ident, "transparent is not supported for enums")?;
    }
    Ok(())
}

pub(crate) fn check_repr_enum_attrs(attrs: &ContainerAttributes) -> syn::Result<()> {
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

pub(crate) fn check_transparent_attrs(attrs: &ContainerAttributes) -> syn::Result<()> {
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

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn it_parses_attrs() {
        let good_input = r#"
            #[musqlite(rename_all = "snake_case")]
            pub struct Foo {
                #[musqlite(skip)]
                bar: bool,

                baz: i64,
                }
        "#;
        let parsed = syn::parse_str(good_input).unwrap();
        assert!(ContainerAttributes::from_derive_input(&parsed).is_ok());
    }
}
