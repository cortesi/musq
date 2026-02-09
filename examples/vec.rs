// sqlite-vec usage example.
//
// Run with:
// `cargo run -p musq --example vec`

#[cfg(feature = "vec")]
use musq::{Musq, VecBit, VecF32, VecInt8};

#[cfg(feature = "vec")]
#[tokio::main]
async fn main() -> musq::Result<()> {
    let pool = Musq::new().open_in_memory().await?;

    musq::query("CREATE VIRTUAL TABLE items USING vec0(embedding float[3])")
        .execute(&pool)
        .await?;

    musq::query("INSERT INTO items(rowid, embedding) VALUES (?, ?)")
        .bind(1_i64)
        .bind(VecF32(vec![1.0, 0.0, 0.0]))
        .execute(&pool)
        .await?;

    musq::query("INSERT INTO items(rowid, embedding) VALUES (?, ?)")
        .bind(2_i64)
        .bind(VecF32(vec![0.0, 1.0, 0.0]))
        .execute(&pool)
        .await?;

    let neighbors: Vec<(i64, f64)> = musq::query_as(
        "SELECT rowid, distance
         FROM items
         WHERE embedding MATCH ?
         ORDER BY distance
         LIMIT 2",
    )
    .bind(VecF32(vec![0.0, 1.0, 0.0]))
    .fetch_all(&pool)
    .await?;

    println!("nearest neighbors: {neighbors:?}");

    // Subtype-sensitive wrappers for int8 and bit vectors.
    let int8_type: String = musq::query_scalar("SELECT vec_type(vec_int8(?))")
        .bind(VecInt8(vec![-128, -1, 0, 127]))
        .fetch_one(&pool)
        .await?;
    let bit_type: String = musq::query_scalar("SELECT vec_type(vec_bit(?))")
        .bind(VecBit(vec![0b1010_1010, 0b0101_0101]))
        .fetch_one(&pool)
        .await?;

    println!("int8 type: {int8_type}, bit type: {bit_type}");
    let _ = pool.close().await;
    Ok(())
}

#[cfg(not(feature = "vec"))]
fn main() {
    eprintln!("This example requires the `vec` feature (enabled by default).");
    eprintln!(
        "If you disabled default features, run: cargo run -p musq --example vec --features vec"
    );
}
