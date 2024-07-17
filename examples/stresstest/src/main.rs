use clap::Parser;
use futures::stream::{self, StreamExt};
use musq::{Error, Pool};
use rand::Rng;
use std::path::PathBuf;
use std::time::Instant;
use tempfile::TempDir;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the SQLite database file
    #[arg(short, long)]
    database: Option<PathBuf>,

    /// Number of records to insert
    #[arg(short, long, default_value_t = 100000)]
    records: u64,

    /// Degree of concurrency for inserts
    #[arg(short, long, default_value_t = 1)]
    concurrency: usize,

    /// Size of the blob value in bytes
    #[arg(short, long, default_value_t = 250)]
    blob_size: usize,
}

async fn setup_database(path: &PathBuf) -> Result<Pool, Error> {
    let musq = musq::Musq::new()
        .create_if_missing(true)
        .journal_mode(musq::JournalMode::Wal)
        .synchronous(musq::Synchronous::Normal);

    musq.open(path).await
}

async fn create_schema(pool: &Pool) -> Result<(), Error> {
    musq::query(
        "CREATE TABLE IF NOT EXISTS records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            value BLOB NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    musq::query("CREATE INDEX IF NOT EXISTS idx_records_name ON records (name)")
        .execute(pool)
        .await?;

    Ok(())
}

fn generate_random_blob(size: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen::<u8>()).collect()
}

async fn insert_record(pool: &Pool, name: &str, value: &[u8]) -> Result<(), Error> {
    musq::query("INSERT INTO records (name, value) VALUES (?, ?)")
        .bind(name)
        .bind(value)
        .execute(pool)
        .await?;
    Ok(())
}

async fn insert_records(
    pool: &Pool,
    num_records: u64,
    concurrency: usize,
    blob_size: usize,
) -> Result<(), Error> {
    let start = Instant::now();

    stream::iter(0..num_records)
        .map(|i| {
            let name = format!("Record {}", i);
            let value = generate_random_blob(blob_size);
            (name, value)
        })
        .for_each_concurrent(concurrency, |(name, value)| async move {
            if let Err(e) = insert_record(pool, &name, &value).await {
                eprintln!("Error inserting record: {}", e);
            }
        })
        .await;

    let duration = start.elapsed();
    println!("Inserted {} records in {:?}", num_records, duration);

    Ok(())
}

async fn count_records(pool: &Pool) -> Result<i64, Error> {
    let row = musq::query("SELECT COUNT(*) FROM records")
        .fetch_one(pool)
        .await?;
    let count: i64 = row.get_value_idx(0)?;
    Ok(count)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();

    println!(
        "Preparing to insert {} records with blob size of {} bytes",
        args.records, args.blob_size
    );

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let database_path = args.database.as_ref().map_or_else(
        || temp_dir.path().join("temp_database.db"),
        |path| path.to_owned(),
    );

    let pool = setup_database(&database_path).await?;
    create_schema(&pool).await?;
    insert_records(&pool, args.records, args.concurrency, args.blob_size).await?;

    // Sanity check
    let record_count = count_records(&pool).await?;
    if record_count as u64 == args.records {
        println!(
            "Sanity check passed: {} records found in the database",
            record_count
        );
    } else {
        eprintln!(
            "Sanity check failed: Expected {} records, but found {} in the database",
            args.records, record_count
        );
    }

    if args.database.is_none() {
        println!("Using temporary database");
    }

    Ok(())
}
