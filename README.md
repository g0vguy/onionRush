# onionRush 🧅

Parallel multi-circuit downloader and uploader over Tor for high-speed anonymous file transfers.

[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tor](https://img.shields.io/badge/Tor-Compatible-purple.svg)](https://www.torproject.org/)
[![Version](https://img.shields.io/badge/version-1.0.0-green.svg)](https://github.com/g0vguy/onionRush)

## Features

### Download
- **Parallel Downloads**: Split files into chunks and download simultaneously
- **Tor Circuit Isolation**: Each chunk uses a unique SOCKS5 auth identity for separate circuits
- **Resume Support**: Interrupted downloads resume from the last verified chunk
- **Stall Detection**: Automatically detects and recovers from stalled connections
- **Multi-Progress Bars**: Real-time per-chunk progress display
- **Integrity Verification**: SHA-256 per-chunk hashing and file size validation
- **Flexible Chunking**: Fixed chunk count or custom chunk size in MB

### Upload
- **Parallel Uploads**: Chunked multipart upload across multiple Tor streams
- **Tor Circuit Isolation**: Each stream uses an isolated SOCKS5 identity
- **Paced Scheduling**: Randomised (log-normal) or fixed inter-chunk delays to avoid pattern fingerprinting
- **Session Pausing**: Optional probabilistic session-level gaps to simulate human behaviour
- **Metadata Stripping**: Automatic EXIF/document metadata removal via `mat2` before upload
- **Isolation Verification**: Self-test confirms SOCKS identities route through different Tor exit IPs
- **Custom Form Fields**: Configurable multipart field names with optional randomised field ordering
- **Connection Reuse**: Optional persistent HTTP connections per stream for compatible endpoints
- **Exponential Backoff**: Automatic retry with jittered backoff on failure

## How It Works

onionRush leverages Tor's `IsolateSOCKSAuth` feature to create independent circuits per chunk. By assigning random credentials to each SOCKS5 connection, traffic is spread across multiple Tor circuits simultaneously.

```
Chunk 0 ──► Circuit A (Identity: rush1a2b3c)
Chunk 1 ──► Circuit B (Identity: rush4d5e6f)
Chunk 2 ──► Circuit C (Identity: rush7g8h9i)
   ...         ...
Chunk N ──► Circuit N (Identity: rush...)
```

## Installation

### From Source

```bash
git clone https://github.com/g0vguy/onionRush.git
cd onionRush
cargo build --release
```

Binary at `target/release/onionRush.exe` (Windows) or `target/release/onionRush` (Unix).

### From Crates.io

```bash
cargo install onionRush
```

## Requirements

- [Tor](https://www.torproject.org/) running with SOCKS5 proxy enabled
- Tor configured with `IsolateSOCKSAuth` (default on most distributions)
- *(Optional)* [`mat2`](https://0xacab.org/jvoisin/mat2) for metadata stripping on upload

### Tor Configuration

Add to your `torrc`:
```
SocksPort 9050 IsolateSOCKSAuth
```

## Usage

```
onionRush <COMMAND>

Commands:
  download    Download a file over Tor using parallel circuits
  upload      Upload a file over Tor using parallel streams
```

---

### Download

```bash
onionRush download [OPTIONS] <URL>
```

| Option | Description | Default |
|--------|-------------|---------|
| `-o, --output <PATH>` | Output file path | Filename from URL |
| `-n, --circuits <NUM>` | Number of parallel circuits/chunks | 8 |
| `--socks <ADDR>` | Tor SOCKS5 proxy address | 127.0.0.1:9050 |
| `-r, --retries <NUM>` | Retries per chunk | 4 |
| `-t, --timeout <SEC>` | Per-request timeout in seconds | 120 |
| `--chunk-size-mb <MB>` | Chunk size in MB (overrides `--circuits`) | Auto |
| `-v, --verbose` | Verbose logging | Disabled |

#### Examples

```bash
# Basic download
onionRush download http://example.onion/file.zip

# 16 parallel circuits
onionRush download http://example.onion/file.zip -n 16

# Fixed 1 GB chunks
onionRush download http://example.onion/file.zip --chunk-size-mb 1024

# Verbose with increased timeout and retries
onionRush download http://example.onion/file.zip -v --timeout 300 -r 10
```

---

### Upload

```bash
onionRush upload [OPTIONS] <URL> --file <FILE> <CHUNK_SIZE>
```

| Option | Description | Default |
|--------|-------------|---------|
| `-f, --file <PATH>` | File to upload | *(required)* |
| `<CHUNK_SIZE>` | Chunk size in bytes | *(required)* |
| `-n, --streams <NUM>` | Number of parallel upload streams | 4 |
| `--socks <ADDR>` | Tor SOCKS5 proxy address | 127.0.0.1:9050 |
| `-r, --retries <NUM>` | Retries per chunk | 3 |
| `-t, --timeout <SEC>` | Per-request timeout in seconds | 60 |
| `--interval <SPEC>` | Delay between chunks: `rand`, `rand:mean=<s>,jitter=<s>`, or integer seconds | None |
| `--session-pause-chance <0.0-1.0>` | Probability of injecting a session-level gap between chunks | None |
| `--session-pause-min <SEC>` | Minimum session gap duration | 60 |
| `--session-pause-max <SEC>` | Maximum session gap duration | 300 |
| `--session-window <HOURS>` | Spread upload evenly across a time window | None |
| `--field-file <NAME>` | Multipart field name for file data | `file` |
| `--field-index <NAME>` | Multipart field name for chunk index | `chunk_index` |
| `--field-offset <NAME>` | Multipart field name for chunk offset | `chunk_offset` |
| `--field-size <NAME>` | Multipart field name for chunk size | `chunk_size` |
| `--randomize-fields` | Randomise multipart field ordering per request | Disabled |
| `--reuse-connections` | Reuse HTTP connections per stream | Disabled |
| `--strip-metadata` | Strip file metadata via `mat2` before upload | Disabled |
| `--skip-isolation-check` | Skip SOCKS circuit isolation self-test | Disabled |
| `-H, --headers <KEY: VALUE>` | Extra request headers (repeatable) | None |
| `-C, --cookies <VALUE>` | Cookie strings (repeatable) | None |
| `-v, --verbose` | Verbose logging | Disabled |

#### Examples

```bash
# Basic upload, 4 streams, 10 MB chunks
onionRush upload http://example.onion/upload --file secret.zip 10485760

# 8 streams, randomised inter-chunk delay averaging 30s
onionRush upload http://example.onion/upload --file data.tar.gz 5242880 -n 8 --interval rand:mean=30,jitter=15

# Strip metadata, custom auth header, spread over 2-hour window
onionRush upload http://example.onion/upload --file document.pdf 2097152 \
  --strip-metadata \
  --session-window 2.0 \
  -H "Authorization: Bearer <token>"

# Reuse connections, randomise fields, skip isolation check
onionRush upload http://example.onion/upload --file archive.zip 10485760 \
  --reuse-connections \
  --randomize-fields \
  --skip-isolation-check
```

---

## Output Example

```
[+] onionRush v1.0.0 :: Download mode
[+] size: 10737418240 bytes (10.00 GB), range support: true
[*] Downloading 8 chunk(s)
   chunk 0 [==============================] 1.25 GiB/1.25 GiB 9.63 MiB/s 0s
   chunk 1 [==============================] 1.25 GiB/1.25 GiB 9.63 MiB/s 0s
   chunk 2 [==============================] 1.25 GiB/1.25 GiB 9.63 MiB/s 0s
   chunk 3 [==============================] 1.25 GiB/1.25 GiB 9.63 MiB/s 0s
   chunk 4 [==============================] 1.25 GiB/1.25 GiB 9.63 MiB/s 0s
   chunk 5 [==============================] 1.25 GiB/1.25 GiB 9.63 MiB/s 0s
   chunk 6 [==============================] 1.25 GiB/1.25 GiB 9.63 MiB/s 0s
   chunk 7 [==============================] 1.25 GiB/1.25 GiB 9.63 MiB/s 0s
[*] Progress: 100.0% (8 completed, 0 failed)
[+] Download complete! File saved to: "file.zip"
[+] Total size: 10737418240 bytes (10.00 GB)
```

## Performance Tips

1. **Circuits/Streams**: Start with `-n 8` for downloads, `-n 4` for uploads, and scale up
2. **Tor Bandwidth**: Raise limits in `torrc` if you control the relay:
   ```
   RelayBandwidthRate 50 MB
   RelayBandwidthBurst 100 MB
   ```
3. **Chunk Size**: Use `--chunk-size-mb` for large files to control memory usage
4. **Timeouts**: Slow onion services may need `--timeout 300` or higher
5. **Upload Pacing**: Use `--interval rand:mean=20,jitter=10` to blend into normal traffic patterns

## Troubleshooting

### "failed to reach target host - is it online and is the SOCKS proxy reachable?"

- Confirm Tor is running: `systemctl status tor` (Linux) or check Windows services
- Verify SOCKS address matches `torrc` (default `127.0.0.1:9050`)
- Confirm the target is reachable via Tor Browser first

### Chunks timing out or stalling

- Increase timeout: `--timeout 300`
- Reduce parallelism: `-n 4`
- Add more retries: `-r 10`

### Slow speeds

- Increase circuit count: `-n 16` or `-n 32`
- Tor will cycle exit nodes automatically across retries
- Check Tor bandwidth configuration

### "File size mismatch"

- Use `--chunk-size-mb` for finer-grained chunks
- Increase retries and timeout
- The remote file may have changed during the download

### Upload isolation check warning

- Add `IsolateSOCKSAuth` to your `torrc` `SocksPort` line
- Use `--skip-isolation-check` to bypass the check if you're certain isolation is configured elsewhere

## Building for Production

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

## Security Considerations

- All traffic is routed exclusively through Tor's SOCKS5 proxy
- Unique SOCKS5 credentials per chunk enforce circuit isolation
- Upload pacing and session gaps reduce traffic-analysis correlation
- `--strip-metadata` removes EXIF/document properties before transmission
- No sensitive data is logged at the default log level

## License

MIT License — see [LICENSE](LICENSE) for details.

## Disclaimer

This tool is for educational and legitimate purposes only. Users are responsible for complying with all applicable laws and regulations. The authors assume no liability for misuse.
