//! A complete custom type with [`musq::encode::Encode`] and [`musq::decode::Decode`].
//! This is nearly identical to the code produced by the built-in Json derive.

use musq::{DecodeError, EncodeError, Value, decode::Decode, encode::Encode};

/// Example JSON-backed record.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct JsonRecord {
    /// Nested text field.
    name: String,
}

impl Encode for JsonRecord {
    fn encode(&self) -> Result<Value, EncodeError> {
        let v = serde_json::to_string(self)
            .map_err(|e| EncodeError::Conversion(format!("failed to encode: {e}")))?;
        Ok(Value::Text {
            value: bytes::Bytes::from(v),
            type_info: None,
        })
    }
}

impl Decode<'_> for JsonRecord {
    fn decode(value: &Value) -> Result<Self, DecodeError> {
        serde_json::from_str(value.text()?)
            .map_err(|error| DecodeError::Conversion(error.to_string()))
    }
}

fn main() {
    let record = JsonRecord {
        name: "sample".into(),
    };
    let value = record.encode().expect("encode JsonRecord");
    let decoded = JsonRecord::decode(&value).expect("decode JsonRecord");
    assert_eq!(record, decoded);
}
