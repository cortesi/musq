//! Performance benchmarks for musq.

use std::sync::mpsc;

use criterion::{BatchSize, Criterion};
use futures::future::join_all;
use tokio::runtime::{Handle, Runtime};

/// SQL schema used by benchmarks.
const BENCH_SCHEMA: &str = include_str!("benchschema.sql");

/// How many concurrent read or write requests should we make?
const CONCURRENCY: usize = 20;

/// Set min and max pool connections to the same value
const CONNECTIONS: u32 = 5;

/// Row type used by benchmark queries.
#[derive(Debug, musq::FromRow)]
pub struct Data {
    /// Integer column.
    pub a: i32,
    /// String column.
    pub b: String,
}

/// Create a pool initialized with the benchmark schema.
async fn pool() -> musq::Pool {
    let pool = musq::Musq::new()
        .max_connections(CONNECTIONS)
        .open_in_memory()
        .await
        .unwrap();
    musq::query(BENCH_SCHEMA)
        .execute(&pool.acquire().await.unwrap())
        .await
        .unwrap();
    pool
}

/// Build a pool and insert a seed row for benchmarks.
fn setup() -> musq::Pool {
    let (tx, rx) = mpsc::channel();
    Handle::current().spawn(async move {
        let p = pool().await;
        musq::query("INSERT INTO data (a, b) VALUES (?1, ?2)")
            .bind(1)
            .bind("two")
            .execute(&p.acquire().await.unwrap())
            .await
            .unwrap();
        tx.send(pool().await).unwrap();
    });
    rx.recv().unwrap()
}

/// Run concurrent write workloads.
async fn writes(pool: musq::Pool) {
    let mut futs = vec![];
    for _ in 0..CONCURRENCY {
        let pool = pool.clone();
        futs.push(async move {
            let conn = pool.acquire().await.unwrap();
            musq::query("INSERT INTO data (a, b) VALUES (?1, ?2)")
                .bind(1)
                .bind("two")
                .execute(&conn)
                .await
        });
    }
    join_all(futs).await;
}

/// Run concurrent read workloads.
async fn reads(pool: musq::Pool) {
    let mut futs = vec![];
    for _ in 0..CONCURRENCY {
        let pool = pool.clone();
        futs.push(async move {
            let conn = pool.acquire().await.unwrap();
            musq::query_as::<Data>("SELECT * from DATA")
                .fetch_one(&conn)
                .await
        });
    }
    join_all(futs).await;
}

/// Register benchmarks with Criterion.
pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("write", |b| {
        b.to_async(Runtime::new().unwrap())
            .iter_batched(setup, writes, BatchSize::SmallInput)
    });
    c.bench_function("read", |b| {
        b.to_async(Runtime::new().unwrap())
            .iter_batched(setup, reads, BatchSize::SmallInput)
    });
}

/// Criterion benchmark entry point.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    criterion_benchmark(&mut c);
    c.final_summary();
}
