//! Batch extract all files from a RAR archive using the optimized extract_all() method.
//!
//! Usage: cargo run --example batch_extract -- <archive.rar> <dest_dir>
//!
//! The destination directory must not exist (it will be created).
//!
//! This example demonstrates the performance-optimized batch extraction,
//! which is significantly faster than per-file extraction for archives
//! containing many small files.

use std::env;
use std::path::Path;
use std::process;
use unrar::Archive;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        eprintln!("Usage: {} <archive.rar> <dest_dir>", args[0]);
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  <archive.rar>  Path to the RAR archive file");
        eprintln!("  <dest_dir>     Destination directory (must not exist)");
        process::exit(1);
    }

    let archive_path = &args[1];
    let dest_dir = &args[2];

    // Check if archive file exists
    if !Path::new(archive_path).exists() {
        eprintln!("Error: Archive file '{}' does not exist", archive_path);
        process::exit(1);
    }

    // Check if destination directory already exists
    if Path::new(dest_dir).exists() {
        eprintln!("Error: Destination directory '{}' already exists", dest_dir);
        eprintln!("Please specify a non-existing directory to avoid accidental overwrites.");
        process::exit(1);
    }

    // Create destination directory
    if let Err(e) = std::fs::create_dir_all(dest_dir) {
        eprintln!("Error: Failed to create destination directory: {}", e);
        process::exit(1);
    }

    println!("Extracting '{}' to '{}'...", archive_path, dest_dir);

    // Open the archive for processing
    let archive = match Archive::new(archive_path).open_for_processing() {
        Ok(archive) => archive,
        Err(e) => {
            eprintln!("Error: Failed to open archive: {}", e);
            // Clean up the created directory
            let _ = std::fs::remove_dir(dest_dir);
            process::exit(1);
        }
    };

    // Use batch extraction for optimal performance
    let start = std::time::Instant::now();
    
    if let Err(e) = archive.extract_all(dest_dir) {
        eprintln!("Error: Failed to extract archive: {}", e);
        process::exit(1);
    }

    let elapsed = start.elapsed();
    println!("Extraction completed in {:.2?}", elapsed);

    // Count extracted files
    let file_count = count_files(Path::new(dest_dir));
    println!("Extracted {} files/directories", file_count);
}

/// Recursively count files and directories in a path
fn count_files(path: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            count += 1;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                count += count_files(&entry_path);
            }
        }
    }
    count
}
