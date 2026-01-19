use darling::{self, FromDeriveInput, FromField, FromMeta, ast, util};
use heck::{ToKebabCase, ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase};
use proc_macro2::TokenStream;
use syn::{DeriveInput, Type};

/// Create a `syn::Error` from a spanned token.
macro_rules! span_err {
    ($t:expr, $err:expr) => {
        Err(syn::Error::new_spanned($t, $err))
    };
}

/// Re-exported for use in tests.
#[allow(unused)]
pub(crate) use span_err;

#[allow(unused)]
/// Assert that a `syn::Result` error contains the expected message.
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

/// Re-exported for use in tests.
#[allow(unused)]
pub(crate) use assert_errors_with;

/// Case conversion rules for rename attributes.
#[derive(Default, Debug, Copy, Clone, FromMeta)]
pub enum RenameAll {
    /// Convert to snake_case.
    #[default]
    SnakeCase,
    /// Convert to lowercase.
    LowerCase,
    /// Convert to UPPERCASE.
    UpperCase,
    /// Convert to SCREAMING_SNAKE_CASE.
    ScreamingSnakeCase,
    /// Convert to kebab-case.
    KebabCase,
    /// Convert to lowerCamelCase.
    CamelCase,
    /// Convert to UpperCamelCase.
    PascalCase,
    /// Preserve the original spelling.
    Verbatim,
}

impl RenameAll {
    /// Apply the case conversion rule to the provided string.
    pub(crate) fn rename(self, s: &str) -> String {
        match self {
            Self::LowerCase => s.to_lowercase(),
            Self::SnakeCase => s.to_snake_case(),
            Self::UpperCase => s.to_uppercase(),
            Self::ScreamingSnakeCase => s.to_shouty_snake_case(),
            Self::KebabCase => s.to_kebab_case(),
            Self::CamelCase => s.to_lower_camel_case(),
            Self::PascalCase => s.to_upper_camel_case(),
            Self::Verbatim => s.to_owned(),
        }
    }
}

/// Parsed inputs for `Json` derives.
#[derive(Debug, FromDeriveInput)]
#[darling(supports(struct_named, struct_tuple))]
pub struct JsonContainer {
    /// Type identifier.
    pub ident: syn::Ident,
    /// Generic parameters.
    pub generics: syn::Generics,
    /// Parsed input data (ignored for JSON derives).
    #[darling(skip)]
    pub _data: Option<ast::Data<util::Ignored, RowField>>,
}

/// Parsed inputs for `FromRow` derives.
#[derive(Debug, FromDeriveInput)]
#[darling(attributes(musq))]
#[darling(supports(struct_named, struct_tuple))]
pub struct RowContainer {
    /// Type identifier.
    pub ident: syn::Ident,
    /// Generic parameters.
    pub generics: syn::Generics,
    /// Parsed row data.
    pub data: ast::Data<util::Ignored, RowField>,

    /// Rename rule to apply to fields.
    #[darling(default = "Default::default")]
    pub rename_all: RenameAll,
}

/// Parsed attributes for a single row field.
#[derive(Debug, FromField)]
#[darling(attributes(musq))]
pub struct RowField {
    /// Field identifier, if present.
    pub ident: Option<syn::Ident>,
    /// Field type.
    pub ty: Type,

    /// Optional explicit rename.
    pub rename: Option<String>,
    /// Whether the field has a default value.
    #[darling(default = "Default::default")]
    pub default: bool,
    /// Whether this field should be flattened.
    #[darling(default = "Default::default")]
    pub flatten: bool,
    /// Prefix applied when flattening.
    #[darling(default = "Default::default")]
    pub prefix: String,
    /// Optional conversion type.
    pub try_from: Option<Type>,
    /// Whether to skip the field.
    #[darling(default = "Default::default")]
    pub skip: bool,
}

/// Validate that row field attributes are compatible.
pub fn check_row_field_attrs(field: &RowField) -> syn::Result<()> {
    if field.flatten {
        if field.skip {
            span_err!(&field.ty, "`flatten` cannot be combined with `skip`")?;
        }
        if field.try_from.is_some() {
            span_err!(&field.ty, "`flatten` cannot be combined with `try_from`")?;
        }
        if field.rename.is_some() {
            span_err!(&field.ty, "`flatten` cannot be combined with `rename`")?;
        }
    } else if !field.prefix.is_empty() {
        span_err!(&field.ty, "`prefix` requires `flatten`")?;
    }
    Ok(())
}

/// Validate container-level row attributes.
pub fn check_row_attrs(container: &RowContainer) -> syn::Result<()> {
    if let ast::Data::Struct(fields) = &container.data {
        for f in fields.iter() {
            check_row_field_attrs(f)?;
        }
    }
    Ok(())
}

/// Parsed inputs for repr enum derives.
#[derive(Debug, FromDeriveInput)]
#[darling(attributes(musq))]
pub struct TypeContainer {
    /// Type identifier.
    pub ident: syn::Ident,
    /// Generic parameters.
    pub generics: syn::Generics,
    /// Parsed enum data.
    pub data: ast::Data<TypeVariant, TypeField>,

    /// Rename rule to apply to variants.
    #[darling(default = "Default::default")]
    pub rename_all: RenameAll,
    /// Optional representation attribute.
    pub repr: Option<Type>,
}

/// Parsed attributes for a single enum variant.
#[derive(darling::FromVariant, Debug)]
pub struct TypeVariant {
    /// Variant identifier.
    pub ident: syn::Ident,
    /// Fields on the variant, if present.
    #[darling(skip)]
    pub _fields: Option<ast::Fields<TypeField>>,

    /// Optional explicit rename.
    pub rename: Option<String>,
}

/// Returns the canonical name of an enum variant after applying any
/// `rename` attribute or container level `rename_all` rule.
pub fn variant_name(container: &TypeContainer, variant: &TypeVariant) -> String {
    variant
        .rename
        .clone()
        .unwrap_or_else(|| container.rename_all.rename(&variant.ident.to_string()))
}

/// Parsed attributes for a single enum field.
#[derive(Debug, FromField)]
#[darling(attributes(musq))]
pub struct TypeField {
    /// Field identifier, if present.
    pub ident: Option<syn::Ident>,
    /// Field type.
    pub ty: Type,

    /// Optional explicit rename.
    #[darling(skip)]
    pub _rename: Option<String>,
}

/// Ensure repr enums declare a representation attribute.
pub fn check_repr_enum_attrs(attrs: &TypeContainer) -> syn::Result<()> {
    if attrs.repr.is_none() {
        span_err!(&attrs.ident, "repr attribute is required")?;
    }
    Ok(())
}

/// Signature for repr enum expansion callbacks.
type ExpandReprEnumFn = dyn Fn(&TypeContainer, &[TypeVariant], &Type) -> syn::Result<TokenStream>;

/// Expand derive input into the appropriate macro output.
pub fn expand_type_derive(
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
