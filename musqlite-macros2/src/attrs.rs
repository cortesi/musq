use darling::{ast, util, FromDeriveInput, FromField, FromMeta};
use syn::{Data, Type};

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
    #[darling(default)]
    pub transparent: bool,
    pub type_name: Option<String>,
    pub rename_all: Option<RenameAll>,
    pub repr: Option<Type>,
    pub data: ast::Data<util::Ignored, FieldAttributes>,
}

#[derive(Debug, FromField)]
#[darling(attributes(musqlite))]
pub struct FieldAttributes {
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

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let good_input = r#"
            #[derive(musqlite)]
            #[musqlite(rename_all = "snake_case")]
            pub struct Foo {
                #[musqlite(skip)]
                bar: bool,

                baz: i64,
                }
        "#;

        let parsed = syn::parse_str(good_input).unwrap();
        let receiver = ContainerAttributes::from_derive_input(&parsed).unwrap();
        println!("{:#?}", receiver);
    }
}
