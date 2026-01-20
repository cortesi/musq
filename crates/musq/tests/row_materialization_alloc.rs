//! Allocation-focused regression tests for row materialization.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint,
    sync::atomic::{AtomicUsize, Ordering},
};

use futures_util::TryStreamExt;
use tokio::runtime::Builder;

/// A global allocator that counts allocations for this test binary.
struct CountingAllocator;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// Snapshot of allocator counters.
#[derive(Clone, Copy, Debug)]
struct AllocationCounters {
    /// Number of allocations performed.
    allocations: usize,
    /// Total bytes allocated.
    bytes: usize,
}

/// Reset the global allocator counters to zero.
fn reset_allocation_counters() {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
}

/// Capture current allocation counter values.
fn allocation_counters() -> AllocationCounters {
    AllocationCounters {
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
    }
}

/// Drain a query's results without decoding, ensuring each row is materialized.
async fn drain_rows(conn: &musq::PoolConnection, sql: &str) {
    let mut stream = musq::query(sql).fetch(conn);
    while let Some(row) = stream.try_next().await.unwrap() {
        hint::black_box(row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_materialization_allocation_overhead_is_bounded() {
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let pool = musq::Musq::new()
                .max_connections(1)
                .open_in_memory()
                .await
                .unwrap();
            let conn = pool.acquire().await.unwrap();

            musq::query(
                "CREATE TABLE data (\
                 id INTEGER, \
                 t1 TEXT, t2 TEXT, t3 TEXT, t4 TEXT, t5 TEXT, t6 TEXT, t7 TEXT, t8 TEXT, \
                 b1 BLOB, b2 BLOB, b3 BLOB, b4 BLOB, b5 BLOB, b6 BLOB, b7 BLOB, b8 BLOB\
                 )",
            )
            .execute(&conn)
            .await
            .unwrap();

            let text = "x".repeat(1024);
            let blob = vec![0_u8; 1024];
            let rows = 200_u32;

            for i in 0..rows {
                musq::query(
                    "INSERT INTO data \
                     (id, t1, t2, t3, t4, t5, t6, t7, t8, b1, b2, b3, b4, b5, b6, b7, b8) \
                     VALUES \
                     (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                )
                .bind(i)
                .bind(&text)
                .bind(&text)
                .bind(&text)
                .bind(&text)
                .bind(&text)
                .bind(&text)
                .bind(&text)
                .bind(&text)
                .bind(blob.as_slice())
                .bind(blob.as_slice())
                .bind(blob.as_slice())
                .bind(blob.as_slice())
                .bind(blob.as_slice())
                .bind(blob.as_slice())
                .bind(blob.as_slice())
                .bind(blob.as_slice())
                .execute(&conn)
                .await
                .unwrap();
            }

            // Warm up statement cache for both queries so we're primarily measuring row materialization.
            drain_rows(&conn, "SELECT t1, b1 FROM data").await;
            drain_rows(
                &conn,
                "SELECT t1, t2, t3, t4, t5, t6, t7, t8, b1, b2, b3, b4, b5, b6, b7, b8 FROM data",
            )
            .await;

            reset_allocation_counters();
            drain_rows(&conn, "SELECT t1, b1 FROM data").await;
            let small = allocation_counters();

            reset_allocation_counters();
            drain_rows(
                &conn,
                "SELECT t1, t2, t3, t4, t5, t6, t7, t8, b1, b2, b3, b4, b5, b6, b7, b8 FROM data",
            )
            .await;
            let large = allocation_counters();

            assert!(
                large.allocations <= small.allocations.saturating_mul(2),
                "expected allocations to stay roughly constant: \
                 small_allocations={}, small_bytes={}, large_allocations={}, large_bytes={}",
                small.allocations,
                small.bytes,
                large.allocations,
                large.bytes,
            );
        });
    }
}
