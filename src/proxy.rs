use anyhow::{Context, Result};
use rand::Rng;
use reqwest::{Client, Proxy};
use std::time::Duration;

pub fn random_identity() -> String {
    let mut rng = rand::thread_rng();
    let n: u64 = rng.r#gen();
    format!("rush{n:x}")
}

pub fn build_client(socks_addr: &str, identity: &str, timeout: Duration) -> Result<Client> {
    let url = format!("socks5h://{identity}:{identity}@{socks_addr}");
    let proxy = Proxy::all(&url).context("bad socks proxy url")?;

    Client::builder()
        .proxy(proxy)
        .timeout(timeout)
        .pool_max_idle_per_host(0)
        .pool_idle_timeout(Duration::from_secs(0))
        .tcp_keepalive(Duration::from_secs(30))
        .build()
        .context("failed building http client")
}