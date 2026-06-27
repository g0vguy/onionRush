use anyhow::{Context, Result};
use rand::Rng;
use reqwest::{Client, Proxy};
use std::time::Duration;

pub fn random_identity() -> String {
    let mut rng = rand::thread_rng();
    let n: u64 = rng.r#gen();
    format!("rush{n:x}")
}

pub fn build_client(socks_addr: &str, identity: &str, timeout: Duration, reuse_connections: bool) -> Result<Client> {
    let url = format!("socks5h://{identity}:{identity}@{socks_addr}");
    let proxy = Proxy::all(&url).context("bad socks proxy url")?;

    let mut builder = Client::builder()
        .proxy(proxy)
        .timeout(timeout)
        .tcp_keepalive(Duration::from_secs(30))
        .user_agent(random_user_agent());

    if !reuse_connections {
        builder = builder
            .pool_max_idle_per_host(0)
            .pool_idle_timeout(Duration::from_secs(0));
    }

    builder.build().context("failed building http client")
}

pub fn random_user_agent() -> String {
    let agents = vec![
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/121.0",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36 Edg/119.0.0.0",
    ];
    let mut rng = rand::thread_rng();
    agents[rng.gen_range(0..agents.len())].to_string()
}