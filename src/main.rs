#![allow(non_snake_case)]

mod args;
mod chunker;
mod download;
mod proxy;

use anyhow::{anyhow, Context, Result};
use args::Args;
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Duration;
use tokio::task::JoinSet;
use tracing::error;
use tracing_subscriber;

fn output_path(args: &Args) -> PathBuf {
    if let Some(o) = &args.output {
        return PathBuf::from(o);
    }

    let name = args
        .url
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or("download.bin");

    PathBuf::from(name)
}

fn style() -> ProgressStyle {
    ProgressStyle::with_template("{prefix:>10} [{bar:30}] {bytes}/{total_bytes} {bytes_per_sec} {eta}")
        .unwrap()
        .progress_chars("=>-")
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    let env_filter = if args.verbose {
        "debug"
    } else {
        "info"
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .init();

    let out_path = output_path(&args);

    println!("[+] onionRush :: probing target through tor");

    let probe_client = proxy::build_client(
        &args.socks,
        &proxy::random_identity(),
        Duration::from_secs(args.timeout),
    )?;

    let target = chunker::probe(&probe_client, &args.url)
        .await
        .context("probe failed, is tor running on the socks port?")?;

    println!("[+] size: {} bytes ({:.2} GB), range support: {}", 
        target.size, 
        target.size as f64 / (1024.0 * 1024.0 * 1024.0),
        target.accepts_ranges
    );

    let circuits = if target.accepts_ranges {
        if let Some(chunk_size_mb) = args.chunk_size_mb {
            println!("[*] using chunk size: {} MB", chunk_size_mb);
            (target.size + chunk_size_mb * 1024 * 1024 - 1) / (chunk_size_mb * 1024 * 1024)
        } else {
            args.circuits as u64
        }
    } else {
        println!("[!] server does not support ranged requests, falling back to a single stream");
        1
    };

    let file = std::fs::File::create(&out_path)
        .with_context(|| format!("could not create output file {:?}", out_path))?;
    file.set_len(target.size)?;
    drop(file);

    let chunks = if let Some(chunk_size_mb) = args.chunk_size_mb {
        chunker::plan_chunks_by_size(target.size, chunk_size_mb)
    } else {
        chunker::plan_chunks(target.size, circuits as usize)
    };

    println!("[*] planning {} chunks for download", chunks.len());

    let multi = MultiProgress::new();
    let mut set = JoinSet::new();

    for chunk in chunks {
        let pb = multi.add(ProgressBar::new(chunk.end - chunk.start + 1));
        pb.set_style(style());
        pb.set_prefix(format!("chunk {}", chunk.index));

        let args = args.clone();
        let url = target.url.clone();
        let out_path = out_path.clone();

        set.spawn(async move {
            let result = download::download_chunk(&args, &url, &chunk, &out_path, &pb).await;
            match &result {
                Ok(()) => {
                    pb.finish_with_message("[+] done");
                },
                Err(e) => {
                    pb.finish_with_message(format!("[-] failed: {}", e));
                    error!("[-] Chunk {} failed: {}", chunk.index, e);
                }
            }
            result
        });
    }

    let mut failures = 0;
    let mut completed = 0;
    let total_chunks = set.len();
    
    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(())) => {
                completed += 1;
            }
            Ok(Err(_)) => {
                failures += 1;
            }
            Err(e) => {
                error!("[-] Task panicked: {}", e);
                failures += 1;
            }
        }
        
        if total_chunks > 0 {
            let progress = (completed + failures) as f64 / total_chunks as f64 * 100.0;
            eprint!("\r[*] Progress: {:.1}% ({} completed, {} failed)", 
                progress, completed, failures);
        }
    }
    eprintln!();

    println!("[*] Verifying downloaded file...");
    let mut verification_failures = 0;
    
    let verify_chunks = if let Some(chunk_size_mb) = args.chunk_size_mb {
        chunker::plan_chunks_by_size(target.size, chunk_size_mb)
    } else {
        chunker::plan_chunks(target.size, circuits as usize)
    };
    
    for chunk in verify_chunks {
        if !download::verify_chunk(&out_path, &chunk).await? {
            error!("[-] Chunk {} verification failed", chunk.index);
            verification_failures += 1;
        }
    }
    
    if verification_failures > 0 {
        error!("[-] {} chunks failed verification", verification_failures);
        return Err(anyhow!("{} chunk(s) failed verification, output is incomplete", verification_failures));
    }

    if failures > 0 {
        error!("[-] {} chunks failed download", failures);
        return Err(anyhow!("{} chunk(s) failed, output is incomplete", failures));
    }

    let metadata = std::fs::metadata(&out_path)?;
    if metadata.len() != target.size {
        error!("[-] File size mismatch: expected {}, got {}", target.size, metadata.len());
        return Err(anyhow!("File size mismatch"));
    }

    println!("[+] Download complete! File saved to: {:?}", out_path);
    println!("[+] Total size: {} bytes ({:.2} GB)", 
        target.size,
        target.size as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    
    Ok(())
}