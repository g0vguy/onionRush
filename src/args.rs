use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "onionRush", version, about = "parallel multi-circuit downloader over Tor")]
pub struct Args {
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

    #[arg(short, long)]
    pub verbose: bool,
}