use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde_json::Value;
use tokio::time::sleep;

use crate::api_params::ApiParams;

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
}

impl ApiClient {
    pub fn new(base_url: impl Into<String>, username: String, password: String) -> Result<Self> {
        let mut headers = HeaderMap::new();
        let auth = format!("Basic {}", base64_basic_auth(&username, &password));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth).context("invalid auth header")?,
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .pool_max_idle_per_host(128)
            .tcp_keepalive(Duration::from_secs(30))
            .build()
            .context("failed to build reqwest client")?;

        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            client,
        })
    }

    pub fn build_url(&self, params: &ApiParams) -> String {
        let path = params
            .path_segments
            .iter()
            .filter(|s| !s.is_empty())
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("/");

        let mut url = format!("{}/{}", self.base_url, path);
        if !params.query_params.is_empty() {
            let query = params
                .query_params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            url = format!("{}?{}", url, query);
        }
        url
    }

    pub async fn get_json(&self, params: &ApiParams) -> Result<Value> {
        let url = self.build_url(params);
        let mut last_err = None;

        for attempt in 0..3 {
            let response = self.client.get(&url).send().await;
            match response {
                Ok(resp) if resp.status().is_success() => {
                    return resp.json().await.context("failed to parse JSON response");
                }
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    last_err = Some(anyhow!("GET {} failed: {} {}", url, status, body));
                }
                Err(err) => {
                    last_err = Some(anyhow!("GET {} request error: {}", url, err));
                }
            }

            if attempt < 2 {
                sleep(Duration::from_millis(200 * (attempt + 1) as u64)).await;
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("unknown request error")))
    }
}

fn base64_basic_auth(user: &str, pass: &str) -> String {
    use base64::{Engine as _, engine::general_purpose};
    general_purpose::STANDARD.encode(format!("{}:{}", user, pass))
}
