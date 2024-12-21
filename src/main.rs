use std::env;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex, Semaphore};
use tokio::time::Instant;
use futures::future::join_all;
use regex::Regex;
use num_cpus;

use indicatif::{ProgressBar, ProgressStyle};
use walkdir::WalkDir;

#[derive(Debug)]
struct FileMatch {
    path: String,
    matches: Vec<String>,
}

fn print_usage() {
    eprintln!("Usage: <regex_pattern> [starting_path] [size_limit_mb]");
    eprintln!("  regex_pattern: Pattern to search for");
    eprintln!("  starting_path: Directory to start search from (default: current directory)");
    eprintln!("  size_limit_mb: Maximum file size in MB to search (default: 1000)");
}

/// This function does a blocking traversal (using `walkdir`) of all files.
fn blocking_enumerate_dirs(
    start_dir: PathBuf,
    file_tx: mpsc::Sender<PathBuf>,
    progress_bar: &ProgressBar,
    size_limit_bytes: u64,
) -> io::Result<()> {
    for entry_res in WalkDir::new(&start_dir)
        .follow_links(false) // optionally skip symlinks
        .into_iter()
    {
        let entry = match entry_res {
            Ok(e) => e,
            Err(e) => {
                eprintln!("WalkDir error: {e}");
                continue;
            }
        };

        // If it's not a file, skip it
        if !entry.file_type().is_file() {
            continue;
        }

        // Check file size
        // (metadata() is synchronous, but it's usually fine in a blocking enumerator)
        let md = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Failed to get metadata for {:?}: {e}", entry.path());
                continue;
            }
        };
        let size = md.len();
        if size > size_limit_bytes {
            continue;
        }

        // We discovered one more valid file
        progress_bar.inc_length(1);

        // Send the path to the async worker channel
        let path = entry.into_path();
        if let Err(_send_err) = file_tx.blocking_send(path) {
            // The receiver was closed, so no reason to keep enumerating
            break;
        }
    }

    Ok(())
}

