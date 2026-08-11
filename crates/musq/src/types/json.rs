use std::result::Result as StdResult;

use serde_json::Value as JsonValue;

use crate::{
    SqliteDataType, Value,
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
};

impl Encode for JsonValue {
    fn encode(&self) -> Result<Value, EncodeError> {
        let value = serde_json::to_string(self).map_err(|error| {
            EncodeError::Conversion(format!("failed to encode value as JSON: {error}"))
        })?;
        Ok(Value::Text {
            value: value.into(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for JsonValue {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        serde_json::from_str(value.text()?)
            .map_err(|error| DecodeError::Conversion(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn json_values_round_trip() {
        for expected in [
            json!(null),
            json!(true),
            json!(42),
            json!([1, "two"]),
            json!({"nested": {"value": 3}}),
        ] {
            let encoded = expected.encode().unwrap();
            assert_eq!(JsonValue::decode(&encoded).unwrap(), expected);
        }
    }

    #[test]
    fn malformed_json_is_rejected() {
        let value = Value::Text {
            value: "{".into(),
            type_info: None,
        };
        assert!(JsonValue::decode(&value).is_err());
    }

    #[test]
    fn optional_json_values_round_trip() {
        let expected = Some(json!({"value": 1}));
        let encoded = expected.encode().unwrap();
        assert_eq!(Option::<JsonValue>::decode(&encoded).unwrap(), expected);

        let null = Option::<JsonValue>::None.encode().unwrap();
        assert_eq!(Option::<JsonValue>::decode(&null).unwrap(), None);
    }
}
