use crate::args::UploadArgs;
use crate::proxy;
use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use rand::seq::SliceRandom;
use rand::Rng;
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

pub struct UploadChunk {
    pub index: usize,
    pub data: Bytes,
    pub offset: u64,
    pub size: usize,
}

#[derive(Debug, Clone, Copy)]
enum Schedule {
    Random { mean: f64, jitter: f64 },
    Fixed(u64),
    Immediate,
}

enum FormField {
    File(String, Part),
    Text(String, String),
}

pub async fn upload_file(args: &UploadArgs) -> Result<()> {
    if !args.skip_isolation_check {
        verify_socks_isolation(&args.socks).await?;
    }

    let mut file_path_str = args.file.clone();
    if args.strip_metadata {
        file_path_str = strip_file_metadata(&args.file).await?;
    } else {
        warn_metadata_leak(&args.file);
    }

    let file_path = Path::new(&file_path_str);
    if !file_path.exists() {
        return Err(anyhow!("File not found: {}", file_path_str));
    }

    let file_size = file_path.metadata()?.len();
    info!("[+] Uploading: {} ({:.2} MB)", file_path_str, file_size as f64 / 1024.0 / 1024.0);
    info!("[*] Base chunk size: {} bytes (jittered +/-20%)", args.chunk_size);
    info!("[*] Streams: {}", args.streams);

    let chunks = split_file_into_chunks(file_path, args.chunk_size)?;
    info!("[*] Split into {} chunks", chunks.len());

    let headers = parse_headers(&args.headers);
    let cookies = parse_cookies(&args.cookies);
    let schedule = parse_interval(&args.interval)?;
    info!("[*] Upload schedule: {:?}", schedule);

    upload_chunks_parallel(args, chunks, headers, cookies, schedule).await?;

    if args.strip_metadata && file_path_str != args.file {
        let _ = std::fs::remove_file(&file_path_str);
    }

    info!("[+] Upload completed successfully!");
    Ok(())
}

fn warn_metadata_leak(file_path: &str) {
    if let Some(ext) = Path::new(file_path).extension().and_then(|s| s.to_str()) {
        let dangerous = ["jpg", "jpeg", "png", "pdf", "docx", "xlsx", "pptx", "zip"];
        if dangerous.contains(&ext.to_lowercase().as_str()) {
            warn!("[!] WARNING: Uploading .{} without --strip-metadata may leak EXIF/document properties.", ext);
        }
    }
}

async fn strip_file_metadata(file_path: &str) -> Result<String> {
    info!("[*] Stripping metadata from file: {}", file_path);
    let path = Path::new(file_path);
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("tmp");
    let random_id = proxy::random_identity();
    let cleaned_path = format!("{}_cleaned.{}", random_id, ext);

    std::fs::copy(file_path, &cleaned_path)?;

    let status = tokio::process::Command::new("mat2")
        .arg(&cleaned_path)
        .status()
        .await;

    match status {
        Ok(s) if s.success() => {
            let mat2_out = format!("{}.cleaned", cleaned_path);
            if Path::new(&mat2_out).exists() {
                let _ = std::fs::remove_file(&cleaned_path);
                info!("[+] Metadata successfully stripped using mat2.");
                return Ok(mat2_out);
            }
        }
        _ => {
            warn!("[!] mat2 failed or is not installed. Aborting due to --strip-metadata constraint.");
            let _ = std::fs::remove_file(&cleaned_path);
            return Err(anyhow!("Failed to strip metadata. Ensure mat2 is installed on your path."));
        }
    }

    Ok(cleaned_path)
}

async fn verify_socks_isolation(socks_addr: &str) -> Result<()> {
    info!("[*] Performing Tor SOCKS isolation validation check...");
    let id_a = proxy::random_identity();
    let id_b = proxy::random_identity();

    let client_a = proxy::build_client(socks_addr, &id_a, Duration::from_secs(20), false)?;
    let client_b = proxy::build_client(socks_addr, &id_b, Duration::from_secs(20), false)?;

    let fetch_ip = |client: Client| async move {
        client.get("https://api.ipify.org")
            .send()
            .await?
            .text()
            .await
    };

    let (ip_a, ip_b) = tokio::join!(
        fetch_ip(client_a),
        fetch_ip(client_b)
    );

    match (ip_a, ip_b) {
        (Ok(a), Ok(b)) => {
            let a = a.trim();
            let b = b.trim();
            if a == b {
                warn!("[!] WARNING: Tor circuit isolation check failed. Both SOCKS credentials resolved to IP: {}", a);
                warn!("[!] Ensure that 'IsolateSOCKSAuth' is configured inside your torrc.");
            } else {
                info!("[+] Tor circuit isolation check passed: SOCKS identities mapped to different exit IPs.");
            }
        }
        _ => {
            warn!("[!] SOCKS isolation self-test could not complete (destination server offline or unreachable). Proceeding...");
        }
    }
    Ok(())
}

