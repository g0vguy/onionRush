use anyhow::{anyhow, Result};
use reqwest::Client;

pub struct Target {
    pub url: String,
    pub size: u64,
    pub accepts_ranges: bool,
}

pub struct Chunk {
    pub index: usize,
    pub start: u64,
    pub end: u64,
}

pub async fn probe(client: &Client, url: &str) -> Result<Target> {
    let resp = client.head(url).send().await?;

    if !resp.status().is_success() {
        return Err(anyhow!("head request failed: {}", resp.status()));
    }

    let size = resp
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or_else(|| anyhow!("server did not report content-length"))?;

    let accepts_ranges = resp
        .headers()
        .get(reqwest::header::ACCEPT_RANGES)
        .map(|v| v != "none")
        .unwrap_or(false);

    Ok(Target {
        url: url.to_string(),
        size,
        accepts_ranges,
    })
}

pub fn plan_chunks(size: u64, count: usize) -> Vec<Chunk> {
    let count = count.max(1) as u64;
    let base = size / count;
    let remainder = size % count;

    let mut chunks = Vec::new();
    let mut offset = 0u64;

    for i in 0..count {
        let extra = if i < remainder { 1 } else { 0 };
        let len = base + extra;
        if len == 0 {
            continue;
        }
        let start = offset;
        let end = start + len - 1;
        chunks.push(Chunk {
            index: i as usize,
            start,
            end,
        });
        offset = end + 1;
    }

    chunks
}

pub fn plan_chunks_by_size(size: u64, chunk_size_mb: u64) -> Vec<Chunk> {
    let chunk_size = chunk_size_mb * 1024 * 1024;
    let count = (size + chunk_size - 1) / chunk_size;
    
    let mut chunks = Vec::new();
    let mut offset = 0u64;
    
    for i in 0..count {
        let start = offset;
        let end = (offset + chunk_size - 1).min(size - 1);
        chunks.push(Chunk {
            index: i as usize,
            start,
            end,
        });
        offset = end + 1;
    }
    
    chunks
}