use crate::args::DownloadArgs;
use crate::chunker::Chunk;
use crate::proxy;
use anyhow::{anyhow, Result};
use futures::StreamExt;
use indicatif::ProgressBar;
use std::path::Path;
use std::time::Duration;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tracing::{error, info, warn};

const STALL_TIMEOUT: Duration = Duration::from_secs(20);

pub async fn download_chunk(
    args: &DownloadArgs,
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
        }

        let identity = proxy::random_identity();
        let client = match proxy::build_client(&args.socks, &identity, Duration::from_secs(args.timeout), false) {
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

        match attempt_chunk(&client, url, &range, chunk, current_start, out_path, pb, &mut bytes_written, args).await {
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
    Err(last_err.unwrap_or_else(|| anyhow!("chunk {} failed", chunk.index)))
}

async fn attempt_chunk(
    client: &reqwest::Client,
    url: &str,
    range: &str,
    chunk: &Chunk,
    start_byte: u64,
    out_path: &Path,
    pb: &ProgressBar,
    total_written: &mut u64,
    args: &DownloadArgs,
) -> Result<()> {
    let mut req = client
        .get(url)
        .header(reqwest::header::RANGE, range)
        .header(reqwest::header::CONNECTION, "keep-alive")
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .header("X-Request-ID", uuid::Uuid::new_v4().to_string());

    // Custom User-Agent
    if let Some(ua) = &args.user_agent {
        req = req.header(reqwest::header::USER_AGENT, ua);
    }

    // Custom headers (-H "Key: Value")
    if let Some(headers) = &args.headers {
        for h in headers {
            if let Some((key, val)) = h.split_once(':') {
                req = req.header(key.trim(), val.trim());
            }
        }
    }

    if let Some(cookies) = &args.cookie {
        let cookie_str = cookies.join("; ");
        req = req.header(reqwest::header::COOKIE, cookie_str);
    }

    let resp = req
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

    loop {
        let next = match tokio::time::timeout(STALL_TIMEOUT, stream.next()).await {
            Ok(Some(result)) => result.map_err(|e| anyhow!("stream error: {}", e))?,
            Ok(None) => break, // stream ended normally
            Err(_) => {
                return Err(anyhow!(
                    "data stall: no data received in {}s",
                    STALL_TIMEOUT.as_secs()
                ))
            }
        };

        file.write_all(&next).await?;
        written += next.len() as u64;
        *total_written += next.len() as u64;
        pb.inc(next.len() as u64);

        if written > expected {
            return Err(anyhow!("received more data than expected: {} > {}", written, expected));
        }
    }

    file.sync_all().await?;

    if written != expected {
        return Err(anyhow!("incomplete: got {} of {} bytes", written, expected));
    }

    Ok(())
}