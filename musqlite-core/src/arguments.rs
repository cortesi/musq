//! Types and traits for passing arguments to SQL queries.

use crate::Arguments;

pub trait IntoArguments<'q>: Sized + Send {
    fn into_arguments(self) -> Arguments<'q>;
}

// NOTE: required due to lack of lazy normalization
#[macro_export]
macro_rules! impl_into_arguments_for_arguments {
    ($Arguments:path) => {
        impl<'q> $crate::arguments::IntoArguments<'q> for $Arguments {
            fn into_arguments(self) -> $Arguments {
                self
            }
        }
    };
}

/// used by the query macros to prevent supernumerary `.bind()` calls
pub struct ImmutableArguments<'q>(pub Arguments<'q>);

impl<'q> IntoArguments<'q> for ImmutableArguments<'q> {
    fn into_arguments(self) -> Arguments<'q> {
        self.0
    }
}
