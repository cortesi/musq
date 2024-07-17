use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use clap::Parser;
use futures::stream::{self, StreamExt};
use rand::Rng;
use tempfile::TempDir;
use tokio::sync::Mutex;

use musq::{Error, JournalMode, Pool, Row, Synchronous};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the SQLite database file
    #[arg(short, long)]
    database: Option<PathBuf>,

    /// Number of records to insert
    #[arg(short, long, default_value_t = 100000)]
    records: u64,

    /// Degree of concurrency for operations
    #[arg(short, long, default_value_t = 1)]
    concurrency: usize,

    /// Size of the blob value in bytes
    #[arg(short, long, default_value_t = 250)]
    blob_size: usize,

    /// Number of bins for timing data analysis
    #[arg(long, default_value_t = 20)]
    bins: usize,

    /// Journal mode for the database
    #[arg(long, default_value = "wal")]
    journal_mode: String,

    /// Synchronous mode for the database
    #[arg(long, default_value = "normal")]
    synchronous: String,

    /// Maximum number of connections in the pool
    #[arg(long, default_value_t = 10)]
    max_connections: u32,
}

#[derive(Debug)]
struct TimingData {
    durations: Vec<Duration>,
}

impl TimingData {
    fn new(capacity: usize) -> Self {
        TimingData {
            durations: Vec::with_capacity(capacity),
        }
    }

    fn add_duration(&mut self, duration: Duration) {
        self.durations.push(duration);
    }

    fn process(&self, bins: usize) {
        let mut durations = self.durations.clone();
        durations.sort_unstable();

        let total_records = durations.len();
        let records_per_bin = total_records / bins;

        println!("\nTiming data (per bin):");
        for (i, chunk) in durations.chunks(records_per_bin).enumerate() {
            let start_record = i * records_per_bin;
            let end_record = start_record + chunk.len() - 1;

            let min_duration = chunk.first().unwrap();
            let max_duration = chunk.last().unwrap();
            let avg_duration: Duration = chunk.iter().sum::<Duration>() / chunk.len() as u32;

            println!(
                "{}-{}: min: {:?}, max: {:?}, avg: {:?}",
                start_record, end_record, min_duration, max_duration, avg_duration
            );
        }
    }
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

async fn perform_operations(
    pool: &Pool,
    num_records: u64,
    concurrency: usize,
    blob_size: usize,
) -> Result<TimingData, Error> {
    let start = Instant::now();
    let timing_data = Arc::new(Mutex::new(TimingData::new(num_records as usize)));
    let max_id = Arc::new(Mutex::new(0u64));

    stream::iter(0..num_records)
        .for_each_concurrent(concurrency, |i| {
            let timing_data = Arc::clone(&timing_data);
            let max_id = Arc::clone(&max_id);
            let pool = pool.clone();
            async move {
                let operation_start = Instant::now();

                let name = format!("Record {}", i);
                let value = generate_random_blob(blob_size);

                if (insert_record(&pool, &name, &value).await).is_ok() {
                    let mut id = max_id.lock().await;
                    *id += 1;
                    drop(id);

                    if let Err(e) = read_random_record(&pool, *max_id.lock().await).await {
                        eprintln!("Error reading record: {}", e);
                    }
                } else {
                    eprintln!("Error inserting record");
                }

                let operation_duration = operation_start.elapsed();
                timing_data.lock().await.add_duration(operation_duration);
            }
        })
        .await;

    let duration = start.elapsed();
    println!(
        "Performed {} write/read operations in {:?}",
        num_records, duration
    );

    Ok(Arc::try_unwrap(timing_data).unwrap().into_inner())
}

async fn insert_record(pool: &Pool, name: &str, value: &[u8]) -> Result<(), Error> {
    musq::query("INSERT INTO records (name, value) VALUES (?, ?)")
        .bind(name)
        .bind(value)
        .execute(pool)
        .await?;
    Ok(())
}

async fn read_random_record(pool: &Pool, max_id: u64) -> Result<Row, Error> {
    let random_id = rand::thread_rng().gen_range(1..=max_id) as u32;
    let row = musq::query("SELECT * FROM records WHERE id = ?")
        .bind(random_id)
        .fetch_one(pool)
        .await?;
    Ok(row)
}

async fn count_records(pool: &Pool) -> Result<i64, Error> {
    let row = musq::query("SELECT COUNT(*) FROM records")
        .fetch_one(pool)
        .await?;
    let count: i64 = row.get_value_idx(0)?;
    Ok(count)
}

async fn setup_database(args: &Args, path: &PathBuf) -> Result<Pool, Error> {
    let journal_mode = match args.journal_mode.to_lowercase().as_str() {
        "wal" => JournalMode::Wal,
        "delete" => JournalMode::Delete,
        "truncate" => JournalMode::Truncate,
        "persist" => JournalMode::Persist,
        "memory" => JournalMode::Memory,
        "off" => JournalMode::Off,
        _ => {
            eprintln!("Invalid journal mode. Defaulting to WAL.");
            JournalMode::Wal
        }
    };

    let synchronous = match args.synchronous.to_lowercase().as_str() {
        "normal" => Synchronous::Normal,
        "full" => Synchronous::Full,
        "extra" => Synchronous::Extra,
        "off" => Synchronous::Off,
        _ => {
            eprintln!("Invalid synchronous mode. Defaulting to Normal.");
            Synchronous::Normal
        }
    };

    let musq = musq::Musq::new()
        .max_connections(args.max_connections)
        .create_if_missing(true)
        .journal_mode(journal_mode)
        .synchronous(synchronous);

    musq.open(path).await
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();

    println!(
        "Preparing to perform {} write/read operations with blob size of {} bytes",
        args.records, args.blob_size
    );

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let database_path = args.database.as_ref().map_or_else(
        || temp_dir.path().join("temp_database.db"),
        |path| path.to_owned(),
    );

    let pool = setup_database(&args, &database_path).await?;
    create_schema(&pool).await?;

    let timing_data =
        perform_operations(&pool, args.records, args.concurrency, args.blob_size).await?;

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

    timing_data.process(args.bins);

    Ok(())
}