#[tokio::main(flavor = "multi_thread", worker_threads = 32)]
async fn main() -> io::Result<()> {
    // Parse args
    let args: Vec<String> = env::args().collect();
    let (pattern, start_path, size_limit_mb) = if cfg!(debug_assertions) {
        (
            args.get(1).unwrap_or(&String::from("lmao")).to_string(),
            PathBuf::from(args.get(2).unwrap_or(&String::from("C:/Users/matt_/Documents"))),
            args.get(3).and_then(|s| s.parse::<u64>().ok()).unwrap_or(1000),
        )
    } else {
        if args.len() < 2 {
            print_usage();
            std::process::exit(1);
        }
        (
            args[1].clone(),
            PathBuf::from(args.get(2).unwrap_or(&String::from("."))),
            args.get(3).and_then(|s| s.parse::<u64>().ok()).unwrap_or(1000),
        )
    };

    if !start_path.exists() || !start_path.is_dir() {
        eprintln!("Error: '{}' is not a valid directory", start_path.display());
        std::process::exit(1);
    }

    let size_limit_bytes = size_limit_mb * 1024 * 1024;

    let regex = match Regex::new(&pattern) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Invalid regex pattern: {e}");
            std::process::exit(1);
        }
    };

    let start_time = Instant::now();

    // We'll have an mpsc::channel for file paths
    let (file_tx, file_rx) = mpsc::channel::<PathBuf>(10_000);
    // We wrap the receiver in a Mutex/Arc so multiple workers can share it
    let file_rx = Arc::new(Mutex::new(file_rx));

    // We'll have another channel for results
    let (result_tx, mut result_rx) = mpsc::channel::<FileMatch>(10_000);

    // Setup progress bar
    let progress_bar = Arc::new(ProgressBar::new(0));
    progress_bar.set_style(
        ProgressStyle::with_template("{bar:40.cyan/blue} {pos}/{len} files processed  {msg}")
            .unwrap()
            .progress_chars("##-"),
    );

    // We do a "blocking" enumeration in a separate thread
    let pb_for_enum = Arc::clone(&progress_bar);
    let enum_handle = tokio::task::spawn_blocking(move || {
        let _ = blocking_enumerate_dirs(start_path, file_tx, &pb_for_enum, size_limit_bytes);
    });

    // We'll spawn multiple async workers that just read files from file_rx
    let num_workers = num_cpus::get();
    println!("Using {num_workers} file-reading workers...");

    // A semaphore to limit concurrency for reading big files
    let semaphore = Arc::new(Semaphore::new(num_workers * 2));

    let mut worker_handles = Vec::new();
    for _worker_id in 0..num_workers {
        let regex = regex.clone();
        let result_tx = result_tx.clone();
        let sem = Arc::clone(&semaphore);
        let file_rx = Arc::clone(&file_rx);
        let pb_for_worker = Arc::clone(&progress_bar);

        // Each worker will keep reading from `file_rx` until it sees None
        let handle = tokio::spawn(async move {
            while let Some(file_path) = {
                let mut rx = file_rx.lock().await;
                rx.recv().await
            } {
                // Acquire semaphore
                let _permit = sem.acquire().await.unwrap();

                // "Complete" 1 file in the progress bar
                pb_for_worker.inc(1);

                // Attempt to read
                match tokio::fs::read_to_string(&file_path).await {
                    Ok(contents) => {
                        let mut matches = Vec::new();
                        for line in contents.lines() {
                            if regex.is_match(line) {
                                matches.push(line.to_string());
                            }
                        }
                        if !matches.is_empty() {
                            let _ = result_tx.send(FileMatch {
                                path: file_path.to_string_lossy().to_string(),
                                matches,
                            }).await;
                        }
                    }
                    Err(_e) => {
                        // Not a UTF-8 file or locked file, etc. We ignore or log
                    }
                }
            }
        });
        worker_handles.push(handle);
    }

    // Drop our copy of result_tx so only workers hold it
    drop(result_tx);

    // Meanwhile, in main, we read from result_rx
    let result_reader = tokio::spawn(async move {
        let mut total_matches = 0;
        let mut detected_files = Vec::new();
        while let Some(file_match) = result_rx.recv().await {
            println!("\nFound matches in file: {}", file_match.path);
            detected_files.push(file_match.path.clone());
            for line in file_match.matches {
                // Determine the maximum between the matched substring length and 128
                let max_length = std::cmp::max(
                    regex.find(&line).map_or(0, |m| m.end() - m.start()),
                    128
                );

                // Truncate the line if it's longer than max_length
                let truncated_line = if line.len() > max_length {
                    // Ensure we don't break in the middle of a multi-byte character
                    line.char_indices()
                        .nth(max_length)
                        .map(|(idx, _)| &line[..idx])
                        .unwrap_or(line.as_str())
                } else {
                    line.as_str()
                };

                println!("  {truncated_line}");
                total_matches += 1;
            }
        }
        (total_matches, detected_files)
    });

    // Wait for the enumerator to finish
    if let Err(e) = enum_handle.await {
        eprintln!("Enumerator task error: {e}");
    }

    // After enumerator finishes, the file_tx is closed, so eventually all workers see `None` from file_rx
    // Wait for all workers
    if let Err(e) = join_all(worker_handles).await.into_iter().collect::<Result<Vec<_>, _>>() {
        eprintln!("Error joining worker: {e}");
    }

    // Now workers are done => results stop eventually
    let (total_matches, detected_files) = result_reader.await.unwrap_or((0, vec![]));

    // Finish the progress bar
    progress_bar.finish_with_message("All files processed!");

    let elapsed = start_time.elapsed();
    println!("\nEnumeration + search completed in {elapsed:.2?}");
    println!("Total matches found: {}", total_matches);

    println!("\nDetected files:");
    for file in detected_files {
        println!("{}", file);
    }

    Ok(())
}
