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

use musq::{JournalMode, Pool, Result, Row, Synchronous};

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
    start_time: Instant,
    last_report_time: Instant,
    report_interval: Duration,
    total_records: u64,
}

impl TimingData {
    fn new(total_records: u64, report_interval: Duration) -> Self {
        let now = Instant::now();
        TimingData {
            durations: Vec::with_capacity(total_records as usize),
            start_time: now,
            last_report_time: now,
            report_interval,
            total_records,
        }
    }

    fn add_duration(&mut self, duration: Duration) {
        self.durations.push(duration);
        self.maybe_report_progress();
    }

    fn maybe_report_progress(&mut self) {
        let now = Instant::now();
        if now - self.last_report_time >= self.report_interval {
            self.report_progress();
            self.last_report_time = now;
        }
    }

    fn report_progress(&self) {
        let records_processed = self.durations.len() as u64;
        let elapsed = self.start_time.elapsed();
        let operations_per_second = records_processed as f64 / elapsed.as_secs_f64();
        let progress_percentage = (records_processed as f64 / self.total_records as f64) * 100.0;

        println!(
            "Progress: {:.2}% ({}/{}) | Elapsed: {:.0?} | Ops/sec: {:.2}",
            progress_percentage,
            records_processed,
            self.total_records,
            elapsed,
            operations_per_second
        );
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
                "{start_record}-{end_record}: min: {min_duration:?}, max: {max_duration:?}, avg: {avg_duration:?}"
            );
        }
    }
}

async fn setup_database(args: &Args, path: &PathBuf) -> Result<Pool> {
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

async fn create_schema(pool: &Pool) -> Result<()> {
    // Create table A
    musq::query(
        "CREATE TABLE IF NOT EXISTS a (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            data BLOB NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // Create table B with a foreign key to A
    musq::query(
        "CREATE TABLE IF NOT EXISTS b (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            a_id INTEGER NOT NULL,
            data BLOB NOT NULL,
            FOREIGN KEY (a_id) REFERENCES a(id)
        )",
    )
    .execute(pool)
    .await?;

    // Create index on B's foreign key
    musq::query("CREATE INDEX IF NOT EXISTS idx_b_a_id ON b (a_id)")
        .execute(pool)
        .await?;

    Ok(())
}

async fn insert_record(pool: &Pool, a_data: &[u8], b_data: &[u8]) -> Result<()> {
    // Start a transaction
    let mut tx = pool.begin().await?;

    // Insert into A
    musq::query("INSERT INTO a (data) VALUES (?)")
        .bind(a_data)
        .execute(&mut tx)
        .await?;

    // Get the last inserted id from A
    let a_id: i64 = musq::query("SELECT last_insert_rowid()")
        .fetch_one(&mut tx)
        .await?
        .get_value_idx(0)?;

    // Insert into B
    musq::query("INSERT INTO b (a_id, data) VALUES (?, ?)")
        .bind(a_id)
        .bind(b_data)
        .execute(&mut tx)
        .await?;

    // Commit the transaction
    tx.commit().await?;

    Ok(())
}

async fn read_random_record(pool: &Pool, max_id: u64) -> Result<(Row, Row)> {
    let random_id = rand::rng().random_range(1..=max_id) as i64;

    let b_row = musq::query("SELECT * FROM b WHERE id = ?")
        .bind(random_id)
        .fetch_one(pool)
        .await?;

    let a_id: i64 = b_row.get_value("a_id")?;

    let a_row = musq::query("SELECT * FROM a WHERE id = ?")
        .bind(a_id)
        .fetch_one(pool)
        .await?;

    Ok((a_row, b_row))
}

async fn count_records(pool: &Pool) -> Result<(i64, i64)> {
    let a_count: i64 = musq::query("SELECT COUNT(*) FROM a")
        .fetch_one(pool)
        .await?
        .get_value_idx(0)?;

    let b_count: i64 = musq::query("SELECT COUNT(*) FROM b")
        .fetch_one(pool)
        .await?
        .get_value_idx(0)?;

    Ok((a_count, b_count))
}

fn generate_random_data(size: usize) -> (Vec<u8>, Vec<u8>) {
    let mut rng = rand::rng();
    let a_data = (0..size).map(|_| rng.random::<u8>()).collect();
    let b_data = (0..size).map(|_| rng.random::<u8>()).collect();
    (a_data, b_data)
}

async fn perform_operations(
    pool: &Pool,
    num_records: u64,
    concurrency: usize,
    blob_size: usize,
) -> Result<TimingData> {
    let start = Instant::now();
    let timing_data = Arc::new(Mutex::new(TimingData::new(
        num_records,
        Duration::from_secs(5),
    ))); // Report every 5 seconds
    let max_id = Arc::new(Mutex::new(0u64));

    stream::iter(0..num_records)
        .for_each_concurrent(concurrency, |_| {
            let timing_data = Arc::clone(&timing_data);
            let max_id = Arc::clone(&max_id);
            let pool = pool.clone();
            async move {
                let operation_start = Instant::now();

                let (a_data, b_data) = generate_random_data(blob_size);

                if (insert_record(&pool, &a_data, &b_data).await).is_ok() {
                    let mut id = max_id.lock().await;
                    *id += 1;
                    drop(id);

                    if let Err(e) = read_random_record(&pool, *max_id.lock().await).await {
                        eprintln!("Error reading record: {e}");
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
    println!("Performed {num_records} write/read operations in {duration:?}");

    Ok(Arc::try_unwrap(timing_data).unwrap().into_inner())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!(
        "Preparing to perform {} write/read operations with blob size of {} bytes",
        args.records, args.blob_size
    );
    println!(
        "Using journal mode: {}, synchronous mode: {}, max connections: {}",
        args.journal_mode, args.synchronous, args.max_connections
    );

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let database_path = args.database.as_ref().map_or_else(
        || temp_dir.path().join("temp_database.db"),
        |path| path.to_owned(),
    );

    let pool = setup_database(&args, &database_path).await?;
    create_schema(&pool).await?;

    println!("Starting operations...");
    let timing_data =
        perform_operations(&pool, args.records, args.concurrency, args.blob_size).await?;

    // Sanity check
    let (a_count, b_count) = count_records(&pool).await?;
    if a_count as u64 == args.records && b_count as u64 == args.records {
        println!(
            "Sanity check passed: {a_count} records in table A and {b_count} records in table B found in the database"
        );
    } else {
        eprintln!(
            "Sanity check failed: Expected {} records, but found {} records in table A and {} records in table B",
            args.records, a_count, b_count
        );
    }

    if args.database.is_none() {
        println!("Using temporary database");
    }

    println!("\nFinal timing data:");
    timing_data.process(args.bins);

    Ok(())
}
