use crate::{
    SqliteDataType, Value,
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
};

impl Encode for serde_json::Value {
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

impl<'r> Decode<'r> for serde_json::Value {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
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
            assert_eq!(serde_json::Value::decode(&encoded).unwrap(), expected);
        }
    }

    #[test]
    fn malformed_json_is_rejected() {
        let value = Value::Text {
            value: "{".into(),
            type_info: None,
        };
        assert!(serde_json::Value::decode(&value).is_err());
    }

    #[test]
    fn optional_json_values_round_trip() {
        let expected = Some(json!({"value": 1}));
        let encoded = expected.encode().unwrap();
        assert_eq!(
            Option::<serde_json::Value>::decode(&encoded).unwrap(),
            expected
        );

        let null = Option::<serde_json::Value>::None.encode().unwrap();
        assert_eq!(Option::<serde_json::Value>::decode(&null).unwrap(), None);
    }
}
