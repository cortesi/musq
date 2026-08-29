//! Provides [`Encode`] for encoding values for the database.
use crate::{Value, error::EncodeError};

/// Encode a single value to be sent to the database.
pub trait Encode {
    /// Writes the value of `self` into `buf` in the expected format for the database.
    /// Takes `&self` to avoid consuming the value, allowing for more flexible usage patterns.
    fn encode(&self) -> Result<Value, EncodeError>;
}

impl<T> Encode for &T
where
    T: Encode + ?Sized,
{
    fn encode(&self) -> Result<Value, EncodeError> {
        T::encode(self)
    }
}

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

/// A unit value that encodes as SQL NULL.
///
/// Use this in `values!` blocks when you want NULL without a type annotation.
pub struct Null;

impl Encode for Null {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Null { type_info: None })
    }
}

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
            assert_eq!(value.as_str(), "hello");
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
            assert_eq!(value.as_str(), "world");
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
    fn reference_propagates_encode_errors() {
        struct Bad;

        impl Encode for Bad {
            fn encode(&self) -> Result<Value, EncodeError> {
                Err(EncodeError::Conversion("bad value".into()))
            }
        }

        let value = Bad;
        let error = <&Bad as Encode>::encode(&&value).unwrap_err();
        assert!(error.to_string().contains("bad value"));
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
    fn test_null_unit_struct() {
        let null_val = Null;
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
