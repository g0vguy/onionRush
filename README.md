# onionRush 🧅

Parallel multi-circuit downloader over Tor for high-speed anonymous downloads.

[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tor](https://img.shields.io/badge/Tor-Compatible-purple.svg)](https://www.torproject.org/)

## Features

- **Parallel Downloads**: Split files into chunks and download simultaneously
- **Tor Circuit Isolation**: Each chunk uses a unique SOCKS5 auth identity for separate circuits
- **Resume Support**: Failed chunks resume from where they left off
- **Stall Detection**: Automatically detects and handles stalled connections
- **Multi-Progress Bars**: Real-time progress for each chunk
- **Verification**: Validates chunk integrity and file size after download
- **Flexible Chunking**: Custom chunk size or auto-distribution

## How It Works

onionRush leverages Tor's `IsolateSOCKSAuth` feature to create separate circuits for each download chunk. By assigning random authentication credentials to each request, the download is spread across multiple Tor circuits, dramatically improving download speeds over single-circuit solutions.

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

The binary will be at `target/release/onionRush.exe` (Windows) or `target/release/onionRush` (Unix).

### From Crates.io

```bash
cargo install onionRush
```

## Requirements

- [Tor](https://www.torproject.org/) running with SOCKS5 proxy enabled
- Tor configured with `IsolateSOCKSAuth` (default on most distributions)

### Tor Configuration

Add to your `torrc` file:
```
SocksPort 9050 IsolateSOCKSAuth
```

## Usage

### Basic Usage

```bash
onionRush http://example.onion/file.zip
```

### Advanced Options

```bash
onionRush [OPTIONS] <URL>
```

#### Options

| Option | Description | Default |
|--------|-------------|---------|
| `-o, --output <PATH>` | Output file path | Filename from URL |
| `-n, --circuits <NUM>` | Number of parallel circuits/chunks | 8 |
| `--socks <ADDR>` | Tor SOCKS5 proxy address | 127.0.0.1:9050 |
| `-r, --retries <NUM>` | Retries per chunk before giving up | 4 |
| `-t, --timeout <SEC>` | Per-request timeout in seconds | 120 |
| `--chunk-size-mb <MB>` | Chunk size in MB (overrides circuits) | Auto |
| `-v, --verbose` | Verbose logging | Disabled |
| `-h, --help` | Print help information | |
| `-V, --version` | Print version information | |

### Examples

#### Download with 16 parallel chunks
```bash
onionRush http://example.onion/file.zip -n 16
```

#### Download with custom chunk size (1GB chunks)
```bash
onionRush http://example.onion/file.zip --chunk-size-mb 1024
```

#### Download with verbose logging and custom timeout
```bash
onionRush http://example.onion/file.zip -v --timeout 300
```

#### Download with 32 chunks and 10 retries
```bash
onionRush http://example.onion/file.zip -n 32 -r 10
```

## Output Example

```
[+] onionRush :: probing target through tor
[+] size: 10737418240 bytes (10 GB), range support: true
[*] planning 8 chunks for download
   chunk 0 [==============================] 2.26 GiB/2.26 GiB 9.63 MiB/s 0s
   chunk 1 [==============================] 2.26 GiB/2.26 GiB 9.63 MiB/s 0s
   chunk 2 [==============================] 2.26 GiB/2.26 GiB 9.63 MiB/s 0s
   chunk 3 [==============================] 2.26 GiB/2.26 GiB 9.63 MiB/s 0s
   chunk 4 [==============================] 2.26 GiB/2.26 GiB 9.63 MiB/s 0s
   chunk 5 [==============================] 2.26 GiB/2.26 GiB 9.63 MiB/s 0s
   chunk 6 [==============================] 2.26 GiB/2.26 GiB 9.63 MiB/s 0s
   chunk 7 [==============================] 2.26 GiB/2.26 GiB 9.63 MiB/s 0s
[*] Progress: 100.0% (8 completed, 0 failed)
[*] Verifying downloaded file...
[+] Download complete! File saved to: "10GiB.zip"
[+] Total size: 10737418240 bytes (10 GB)
```

## Performance Tips

1. **Adjust Circuits**: Start with `-n 8` and increase based on your connection speed
2. **Tor Bandwidth**: Increase Tor's bandwidth limits in `torrc`:
   ```
   RelayBandwidthRate 50 MB
   RelayBandwidthBurst 100 MB
   ```
3. **Chunk Size**: For very large files, use `--chunk-size-mb` to control memory usage
4. **Retries**: Increase retries for unstable connections (`-r 10`)
5. **Timeout**: Set higher timeout for slow connections (`--timeout 300`)

## Troubleshooting

### "probe failed, is tor running on the socks port?"

- Ensure Tor is running: `systemctl status tor` (Linux) or check Windows services
- Verify SOCKS port: default is `127.0.0.1:9050`
- Check if `IsolateSOCKSAuth` is enabled in torrc

### Chunks are failing with timeout errors

- Increase timeout: `--timeout 300` or higher
- Reduce chunk count: `-n 4`
- Check your network connection
- Consider using more retries: `-r 10`

### Slow download speeds

- Increase circuit count: `-n 16` or `-n 32`
- Check Tor bandwidth settings
- Try different exit nodes (Tor will cycle automatically)
- Ensure you're not bandwidth-limited by your ISP

### "File size mismatch"

- Run with `--chunk-size-mb` to use smaller chunks
- Increase retries and timeout values
- The file may have been modified on the server

## Building for Production

For maximum performance:
```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

## Security Considerations

- All traffic is routed through Tor's SOCKS5 proxy
- Each chunk uses unique authentication to isolate circuits
- No personal information is stored or transmitted
- The tool does not log any sensitive data

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Disclaimer

This tool is for educational and legitimate purposes only. Users are responsible for complying with all applicable laws and regulations. The authors assume no liability for misuse.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request