fn split_file_into_chunks(path: &Path, base_chunk_size: u64) -> Result<Vec<UploadChunk>> {
    let mut file = File::open(path)?;
    let mut chunks = Vec::new();
    let mut offset = 0u64;
    let mut index = 0;
    let mut rng = rand::thread_rng();

    loop {
        let jitter_factor: f64 = rng.gen_range(0.8..1.2);
        let target_size = ((base_chunk_size as f64) * jitter_factor).round() as usize;
        let mut buffer = vec![0u8; target_size.max(1)];

        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        let data = Bytes::from(buffer[..bytes_read].to_vec());
        chunks.push(UploadChunk {
            index,
            data,
            offset,
            size: bytes_read,
        });
        offset += bytes_read as u64;
        index += 1;
    }

    Ok(chunks)
}

fn parse_headers(headers: &Option<Vec<String>>) -> Vec<(String, String)> {
    let mut parsed = Vec::new();
    if let Some(headers) = headers {
        for header in headers {
            if let Some((key, value)) = header.split_once(": ") {
                parsed.push((key.to_string(), value.to_string()));
            }
        }
    }
    parsed
}

fn parse_cookies(cookies: &Option<Vec<String>>) -> Vec<String> {
    cookies.clone().unwrap_or_default()
}

fn parse_interval(interval: &Option<String>) -> Result<Schedule> {
    let s = match interval {
        None => return Ok(Schedule::Immediate),
        Some(s) => s,
    };

    if s == "rand" {
        return Ok(Schedule::Random { mean: 15.0, jitter: 10.0 });
    }

    if let Some(params) = s.strip_prefix("rand:") {
        let mut mean = 15.0_f64;
        let mut jitter = 10.0_f64;

        for kv in params.split(',') {
            let kv = kv.trim();
            if kv.is_empty() {
                continue;
            }
            let mut parts = kv.splitn(2, '=');
            let key = parts.next().unwrap_or("").trim();
            let val = parts.next().unwrap_or("").trim();
            match key {
                "mean" => {
                    mean = val
                        .parse::<f64>()
                        .map_err(|_| anyhow!("invalid mean value '{val}' in --interval"))?;
                }
                "jitter" => {
                    jitter = val
                        .parse::<f64>()
                        .map_err(|_| anyhow!("invalid jitter value '{val}' in --interval"))?;
                }
                other => {
                    return Err(anyhow!(
                        "unknown --interval param '{other}', expected 'mean' or 'jitter'"
                    ));
                }
            }
        }

        if mean <= 0.0 {
            return Err(anyhow!("--interval mean must be > 0"));
        }

        return Ok(Schedule::Random { mean, jitter: jitter.max(0.0) });
    }

    s.parse::<u64>().map(Schedule::Fixed).map_err(|_| {
        anyhow!(
            "--interval must be 'rand', 'rand:mean=<secs>,jitter=<secs>', or an integer (seconds); got '{s}'"
        )
    })
}

fn sample_delay_secs(mean: f64, jitter: f64) -> u64 {
    let mut rng = rand::thread_rng();

    let u1: f64 = rng.gen_range(0.0001_f64..1.0);
    let u2: f64 = rng.gen_range(0.0_f64..1.0);
    let z0 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();

    let mean = mean.max(0.5);
    let cv = (jitter / mean).clamp(0.05, 2.0);
    let sigma = (1.0 + cv * cv).ln().sqrt();
    let mu = mean.ln() - 0.5 * sigma * sigma;

    let sample = (mu + sigma * z0).exp();
    sample.clamp(0.5, mean * 6.0).round() as u64
}

