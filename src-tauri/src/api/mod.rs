use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use std::time::Duration;

pub mod assets;
pub mod auth;
pub mod discovery;
pub mod types;

#[derive(Debug, Clone, PartialEq)]
pub enum UrlMode {
    /// Connected via the local network URL
    Local,
    /// Connected via the remote/internet URL (local was unreachable)
    Remote,
    /// Only one URL was configured
    Direct,
}

impl UrlMode {
    pub fn as_str(&self) -> &str {
        match self {
            UrlMode::Local => "Local",
            UrlMode::Remote => "Remote",
            UrlMode::Direct => "Direct",
        }
    }
}

pub struct ImmichClient {
    pub client: reqwest::Client,
    pub base_url: String,
    pub url_mode: UrlMode,
}

impl ImmichClient {
    /// Create a client with a single URL (used for pre-login testing)
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Result<Self> {
        let base_url = normalize_url(base_url.into());
        let client = build_client(&api_key.into(), Duration::from_secs(30))?;
        Ok(Self { client, base_url, url_mode: UrlMode::Direct })
    }

    /// Create a client for a profile, automatically picking local vs remote.
    /// Tries local_url first with a 2-second timeout; falls back to remote_url.
    pub async fn for_profile(
        local_url: Option<String>,
        remote_url: String,
        api_key: String,
    ) -> Result<Self> {
        let remote_url = normalize_url(remote_url);
        let has_local = local_url.as_ref().map(|u| !u.is_empty()).unwrap_or(false);

        if let Some(local) = local_url.as_ref().filter(|u| !u.is_empty()) {
            let local = normalize_url(local.clone());
            let probe = build_client(&api_key, Duration::from_secs(2))?;
            let ping = format!("{}/api/server/ping", local);

            if tokio::time::timeout(Duration::from_secs(2), probe.get(&ping).send())
                .await
                .ok()
                .and_then(|r| r.ok())
                .map(|r| r.status().is_success())
                .unwrap_or(false)
            {
                let client = build_client(&api_key, Duration::from_secs(30))?;
                return Ok(Self { client, base_url: local, url_mode: UrlMode::Local });
            }
        }

        let client = build_client(&api_key, Duration::from_secs(30))?;
        Ok(Self {
            client,
            base_url: remote_url,
            url_mode: if has_local { UrlMode::Remote } else { UrlMode::Direct },
        })
    }
}

fn normalize_url(mut url: String) -> String {
    while url.ends_with('/') {
        url.pop();
    }
    url
}

fn build_client(api_key: &str, timeout: Duration) -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-api-key",
        HeaderValue::from_str(api_key).map_err(|_| anyhow!("Invalid API key format"))?,
    );
    Ok(reqwest::Client::builder()
        .default_headers(headers)
        .timeout(timeout)
        .build()?)
}
