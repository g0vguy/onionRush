#![allow(non_snake_case)]

mod args;
mod chunker;
mod download;
mod proxy;
mod resume;
mod upload;

use anyhow::{anyhow, Context, Result};
use args::{Args, Commands};
use clap::Parser;
use std::path::PathBuf;
use tracing::{error, warn};
use tracing_subscriber;

fn setup_logging(verbose: bool) {
    let env_filter = if verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .init();
}

fn output_path(url: &str, output: &Option<String>) -> PathBuf {
    if let Some(o) = output {
        return PathBuf::from(o);
    }

    let name = url
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or("download.bin");

    PathBuf::from(name)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Download(dl_args) => {
            setup_logging(dl_args.verbose);
            println!("[+] onionRush v1.0.0 :: Download mode");

            let out_path = output_path(&dl_args.url, &dl_args.output);

            let probe_client = proxy::build_client(
                &dl_args.socks,
                &proxy::random_identity(),
                std::time::Duration::from_secs(dl_args.timeout),
                false,
            )?;

            let target = chunker::probe(&probe_client, &dl_args.url)
                .await
                .context("failed to reach target host - is it online and is the SOCKS proxy reachable?")?;

            println!("[+] size: {} bytes ({:.2} GB), range support: {}",
                target.size,
                target.size as f64 / (1024.0 * 1024.0 * 1024.0),
                target.accepts_ranges
            );

            let planned_chunks = if let Some(chunk_size_mb) = dl_args.chunk_size_mb {
                chunker::plan_chunks_by_size(target.size, chunk_size_mb)
            } else {
                chunker::plan_chunks(target.size, dl_args.circuits)
            };

            let mut manifest = match resume::Manifest::load(&out_path) {
                Some(m)
                    if m.url == dl_args.url
                        && m.total_size == target.size
                        && m.chunk_layout_matches(&planned_chunks) =>
                {
                    println!("[*] Found resume state from a previous run, verifying integrity...");
                    m
                }
                Some(_) => {
                    println!("[!] Resume state found but doesn't match this run (URL, remote size, or chunk layout changed). Starting fresh.");
                    resume::Manifest::delete(&out_path);
                    resume::Manifest::fresh(&dl_args.url, &out_path, target.size, &planned_chunks)
                }
                None => resume::Manifest::fresh(&dl_args.url, &out_path, target.size, &planned_chunks),
            };


            let file_ok = out_path.exists()
                && std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0) == target.size;

            if !file_ok {
                if manifest.chunks.iter().any(|c| c.completed) {
                    println!("[!] Output file missing or wrong size; discarding resume state.");
                }
                let file = std::fs::File::create(&out_path)
                    .with_context(|| format!("could not create output file {:?}", out_path))?;
                file.set_len(target.size)?;
                drop(file);
                manifest = resume::Manifest::fresh(&dl_args.url, &out_path, target.size, &planned_chunks);
            }

            for cs in manifest.chunks.iter_mut() {
                if cs.completed {
                    match resume::hash_range(&out_path, cs.start, cs.end).await {
                        Ok(h) if Some(&h) == cs.sha256.as_ref() => {}
                        _ => {
                            println!("[!] Chunk {} failed integrity check, will be re-downloaded", cs.index);
                            cs.completed = false;
                            cs.sha256 = None;
                        }
                    }
                }
            }
            manifest.save(&out_path)?;

            let pending: Vec<chunker::Chunk> = planned_chunks
                .into_iter()
                .filter(|c| !manifest.chunks.iter().any(|cs| cs.index == c.index && cs.completed))
                .collect();

            let already_done = manifest.chunks.iter().filter(|c| c.completed).count();
            if already_done > 0 {
                println!(
                    "[*] Resuming: {} of {} chunks already verified complete, {} remaining",
                    already_done,
                    manifest.chunks.len(),
                    pending.len()
                );
            }

            let mut failures = 0;

            if pending.is_empty() {
                println!("[+] All chunks already complete and verified.");
            } else {
                println!("[*] Downloading {} chunk(s)", pending.len());

                let multi = indicatif::MultiProgress::new();
                let mut set = tokio::task::JoinSet::new();

                for chunk in pending {
                    let pb = multi.add(indicatif::ProgressBar::new(chunk.end - chunk.start + 1));
                    pb.set_style(style());
                    pb.set_prefix(format!("chunk {}", chunk.index));

                    let dl_args = dl_args.clone();
                    let url = target.url.clone();
                    let out_path_c = out_path.clone();
                    let index = chunk.index;
                    let start = chunk.start;
                    let end = chunk.end;

                    set.spawn(async move {
                        let result = download::download_chunk(&dl_args, &url, &chunk, &out_path_c, &pb).await;
                        match &result {
                            Ok(()) => pb.finish_with_message("[+] done"),
                            Err(e) => pb.finish_with_message(format!("[-] failed: {}", e)),
                        }
                        (index, start, end, result)
                    });
                }

                let mut completed = 0;
                let total = set.len();

                while let Some(res) = set.join_next().await {
                    match res {
                        Ok((index, start, end, Ok(()))) => {
                            match resume::hash_range(&out_path, start, end).await {
                                Ok(hash) => {
                                    if let Some(cs) = manifest.chunks.iter_mut().find(|c| c.index == index) {
                                        cs.completed = true;
                                        cs.sha256 = Some(hash);
                                    }
                                    if let Err(e) = manifest.save(&out_path) {
                                        warn!("[!] could not persist resume state: {}", e);
                                    }
                                    completed += 1;
                                }
                                Err(e) => {
                                    error!("[-] Chunk {} downloaded but could not be hashed: {}", index, e);
                                    failures += 1;
                                }
                            }
                        }
                        Ok((index, _, _, Err(e))) => {
                            error!("[-] Chunk {} failed: {}", index, e);
                            failures += 1;
                        }
                        Err(e) => {
                            error!("[-] Task panicked: {}", e);
                            failures += 1;
                        }
                    }
                    if total > 0 {
                        let progress = (completed + failures) as f64 / total as f64 * 100.0;
                        eprint!("\r[*] Progress: {:.1}% ({} completed, {} failed)", progress, completed, failures);
                    }
                }
                eprintln!();
            }

            if failures > 0 {
                println!(
                    "[-] {} chunk(s) failed. Resume state has been saved -- re-run the same command (same URL, output, --circuits/--chunk-size-mb) once the host is back to continue.",
                    failures
                );
                return Err(anyhow!("{} chunk(s) failed, output is incomplete", failures));
            }

            let metadata = std::fs::metadata(&out_path)?;
            if metadata.len() != target.size {
                return Err(anyhow!("File size mismatch: expected {}, got {}", target.size, metadata.len()));
            }

            resume::Manifest::delete(&out_path);

            println!("[+] Download complete! File saved to: {:?}", out_path);
            println!("[+] Total size: {} bytes ({:.2} GB)",
                target.size,
                target.size as f64 / (1024.0 * 1024.0 * 1024.0)
            );
        }
        Commands::Upload(up_args) => {
            setup_logging(up_args.verbose);
            println!("[+] onionRush v1.0.0 :: Upload mode");
            println!("[!] WARNING: This tool is for educational purposes only");
            println!("[!] Ensure you have permission to upload to the target");

            upload::upload_file(&up_args).await?;
        }
    }

    Ok(())
}

fn style() -> indicatif::ProgressStyle {
    indicatif::ProgressStyle::with_template("{prefix:>10} [{bar:30}] {bytes}/{total_bytes} {bytes_per_sec} {eta}")
        .unwrap()
        .progress_chars("=>-")
}