async fn upload_chunks_parallel(
    args: &UploadArgs,
    mut chunks: Vec<UploadChunk>,
    headers: Vec<(String, String)>,
    cookies: Vec<String>,
    schedule: Schedule,
) -> Result<()> {
    use tokio::sync::Semaphore;
    use tokio::task::JoinSet;

    let semaphore = Arc::new(Semaphore::new(args.streams));
    let mut set = JoinSet::new();

    {
        let mut rng = rand::thread_rng();
        chunks.shuffle(&mut rng);
    }

    let total_chunks = chunks.len();
    let mut uploaded = 0;

    let mut persistent_clients = Vec::new();
    if args.reuse_connections {
        for _ in 0..args.streams {
            let identity = proxy::random_identity();
            let client = proxy::build_client(&args.socks, &identity, Duration::from_secs(args.timeout), true)?;
            persistent_clients.push(Arc::new(client));
        }
    }

    let start_time = std::time::Instant::now();
    let session_window_secs = args.session_window.map(|w| w * 3600.0);

    for (i, chunk) in chunks.into_iter().enumerate() {
        let permit = semaphore.clone().acquire_owned().await?;
        let args_c = args.clone();
        let headers_c = headers.clone();
        let cookies_c = cookies.clone();

        let client = if args.reuse_connections {
            let client_idx = i % args.streams;
            Some(persistent_clients[client_idx].clone())
        } else {
            None
        };

        if i > 0 {
            if let Some(chance) = args.session_pause_chance {
                let mut rng = rand::thread_rng();
                if rng.gen_bool(chance) {
                    let pause = rng.gen_range(args.session_pause_min..=args.session_pause_max);
                    info!("[*] Injecting session-level off gap: {} seconds", pause);
                    sleep(Duration::from_secs(pause)).await;
                }
            }

            let active_schedule = if let Some(total_secs) = session_window_secs {
                let elapsed = start_time.elapsed().as_secs_f64();
                let remaining_time = (total_secs - elapsed).max(0.0);
                let remaining_chunks = (total_chunks - i) as f64;
                let current_mean = if remaining_chunks > 1.0 {
                    remaining_time / remaining_chunks
                } else {
                    0.0
                };
                Schedule::Random {
                    mean: current_mean,
                    jitter: current_mean * 0.5,
                }
            } else {
                schedule
            };

            match active_schedule {
                Schedule::Random { mean, jitter } => {
                    if mean > 0.0 {
                        let delay = sample_delay_secs(mean, jitter);
                        info!("[*] Paced delay: {}s (mean={:.1}s, jitter={:.1}s)", delay, mean, jitter);
                        sleep(Duration::from_secs(delay)).await;
                    }
                }
                Schedule::Fixed(secs) => {
                    info!("[*] Fixed delay: {} seconds", secs);
                    sleep(Duration::from_secs(secs)).await;
                }
                Schedule::Immediate => {}
            }
        }

        set.spawn(async move {
            let result = upload_single_chunk(&args_c, &chunk, &headers_c, &cookies_c, client).await;
            drop(permit);
            result
        });
    }

    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(())) => {
                uploaded += 1;
                let progress = (uploaded as f64 / total_chunks as f64) * 100.0;
                info!("[*] Upload progress: {:.1}% ({}/{})", progress, uploaded, total_chunks);
            }
            Ok(Err(e)) => {
                error!("[-] Chunk upload failed: {}", e);
            }
            Err(e) => {
                error!("[-] Task panicked: {}", e);
            }
        }
    }

    Ok(())
}

async fn upload_single_chunk(
    args: &UploadArgs,
    chunk: &UploadChunk,
    headers: &[(String, String)],
    cookies: &[String],
    client: Option<Arc<Client>>,
) -> Result<()> {
    let mut retries = 0;
    while retries < args.retries {
        let active_client = match &client {
            Some(c) => c.as_ref().clone(),
            None => {
                let identity = proxy::random_identity();
                match proxy::build_client(&args.socks, &identity, Duration::from_secs(args.timeout), false) {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("[!] Failed to build client: {}", e);
                        retries += 1;
                        continue;
                    }
                }
            }
        };

        let data_vec = chunk.data.to_vec();
        let part = Part::bytes(data_vec)
            .file_name(format!("chunk_{}", chunk.index))
            .mime_str("application/octet-stream")?;

        let mut form_fields = vec![
            FormField::File(args.field_file.clone(), part),
            FormField::Text(args.field_index.clone(), chunk.index.to_string()),
            FormField::Text(args.field_offset.clone(), chunk.offset.to_string()),
            FormField::Text(args.field_size.clone(), chunk.size.to_string()),
        ];

        if args.randomize_fields {
            form_fields.shuffle(&mut rand::thread_rng());
        }

        let mut form = Form::new();
        for field in form_fields {
            form = match field {
                FormField::File(name, value) => form.part(name, value),
                FormField::Text(name, value) => form.text(name, value),
            };
        }

        let mut request = active_client.post(&args.url).multipart(form);

        for (key, value) in headers {
            request = request.header(key, value);
        }

        for cookie in cookies {
            request = request.header("Cookie", cookie);
        }

        request = request.header("X-Request-ID", uuid::Uuid::new_v4().to_string());
        request = request.header("X-Timestamp", chrono::Utc::now().to_rfc3339());

        match request.send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    debug!("[+] Chunk {} uploaded successfully", chunk.index);
                    return Ok(());
                } else {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    warn!("[!] Upload failed with status {}: {}", status, text);
                    retries += 1;
                }
            }
            Err(e) => {
                warn!("[!] Upload error: {}", e);
                retries += 1;
            }
        }

        let backoff = {
            let mut rng = rand::thread_rng();
            let base = 2.0_f64;
            let max_backoff = 60.0_f64;
            let limit = (base * 2.0_f64.powi(retries as i32)).min(max_backoff);
            rng.gen_range(0.5..limit).round() as u64
        };
        info!("[*] Retry {} in {} seconds", retries, backoff);
        sleep(Duration::from_secs(backoff)).await;
    }

    Err(anyhow!("Failed to upload chunk {} after {} retries", chunk.index, args.retries))
}