use renamed_musq::{Decode, Encode, FromRow, Json, sql, sql_as};

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

    Ok(())
}
