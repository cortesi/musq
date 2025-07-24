#![allow(dead_code)]

use musq::{FromRow, Musq, sql, sql_as};

// START - Type Handling section (Json derive)
#[derive(musq::Json, serde::Serialize, serde::Deserialize, Debug, PartialEq)]
struct Metadata {
    tags: Vec<String>,
    version: i32,
}
// END - Type Handling section (Json derive)

#[derive(FromRow, Debug)]
struct Document {
    id: i32,
    title: String,
    metadata: Metadata,
}

#[tokio::main]
async fn main() -> musq::Result<()> {
    let pool = Musq::new().open_in_memory().await?;

    // Create table with JSON column stored as TEXT
    sql!(
        "CREATE TABLE documents (
        id INTEGER PRIMARY KEY,
        title TEXT NOT NULL,
        metadata TEXT NOT NULL
    );"
    )?
    .execute(&pool)
    .await?;

    // Create test data
    let metadata = Metadata {
        tags: vec![
            "rust".to_string(),
            "database".to_string(),
            "async".to_string(),
        ],
        version: 1,
    };

    let doc_id = 1;
    let title = "Musq Documentation";

    // Insert document with JSON metadata
    sql!("INSERT INTO documents (id, title, metadata) VALUES ({doc_id}, {title}, {metadata})")?
        .execute(&pool)
        .await?;

    // Query back and verify JSON serialization/deserialization works
    let document: Document =
        sql_as!("SELECT id, title, metadata FROM documents WHERE id = {doc_id}")?
            .fetch_one(&pool)
            .await?;

    println!("Retrieved document: {document:?}");

    // Verify the metadata was correctly serialized and deserialized
    assert_eq!(document.metadata.tags, vec!["rust", "database", "async"]);
    assert_eq!(document.metadata.version, 1);

    println!("JSON derive working correctly!");

    Ok(())
}
