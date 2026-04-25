//! Benchmark comparing per-file extraction vs batch extraction.
//!
//! To run this benchmark, you need a test RAR archive.
//! 
//! # Quick test with existing data:
//! ```shell
//! cargo bench --bench extract_benchmark
//! ```
//!
//! # For a more realistic test with many files (like Linux kernel source):
//! 1. Download and prepare test data:
//!    ```shell
//!    curl -LZO https://github.com/torvalds/linux/archive/refs/heads/master.zip
//!    unzip master.zip -d kernel-master
//!    rar a -r kernel.rar kernel-master
//!    ```
//! 2. Set the environment variable and run:
//!    ```shell
//!    UNRAR_BENCH_ARCHIVE=/path/to/kernel.rar cargo bench --bench extract_benchmark
//!    ```

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use std::path::Path;
use tempfile::TempDir;
use unrar_ng::Archive;

/// Extract archive using per-file iteration (the traditional approach)
fn extract_per_file(archive_path: &Path, dest: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    let mut archive = Archive::new(archive_path).open_for_processing()?;
    let mut count = 0;
    
    while let Some(header) = archive.read_header()? {
        archive = if header.entry().is_file() {
            count += 1;
            header.extract_with_base(dest)?
        } else {
            // Create directory or skip
            header.extract_with_base(dest)?
        };
    }
    
    Ok(count)
}

/// Extract archive using batch extraction (the new optimized approach)
fn extract_batch(archive_path: &Path, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let archive = Archive::new(archive_path).open_for_processing()?;
    archive.extract_all(dest)?;
    Ok(())
}

fn benchmark_extraction(c: &mut Criterion) {
    // Try to get archive path from environment variable, or use default test data
    let archive_path = std::env::var("UNRAR_BENCH_ARCHIVE")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("data/version.rar"));
    
    if !archive_path.exists() {
        eprintln!("Warning: Archive file not found: {}", archive_path.display());
        eprintln!("Set UNRAR_BENCH_ARCHIVE environment variable to specify a test archive.");
        eprintln!("Using default test archive if available.");
        
        // Fallback to any available test archive
        let fallback = Path::new("data/version.rar");
        if !fallback.exists() {
            eprintln!("No test archive found. Skipping benchmark.");
            return;
        }
    }
    
    let archive_path_str = archive_path.to_string_lossy();
    
    let mut group = c.benchmark_group("RAR Extraction");
    
    // Configure for potentially long-running benchmarks
    group.sample_size(10);
    
    group.bench_with_input(
        BenchmarkId::new("per_file", &archive_path_str),
        &archive_path,
        |b, path| {
            b.iter_with_setup(
                || TempDir::new().expect("Failed to create temp dir"),
                |temp_dir| {
                    let _ = extract_per_file(path, temp_dir.path());
                },
            );
        },
    );
    
    group.bench_with_input(
        BenchmarkId::new("batch", &archive_path_str),
        &archive_path,
        |b, path| {
            b.iter_with_setup(
                || TempDir::new().expect("Failed to create temp dir"),
                |temp_dir| {
                    let _ = extract_batch(path, temp_dir.path());
                },
            );
        },
    );
    
    group.finish();
}

criterion_group!(benches, benchmark_extraction);
criterion_main!(benches);
