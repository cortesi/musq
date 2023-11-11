use musq::types;
use musq_macros::*;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct Book {
    pub name: String,
}

#[derive(FromRow)]
pub struct Library {
    pub id: String,
    pub dewey_decimal: types::Json<HashMap<String, Book>>,
}

#[tokio::test]
async fn it_derives() -> anyhow::Result<()> {
    Ok(())
}
