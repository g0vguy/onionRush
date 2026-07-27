use clap::{Parser, Subcommand};

pub const DEFAULT_CIRCUITS: usize = 8;
pub const DEFAULT_SOCKS: &str = "127.0.0.1:9050";
pub const DEFAULT_RETRIES: u32 = 4;
pub const DEFAULT_TIMEOUT: u64 = 120;

#[derive(Parser, Debug, Clone)]
#[command(name = "onionRush", version, about = "Parallel multi-circuit downloader and uploader over Tor")]
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

    /// Number of parallel circuits/chunks (default: 8, overridable via --config)
    #[arg(short = 'n', long)]
    pub circuits: Option<usize>,

    /// Tor SOCKS5 proxy address (default: 127.0.0.1:9050, overridable via --config)
    #[arg(long)]
    pub socks: Option<String>,

    /// Retries per chunk before giving up (default: 4, overridable via --config)
    #[arg(short, long)]
    pub retries: Option<u32>,

    /// Per-request timeout in seconds (default: 120, overridable via --config)
    #[arg(short, long)]
    pub timeout: Option<u64>,

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

    /// Expected SHA-256 of the completed file. Optional - not every host publishes one.
    #[arg(long)]
    pub sha256: Option<String>,

    #[arg(long)]
    pub config: Option<String>,

    #[arg(short, long)]
    pub verbose: bool,

    #[arg(short = 'q', long)]
    pub quiet: bool,
}

impl DownloadArgs {
    pub fn circuits(&self) -> usize {
        self.circuits.unwrap_or(DEFAULT_CIRCUITS)
    }

    pub fn socks(&self) -> &str {
        self.socks.as_deref().unwrap_or(DEFAULT_SOCKS)
    }

    pub fn retries(&self) -> u32 {
        self.retries.unwrap_or(DEFAULT_RETRIES)
    }

    pub fn timeout(&self) -> u64 {
        self.timeout.unwrap_or(DEFAULT_TIMEOUT)
    }

    pub fn apply_config(&mut self, config: &crate::config::ConfigFile) {
        if self.circuits.is_none() {
            self.circuits = config.circuits;
        }
        if self.socks.is_none() {
            self.socks = config.socks.clone();
        }
        if self.retries.is_none() {
            self.retries = config.retries;
        }
        if self.timeout.is_none() {
            self.timeout = config.timeout;
        }
        if self.chunk_size_mb.is_none() {
            self.chunk_size_mb = config.chunk_size_mb;
        }
        if self.user_agent.is_none() {
            self.user_agent = config.user_agent.clone();
        }
    }
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