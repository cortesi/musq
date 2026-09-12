//! Performance benchmarks for musq.

use criterion::Criterion;
use tokio::runtime::Runtime;

/// SQL schema used by benchmarks.
const BENCH_SCHEMA: &str = include_str!("benchschema.sql");

/// Set min and max pool connections to the same value.
const CONNECTIONS: u32 = 5;

/// Row counts exercised by the read benchmark.
const DATASET_SIZES: [usize; 3] = [10, 100, 1_000];

/// Row type used by benchmark queries.
#[derive(Debug, musq::FromRow)]
pub struct Data {
    /// Integer column.
    pub a: i32,
    /// String column.
    pub b: String,
}

/// Create a pool initialized with the benchmark schema and `rows` rows.
async fn setup_pool(rows: usize) -> musq::Pool {
    let pool = musq::Musq::new()
        .max_connections(CONNECTIONS)
        .open_in_memory()
        .await
        .unwrap();
    musq::query(BENCH_SCHEMA).execute(&pool).await.unwrap();
    for i in 0..rows {
        musq::query("INSERT INTO data (a, b) VALUES (?1, ?2)")
            .bind(i as i32)
            .bind("seed")
            .execute(&pool)
            .await
            .unwrap();
    }
    pool
}

/// Read every row in the dataset.
async fn read_all(pool: &musq::Pool) {
    musq::query_as::<Data>("SELECT * FROM data")
        .fetch_all(pool)
        .await
        .unwrap();
}

/// Insert one row.
async fn write_one(pool: &musq::Pool) {
    musq::query("INSERT INTO data (a, b) VALUES (?1, ?2)")
        .bind(1)
        .bind("two")
        .execute(pool)
        .await
        .unwrap();
}

/// Register benchmarks with Criterion.
pub fn criterion_benchmark(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();
    for rows in DATASET_SIZES {
        let pool = runtime.block_on(setup_pool(rows));

        c.bench_function(&format!("read/{rows}"), |b| {
            b.to_async(&runtime).iter(|| read_all(&pool));
        });

        c.bench_function(&format!("write/{rows}"), |b| {
            b.to_async(&runtime).iter(|| write_one(&pool));
        });
    }
}

/// Criterion benchmark entry point.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    criterion_benchmark(&mut c);
    c.final_summary();
}
