use darling::{FromDeriveInput, FromField, FromMeta, ast, util};
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

#[derive(Default, Debug, Copy, Clone, FromMeta)]
pub enum RenameAll {
    #[default]
    SnakeCase,
    LowerCase,
    UpperCase,
    ScreamingSnakeCase,
    KebabCase,
    CamelCase,
    PascalCase,
    Verbatim,
}

impl RenameAll {
    pub(crate) fn rename(self, s: &str) -> String {
        match self {
            RenameAll::LowerCase => s.to_lowercase(),
            RenameAll::SnakeCase => s.to_snake_case(),
            RenameAll::UpperCase => s.to_uppercase(),
            RenameAll::ScreamingSnakeCase => s.to_shouty_snake_case(),
            RenameAll::KebabCase => s.to_kebab_case(),
            RenameAll::CamelCase => s.to_lower_camel_case(),
            RenameAll::PascalCase => s.to_upper_camel_case(),
            RenameAll::Verbatim => s.to_owned(),
        }
    }
}

#[derive(Debug, FromDeriveInput)]
#[darling(supports(struct_named, struct_tuple))]
pub struct JsonContainer {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    #[darling(skip)]
    pub _data: Option<ast::Data<util::Ignored, RowField>>,
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(musq))]
#[darling(supports(struct_named, struct_tuple))]
pub struct RowContainer {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    pub data: ast::Data<util::Ignored, RowField>,

    #[darling(default)]
    pub rename_all: RenameAll,
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
    #[darling(default)]
    pub prefix: String,
    pub try_from: Option<Type>,
    #[darling(default)]
    pub skip: bool,
}

pub(crate) fn check_row_field_attrs(field: &RowField) -> syn::Result<()> {
    if field.flatten {
        if field.skip {
            span_err!(&field.ty, "`flatten` cannot be combined with `skip`")?;
        }
        if field.try_from.is_some() {
            span_err!(&field.ty, "`flatten` cannot be combined with `try_from`")?;
        }
        if !field.prefix.is_empty() {
            span_err!(&field.ty, "`flatten` cannot be combined with `prefix`")?;
        }
        if field.rename.is_some() {
            span_err!(&field.ty, "`flatten` cannot be combined with `rename`")?;
        }
    }
    Ok(())
}

pub(crate) fn check_row_attrs(container: &RowContainer) -> syn::Result<()> {
    if let ast::Data::Struct(fields) = &container.data {
        for f in fields.iter() {
            check_row_field_attrs(f)?;
        }
    }
    Ok(())
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(musq))]
pub struct TypeContainer {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    pub data: ast::Data<TypeVariant, TypeField>,

    #[darling(default)]
    pub rename_all: RenameAll,
    pub repr: Option<Type>,
}

#[derive(darling::FromVariant, Debug)]
pub struct TypeVariant {
    pub ident: syn::Ident,
    #[darling(skip)]
    pub _fields: Option<darling::ast::Fields<TypeField>>,

    pub rename: Option<String>,
}

#[derive(Debug, FromField)]
#[darling(attributes(musq))]
pub struct TypeField {
    pub ident: Option<syn::Ident>,
    pub ty: Type,

    #[darling(skip)]
    pub _rename: Option<String>,
}

pub(crate) fn check_repr_enum_attrs(attrs: &TypeContainer) -> syn::Result<()> {
    if attrs.repr.is_none() {
        span_err!(&attrs.ident, "repr attribute is required")?;
    }
    Ok(())
}

type ExpandReprEnumFn = dyn Fn(&TypeContainer, &[TypeVariant], &Type) -> syn::Result<TokenStream>;

pub(crate) fn expand_type_derive(
    input: &DeriveInput,
    expand_struct: &dyn Fn(&TypeContainer, &TypeField) -> syn::Result<TokenStream>,
    expand_repr_enum: &ExpandReprEnumFn,
    expand_enum: &dyn Fn(&TypeContainer, &[TypeVariant]) -> syn::Result<TokenStream>,
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
            expand_struct(&attrs, fields.iter().next().unwrap())?
        }
        ast::Data::Enum(v) => match &attrs.repr {
            Some(t) => {
                check_repr_enum_attrs(&attrs)?;
                expand_repr_enum(&attrs, v, t)?
            }
            None => expand_enum(&attrs, v)?,
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
