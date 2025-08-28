use std::sync::Arc;

use crate::{
    SqliteDataType, Value,
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
};

impl Encode for &str {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Text {
            value: self.to_string(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for &'r str {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text()
    }
}

impl Encode for &String {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Text {
            value: (*self).clone(),
            type_info: None,
        })
    }
}

impl Encode for String {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Text {
            value: self.clone(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for String {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text().map(ToOwned::to_owned)
    }
}

impl Encode for Arc<String> {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Text {
            value: self.as_ref().clone(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for Arc<String> {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text().map(|x| Arc::new(x.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_reference_encode() {
        let value = String::from("hello");
        let result = value.encode().unwrap();
        if let Value::Text { value: encoded, .. } = result {
            assert_eq!(encoded, "hello");
        } else {
            panic!("Expected Text value");
        }
    }

    #[test]
    fn test_ref_pattern_like_user_code() {
        // Simulate the pattern: RemoveFilter::Tag(ref tag) => sql!("DELETE FROM tags WHERE tag = {tag}")
        let tag = String::from("test_tag");
        let ref_tag = &tag; // This is what "ref tag" creates

        // This should now work without the Copy error
        let result = ref_tag.encode().unwrap();
        if let Value::Text { value: encoded, .. } = result {
            assert_eq!(encoded, "test_tag");
        } else {
            panic!("Expected Text value");
        }
    }
}
