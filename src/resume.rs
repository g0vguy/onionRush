use crate::chunker::Chunk;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChunkState {
    pub index: usize,
    pub start: u64,
    pub end: u64,
    pub completed: bool,
    pub sha256: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Manifest {
    pub url: String,
    pub output: PathBuf,
    pub total_size: u64,
    pub chunks: Vec<ChunkState>,
    pub created_at: String,
    pub updated_at: String,
}

impl Manifest {
    /// Sidecar path for a given output file, e.g. `video.mp4` -> `video.mp4.onionrush-resume.json`.
    pub fn path_for(output: &Path) -> PathBuf {
        let mut name = output.as_os_str().to_owned();
        name.push(".onionrush-resume.json");
        PathBuf::from(name)
    }

    pub fn fresh(url: &str, output: &Path, total_size: u64, chunks: &[Chunk]) -> Manifest {
        let now = chrono::Utc::now().to_rfc3339();
        Manifest {
            url: url.to_string(),
            output: output.to_path_buf(),
            total_size,
            chunks: chunks
                .iter()
                .map(|c| ChunkState {
                    index: c.index,
                    start: c.start,
                    end: c.end,
                    completed: false,
                    sha256: None,
                })
                .collect(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn load(output: &Path) -> Option<Manifest> {
        let path = Self::path_for(output);
        let data = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn save(&mut self, output: &Path) -> Result<()> {
        self.updated_at = chrono::Utc::now().to_rfc3339();
        let path = Self::path_for(output);
        let tmp = path.with_extension("json.tmp");
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp, data)?;
        std::fs::rename(&tmp, &path)?; // atomic on same filesystem
        Ok(())
    }

    pub fn delete(output: &Path) {
        let _ = std::fs::remove_file(Self::path_for(output));
    }

    pub fn chunk_layout_matches(&self, planned: &[Chunk]) -> bool {
        self.chunks.len() == planned.len()
            && self
                .chunks
                .iter()
                .zip(planned.iter())
                .all(|(m, p)| m.index == p.index && m.start == p.start && m.end == p.end)
    }
}

pub async fn hash_range(path: &Path, start: u64, end: u64) -> Result<String> {
    let mut file = File::open(path).await?;
    file.seek(SeekFrom::Start(start)).await?;

    let mut remaining = end - start + 1;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20]; // 1 MiB read buffer

    while remaining > 0 {
        let to_read = remaining.min(buf.len() as u64) as usize;
        let n = file.read(&mut buf[..to_read]).await?;
        if n == 0 {
            return Err(anyhow!("unexpected EOF while hashing range {start}-{end}"));
        }
        hasher.update(&buf[..n]);
        remaining -= n as u64;
    }

    Ok(hex::encode(hasher.finalize()))
}