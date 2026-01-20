//! Provides [`Encode`] for encoding values for the database.
use crate::{Value, error::EncodeError};

/// Encode a single value to be sent to the database.
pub trait Encode {
    /// Writes the value of `self` into `buf` in the expected format for the database.
    /// Takes `&self` to avoid consuming the value, allowing for more flexible usage patterns.
    fn encode(&self) -> Result<Value, EncodeError>;
}

/// Marker trait for primitive types that can be encoded by reference
pub trait PrimitiveEncode: Encode + Copy + 'static {}

// Implement PrimitiveEncode for all our primitive types
impl PrimitiveEncode for bool {}
impl PrimitiveEncode for i8 {}
impl PrimitiveEncode for i16 {}
impl PrimitiveEncode for i32 {}
impl PrimitiveEncode for i64 {}
impl PrimitiveEncode for u8 {}
impl PrimitiveEncode for u16 {}
impl PrimitiveEncode for u32 {}
impl PrimitiveEncode for f32 {}
impl PrimitiveEncode for f64 {}

// Blanket implementation for primitive types - now they can be encoded by reference directly
// This implementation is no longer needed since Encode now takes &self

impl<T> Encode for Option<T>
where
    T: Encode,
{
    fn encode(&self) -> Result<Value, EncodeError> {
        if let Some(v) = self {
            v.encode()
        } else {
            Ok(Value::Null { type_info: None })
        }
    }
}

impl<T> Encode for &Option<T>
where
    T: Encode,
{
    fn encode(&self) -> Result<Value, EncodeError> {
        if let Some(v) = self {
            v.encode()
        } else {
            Ok(Value::Null { type_info: None })
        }
    }
}

/// A convenience type alias for inserting NULL values without type annotations.
///
/// This is useful when you want to insert NULL values in `values!` blocks
/// without having to specify a particular type like `None::<String>`.
///
/// # Example
///
/// ```rust,no_run
/// use musq::{values, Null};
///
/// let user_data = values! {
///     "name": "Alice",
///     "middle_name": Null,  // Convenient NULL without type annotation
///     "email": "alice@example.com"
/// }?;
/// # Ok::<(), musq::Error>(())
/// ```
pub type Null = Option<bool>;

/// A convenient constant for inserting NULL values.
#[allow(non_upper_case_globals)]
pub const Null: Null = None;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;

    #[test]
    fn test_option_some_i32() {
        let opt: Option<i32> = Some(42);
        let encoded = opt.encode().unwrap();

        if let Value::Integer { value, type_info } = encoded {
            assert_eq!(value, 42);
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Integer value, got {:?}", encoded);
        }
    }

    #[test]
    fn test_option_none_i32() {
        let opt: Option<i32> = None;
        let encoded = opt.encode().unwrap();

        if let Value::Null { type_info } = encoded {
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Null value, got {:?}", encoded);
        }
    }

    #[test]
    fn test_option_some_string() {
        let opt: Option<String> = Some("hello".to_string());
        let encoded = opt.encode().unwrap();

        if let Value::Text { value, type_info } = encoded {
            assert_eq!(value.as_ref(), b"hello");
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Text value, got {:?}", encoded);
        }
    }

    #[test]
    fn test_option_none_string() {
        let opt: Option<String> = None;
        let encoded = opt.encode().unwrap();

        if let Value::Null { type_info } = encoded {
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Null value, got {:?}", encoded);
        }
    }

    #[test]
    fn test_option_some_bool() {
        let opt: Option<bool> = Some(true);
        let encoded = opt.encode().unwrap();

        if let Value::Integer { value, type_info } = encoded {
            assert_eq!(value, 1);
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Integer value for bool, got {:?}", encoded);
        }
    }

    #[test]
    fn test_option_none_bool() {
        let opt: Option<bool> = None;
        let encoded = opt.encode().unwrap();

        if let Value::Null { type_info } = encoded {
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Null value, got {:?}", encoded);
        }
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_option_some_f64() {
        let opt: Option<f64> = Some(3.14);
        let encoded = opt.encode().unwrap();

        if let Value::Double { value, type_info } = encoded {
            assert_eq!(value, 3.14);
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Double value, got {:?}", encoded);
        }
    }

    #[test]
    fn test_option_none_f64() {
        let opt: Option<f64> = None;
        let encoded = opt.encode().unwrap();

        if let Value::Null { type_info } = encoded {
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Null value, got {:?}", encoded);
        }
    }

    #[test]
    fn test_option_some_str_reference() {
        let opt: Option<&str> = Some("world");
        let encoded = opt.encode().unwrap();

        if let Value::Text { value, type_info } = encoded {
            assert_eq!(value.as_ref(), b"world");
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Text value, got {:?}", encoded);
        }
    }

    #[test]
    fn test_option_none_str_reference() {
        let opt: Option<&str> = None;
        let encoded = opt.encode().unwrap();

        if let Value::Null { type_info } = encoded {
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Null value, got {:?}", encoded);
        }
    }

    #[test]
    fn test_nested_option() {
        // Test that Option<Option<T>> works correctly (though unusual)
        let opt: Option<Option<i32>> = Some(Some(100));
        let encoded = opt.encode().unwrap();

        if let Value::Integer { value, type_info } = encoded {
            assert_eq!(value, 100);
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Integer value, got {:?}", encoded);
        }
    }

    #[test]
    fn test_nested_option_inner_none() {
        let opt: Option<Option<i32>> = Some(None);
        let encoded = opt.encode().unwrap();

        if let Value::Null { type_info } = encoded {
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Null value, got {:?}", encoded);
        }
    }

    #[test]
    fn test_nested_option_outer_none() {
        let opt: Option<Option<i32>> = None;
        let encoded = opt.encode().unwrap();

        if let Value::Null { type_info } = encoded {
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Null value, got {:?}", encoded);
        }
    }

    #[test]
    fn test_null_constant() {
        let encoded = Null.encode().unwrap();

        if let Value::Null { type_info } = encoded {
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Null value, got {:?}", encoded);
        }
    }

    #[test]
    fn test_null_alias_type() {
        let null_val: Null = None;
        let encoded = null_val.encode().unwrap();

        if let Value::Null { type_info } = encoded {
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Null value, got {:?}", encoded);
        }
    }

    #[test]
    fn test_null_reference() {
        let encoded = Null.encode().unwrap();

        if let Value::Null { type_info } = encoded {
            assert_eq!(type_info, None);
        } else {
            panic!("Expected Null value, got {:?}", encoded);
        }
    }
}
