/// An example showing a complete custom Type implementation, including Encode and Decode. This is nearly identical
/// to the code produced by the built-in Json derive.
use musq::*;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Foo {
    bar: String,
}

impl encode::Encode for Foo {
    fn encode(self, buf: &mut ArgumentBuffer) -> encode::IsNull {
        let v = serde_json::to_string(&self).expect("failed to encode");
        buf.push(ArgumentValue::Text(std::sync::Arc::new(v)));
        encode::IsNull::No
    }
}
impl<'r> decode::Decode<'r> for Foo {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        serde_json::from_str(value.text()?)
            .map_err(|x| DecodeError::Conversion(x.to_string().into()))
    }
}

fn main() {}
