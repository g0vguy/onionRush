use crate::args::Args;
use crate::chunker::Chunk;
use crate::proxy;
use anyhow::{anyhow, Result};
use futures::StreamExt;
use indicatif::ProgressBar;
use std::path::Path;
use std::time::Duration;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tracing::{debug, error, info, warn};

pub async fn download_chunk(
    args: &Args,
    url: &str,
    chunk: &Chunk,
    out_path: &Path,
    pb: &ProgressBar,
) -> Result<()> {
    let mut last_err = None;
    let mut current_start = chunk.start;
    let mut bytes_written = 0u64;

    for attempt in 1..=args.retries {
        if attempt > 1 && bytes_written > 0 {
            current_start = chunk.start + bytes_written;
            info!(
                "[*] Resuming chunk {} at byte {} ({}%)",
                chunk.index,
                current_start,
                (bytes_written as f64 / (chunk.end - chunk.start + 1) as f64) * 100.0
            );
            pb.set_message(format!("resuming at {:.1}%", 
                (bytes_written as f64 / (chunk.end - chunk.start + 1) as f64) * 100.0));
        }

        let identity = proxy::random_identity();
        let client = match proxy::build_client(&args.socks, &identity, Duration::from_secs(args.timeout)) {
            Ok(c) => c,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };

        let range = if current_start <= chunk.end {
            format!("bytes={}-{}", current_start, chunk.end)
        } else {
            pb.finish_with_message("[+] done");
            return Ok(());
        };

        match attempt_chunk(&client, args, url, &range, chunk, current_start, out_path, pb, &mut bytes_written).await {
            Ok(()) => {
                pb.finish_with_message("[+] done");
                return Ok(());
            },
            Err(e) => {
                let err_msg = format!("chunk {} retry {}/{}: {}", chunk.index, attempt, args.retries, e);
                pb.set_message(err_msg.clone());
                warn!("[!] {}", err_msg);
                last_err = Some(e);
                
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }

    error!("[-] Chunk {} failed after {} retries", chunk.index, args.retries);
    Err(last_err.unwrap_or_else(|| anyhow!("chunk {} failed with no error recorded", chunk.index)))
}

async fn attempt_chunk(
    client: &reqwest::Client,
    args: &Args,
    url: &str,
    range: &str,
    chunk: &Chunk,
    start_byte: u64,
    out_path: &Path,
    pb: &ProgressBar,
    total_written: &mut u64,
) -> Result<()> {
    debug!("[*] Requesting range: {}", range);

    let resp = client
        .get(url)
        .header(reqwest::header::RANGE, range)
        .header(reqwest::header::CONNECTION, "keep-alive")
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .send()
        .await
        .map_err(|e| anyhow!("request failed: {}", e))?;

    let status = resp.status();
    if status != reqwest::StatusCode::PARTIAL_CONTENT && status != reqwest::StatusCode::OK {
        return Err(anyhow!("expected 206 or 200, got {}", status));
    }

    let mut file = OpenOptions::new().write(true).open(out_path).await?;
    file.seek(SeekFrom::Start(start_byte)).await?;

    let expected = chunk.end - start_byte + 1;
    let mut written = 0u64;
    let mut stream = resp.bytes_stream();
    let mut last_progress_update = std::time::Instant::now();
    let mut consecutive_timeouts = 0;

    while let Some(result) = stream.next().await {
        // Check for data stall - only fail if no data for 60 seconds
        if last_progress_update.elapsed() > Duration::from_secs(60) {
            return Err(anyhow!("data stall: no data received for 60 seconds"));
        }

        // Process the data with a 10 second timeout per chunk
        let piece = match tokio::time::timeout(Duration::from_secs(10), async {
            result.map_err(|e| anyhow!("stream error: {}", e))
        }).await {
            Ok(Ok(p)) => {
                consecutive_timeouts = 0;
                p
            },
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                consecutive_timeouts += 1;
                if consecutive_timeouts > 5 {
                    return Err(anyhow!("too many consecutive timeouts (5)"));
                }
                continue;
            }
        };

        // Write the data
        file.write_all(&piece).await?;
        written += piece.len() as u64;
        *total_written += piece.len() as u64;
        pb.inc(piece.len() as u64);
        
        // Update progress tracking
        last_progress_update = std::time::Instant::now();
        
        // Check if we've downloaded more than expected
        if written > expected {
            return Err(anyhow!("received more data than expected: {} > {}", written, expected));
        }
        
        // Log progress every 10%
        let percent = (written as f64 / expected as f64) * 100.0;
        if percent % 10.0 < 1.0 && written > 0 {
            debug!("[*] Chunk {}: {:.1}% complete", chunk.index, percent);
        }
    }

    // Force sync to disk
    file.sync_all().await?;

    // Final verification
    if written != expected {
        return Err(anyhow!(
            "incomplete download: got {} of {} bytes ({}% complete)",
            written,
            expected,
            (written as f64 / expected as f64) * 100.0
        ));
    }

    debug!("[+] Chunk {} completed successfully", chunk.index);
    Ok(())
}

pub async fn verify_chunk(out_path: &Path, chunk: &Chunk) -> Result<bool> {
    let file = tokio::fs::File::open(out_path).await?;
    let metadata = file.metadata().await?;
    
    if metadata.len() <= chunk.end {
        return Ok(false);
    }
    
    Ok(true)
}