use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(name = "onionRush", version = "1.0.0", about = "Parallel multi-circuit downloader and uploader over Tor")]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    Download(DownloadArgs),
    Upload(UploadArgs),
}

#[derive(Parser, Debug, Clone)]
pub struct DownloadArgs {
    pub url: String,

    #[arg(short, long)]
    pub output: Option<String>,

    #[arg(short = 'n', long, default_value_t = 8)]
    pub circuits: usize,

    #[arg(long, default_value = "127.0.0.1:9050")]
    pub socks: String,

    #[arg(short, long, default_value_t = 4)]
    pub retries: u32,

    #[arg(short, long, default_value_t = 120)]
    pub timeout: u64,

    #[arg(long)]
    pub chunk_size_mb: Option<u64>,

    /// Extra request headers, e.g. -H "Authorization: Bearer token" (repeatable)
    #[arg(short = 'H', long = "header")]
    pub headers: Option<Vec<String>>,

    /// Cookie string(s), e.g. --cookie "session=abc123" (repeatable)
    #[arg(long)]
    pub cookie: Option<Vec<String>>,

    /// Override the User-Agent header
    #[arg(long)]
    pub user_agent: Option<String>,

    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct UploadArgs {
    pub url: String,

    #[arg(short, long)]
    pub file: String,

    #[arg(short = 'H', long)]
    pub headers: Option<Vec<String>>,

    #[arg(short = 'C', long)]
    pub cookies: Option<Vec<String>>,

    #[arg(long)]
    pub interval: Option<String>,

    pub chunk_size: u64,

    #[arg(short = 'n', long, default_value_t = 4)]
    pub streams: usize,

    #[arg(long, default_value = "127.0.0.1:9050")]
    pub socks: String,

    #[arg(short, long, default_value_t = 3)]
    pub retries: u32,

    #[arg(short, long, default_value_t = 60)]
    pub timeout: u64,

    #[arg(long)]
    pub session_pause_chance: Option<f64>,

    #[arg(long, default_value_t = 60)]
    pub session_pause_min: u64,

    #[arg(long, default_value_t = 300)]
    pub session_pause_max: u64,

    #[arg(long)]
    pub session_window: Option<f64>,

    #[arg(long, default_value = "file")]
    pub field_file: String,

    #[arg(long, default_value = "chunk_index")]
    pub field_index: String,

    #[arg(long, default_value = "chunk_offset")]
    pub field_offset: String,

    #[arg(long, default_value = "chunk_size")]
    pub field_size: String,

    #[arg(long)]
    pub randomize_fields: bool,

    #[arg(long)]
    pub reuse_connections: bool,

    #[arg(long)]
    pub strip_metadata: bool,

    #[arg(long)]
    pub skip_isolation_check: bool,

    #[arg(short, long)]
    pub verbose: bool,
}