use renamed_musq::{Codec, Decode, Encode, FromRow, Json, sql, sql_as};

#[derive(FromRow)]
#[allow(dead_code)]
struct Record {
    id: i32,
    name: String,
}

#[derive(Encode, Decode)]
#[musq(repr = "i32")]
enum Kind {
    One,
    Two,
}

#[derive(Json, serde::Deserialize, serde::Serialize)]
struct Payload {
    value: String,
}

#[derive(Codec)]
#[musq(try_from = "String")]
struct Identifier(String);

impl TryFrom<String> for Identifier {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        (!value.is_empty())
            .then_some(Self(value))
            .ok_or("empty identifier")
    }
}

fn main() -> renamed_musq::Result<()> {
    let id = 1_i32;
    let name = "Ada";
    let _query = sql!("SELECT {id}, {}", name)?;
    let _mapped = sql_as!("SELECT id, name FROM records WHERE id = {id}")?
        .map(|record: Record| record);

    let _ = Kind::One;
    let _ = Kind::Two;
    let _ = Payload {
        value: name.to_string(),
    };
    let _ = Record {
        id,
        name: name.to_string(),
    };
    let _ = Identifier::try_from(name.to_string());

    Ok(())
}
