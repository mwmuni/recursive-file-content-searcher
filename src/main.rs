use std::env;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::{mpsc, Mutex, Semaphore};
use tokio::time::Instant;
use futures::future::join_all;
use regex::Regex;
use num_cpus;

use indicatif::{ProgressBar, ProgressStyle};
use walkdir::WalkDir;

#[derive(Debug)]
struct MatchContext {
    text: String,
    start: usize,
    end: usize,
}

#[derive(Debug)]
struct FileMatch {
    path: String,
    match_count: usize,
    matches: Vec<MatchContext>,
}

fn get_context(line: &str, match_start: usize, match_end: usize, context_size: usize) -> MatchContext {
    let start = if match_start > context_size {
        match_start - context_size
    } else {
        0
    };
    let end = std::cmp::min(match_end + context_size, line.len());
    
    // Find word boundaries or use exact positions
    let context_start = line[start..match_start]
        .rfind(char::is_whitespace)
        .map(|i| start + i + 1)
        .unwrap_or(start);
        
    let context_end = line[match_end..end]
        .find(char::is_whitespace)
        .map(|i| match_end + i)
        .unwrap_or(end);

    MatchContext {
        text: line[context_start..context_end].to_string(),
        start: match_start - context_start,
        end: match_end - context_start,
    }
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
        .follow_links(false)
        .into_iter()
    {
        let entry = match entry_res {
            Ok(e) => e,
            Err(e) => {
                eprintln!("WalkDir error: {e}");
                continue;
            }
        };

        if !entry.file_type().is_file() {
            continue;
        }

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

        progress_bar.inc_length(1);

        let path = entry.into_path();
        if let Err(_send_err) = file_tx.blocking_send(path) {
            break;
        }
    }

    Ok(())
}

#[tokio::main(flavor = "multi_thread", worker_threads = 32)]
async fn main() -> io::Result<()> {
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
    let total_matches = Arc::new(AtomicUsize::new(0));

    let (file_tx, file_rx) = mpsc::channel::<PathBuf>(10_000);
    let file_rx = Arc::new(Mutex::new(file_rx));
    let (result_tx, mut result_rx) = mpsc::channel::<FileMatch>(10_000);

    let progress_bar = Arc::new(ProgressBar::new(0));
    progress_bar.set_style(
        ProgressStyle::with_template("{bar:40.cyan/blue} {pos}/{len} files processed  {msg}")
            .unwrap()
            .progress_chars("##-"),
    );

    let pb_for_enum = Arc::clone(&progress_bar);
    let enum_handle = tokio::task::spawn_blocking(move || {
        let _ = blocking_enumerate_dirs(start_path, file_tx, &pb_for_enum, size_limit_bytes);
    });

    let num_workers = num_cpus::get();
    let semaphore = Arc::new(Semaphore::new(num_workers * 2));

    let mut worker_handles = Vec::new();
    for _worker_id in 0..num_workers {
        let regex = regex.clone();
        let result_tx = result_tx.clone();
        let sem = Arc::clone(&semaphore);
        let file_rx = Arc::clone(&file_rx);
        let pb_for_worker = Arc::clone(&progress_bar);
        let total_matches = Arc::clone(&total_matches);

        let handle = tokio::spawn(async move {
            while let Some(file_path) = {
                let mut rx = file_rx.lock().await;
                rx.recv().await
            } {
                let _permit = sem.acquire().await.unwrap();
                pb_for_worker.inc(1);

                match tokio::fs::read_to_string(&file_path).await {
                    Ok(contents) => {
                        let mut match_count = 0;
                        let mut matches = Vec::new();

                        for line in contents.lines() {
                            if let Some(m) = regex.find(line) {
                                match_count += 1;
                                matches.push(get_context(line, m.start(), m.end(), 64));
                            }
                        }

                        if match_count > 0 {
                            total_matches.fetch_add(match_count, Ordering::Relaxed);
                            let _ = result_tx.send(FileMatch {
                                path: file_path.to_string_lossy().to_string(),
                                match_count,
                                matches,
                            }).await;
                        }
                    }
                    Err(_) => {}
                }
            }
        });
        worker_handles.push(handle);
    }

    drop(result_tx);

    let result_reader = tokio::spawn(async move {
        let mut detected_files = Vec::new();
        while let Some(file_match) = result_rx.recv().await {
            println!("Found {} matches in file: {}", file_match.match_count, file_match.path);
            for context in file_match.matches {
                println!("  Context: {}", context.text);
                println!("  Match position: {}..{}", context.start, context.end);
            }
            detected_files.push(file_match.path.clone());
        }
        detected_files
    });

    if let Err(e) = enum_handle.await {
        eprintln!("Enumerator task error: {e}");
    }

    if let Err(e) = join_all(worker_handles).await.into_iter().collect::<Result<Vec<_>, _>>() {
        eprintln!("Error joining worker: {e}");
    }

    let detected_files = result_reader.await.unwrap_or_default();

    progress_bar.finish_with_message("All files processed!");

    let elapsed = start_time.elapsed();
    println!("\nSearch completed in {elapsed:.2?}");
    println!("Total matches found: {}", total_matches.load(Ordering::Relaxed));
    println!("Files with matches: {}", detected_files.len());

    Ok(())
}
