/// An example showing a complete custom Type implementation, including Encode and Decode. This is nearly identical
/// to the code produced by the built-in Json derive.
use musq::*;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Foo {
    bar: String,
}

impl encode::Encode for Foo {
    fn encode(&self) -> Result<Value, EncodeError> {
        let v = serde_json::to_string(self)
            .map_err(|e| EncodeError::Conversion(format!("failed to encode: {}", e)))?;
        Ok(Value::Text { value: v, type_info: None })
    }
}
impl<'r> decode::Decode<'r> for Foo {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        serde_json::from_str(value.text()?).map_err(|x| DecodeError::Conversion(x.to_string()))
    }
}

fn main() {}
