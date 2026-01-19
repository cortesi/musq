/// Define an enum with string representations and a default variant.
macro_rules! enum_mode {
    (
        $(#[$meta:meta])* $vis:vis $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident => $str:expr, )+
        }
        default $default:ident
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        $vis enum $name {
            $( $(#[$vmeta])* $variant, )+
        }

        impl Default for $name {
            fn default() -> Self { Self::$default }
        }

        impl $name {
            pub(crate) fn as_str(&self) -> &'static str {
                match self {
                    $( Self::$variant => $str, )+
                }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}
