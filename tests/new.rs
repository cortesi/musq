#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct Foo {
    a: i32,
    b: String,
}

#[automatically_derived]
impl musq::Type for Foo {
    fn type_info() -> musq::SqliteDataType {
        musq::SqliteDataType::Text
    }
    fn compatible(ty: &musq::SqliteDataType) -> bool {
        <&str as musq::Type>::compatible(ty)
    }
}
impl musq::encode::Encode for Foo {
    fn encode(self, buf: &mut musq::ArgumentBuffer) -> musq::encode::IsNull {
        let v = serde_json::to_string(&self).expect(
            "failed to encode value as JSON; the most likely cause is \
                         attempting to serialize a map with a non-string key type",
        );
        buf.push(musq::ArgumentValue::Text(std::sync::Arc::new(v)));
        musq::encode::IsNull::No
    }
}
impl<'r> musq::decode::Decode<'r> for Foo {
    fn decode(value: &'r musq::Value) -> Result<Self, musq::DecodeError> {
        serde_json::from_str(value.text()?).map_err(|x| musq::DecodeError(x.to_string().into()))
    }
}
