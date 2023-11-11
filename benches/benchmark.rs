use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use futures;
use pprof::criterion::PProfProfiler;
use tokio::runtime::{Handle, Runtime};

use musq;

const BENCH_SCHEMA: &str = include_str!("benchschema.sql");

/// How many concurrent read or write requests should we make?
const CONCURRENCY: usize = 20;

/// Set min and max pool connections to the same value
const CONNECTIONS: u32 = 5;

#[derive(Debug, musq::FromRow)]
pub struct Data {
    pub a: i32,
    pub b: String,
}

async fn pool() -> musq::Pool {
    let pool = musq::Musq::new()
        .with_pool()
        .max_connections(CONNECTIONS)
        .min_connections(CONNECTIONS)
        .open_in_memory()
        .await
        .unwrap();
    musq::query(BENCH_SCHEMA).execute(&pool).await.unwrap();
    pool
}

fn setup() -> musq::Pool {
    let (tx, rx) = std::sync::mpsc::channel();
    Handle::current().spawn(async move {
        let p = pool().await;
        musq::query("INSERT INTO data (a, b) VALUES (?1, ?2)")
            .bind(1)
            .bind("two")
            .execute(&p)
            .await
            .unwrap();
        tx.send(pool().await).unwrap();
    });
    rx.recv().unwrap()
}

async fn writes(pool: musq::Pool) {
    let mut futs = vec![];
    for _ in 0..CONCURRENCY {
        futs.push(
            musq::query("INSERT INTO data (a, b) VALUES (?1, ?2)")
                .bind(1)
                .bind("two")
                .execute(&pool),
        )
    }
    futures::future::join_all(futs).await;
}

async fn reads(pool: musq::Pool) {
    let mut futs = vec![];
    for _ in 0..CONCURRENCY {
        let f = musq::query_as::<Data>("SELECT * from DATA").fetch_one(&pool);
        futs.push(f)
    }
    futures::future::join_all(futs).await;
}

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

fn criterion() -> Criterion {
    let criterion = Criterion::default().with_profiler(PProfProfiler::new(
        100,
        pprof::criterion::Output::Flamegraph(None),
    ));
    criterion
}

criterion_group! {
    name = benches;
    config = criterion();
    targets = criterion_benchmark
}
criterion_main!(benches);
