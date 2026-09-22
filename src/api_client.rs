use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue, RETRY_AFTER};
use reqwest::{StatusCode, Url};
use serde::de::DeserializeOwned;
use tokio::time::sleep;

use crate::api_params::ApiParams;

const MAX_ATTEMPTS: u32 = 3;
const BASE_BACKOFF: Duration = Duration::from_millis(250);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct ApiClient {
    base_url: Url,
    client: reqwest::Client,
}

impl ApiClient {
    pub fn new(base_url: &str, username: String, password: String) -> Result<Self> {
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
            .timeout(Duration::from_secs(60))
            .build()
            .context("failed to build reqwest client")?;

        Ok(Self {
            base_url: Url::parse(base_url).context("invalid API base URL")?,
            client,
        })
    }

    fn url(&self, params: &ApiParams) -> Result<Url> {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .map_err(|_| anyhow!("API base URL cannot have path segments appended"))?
            .pop_if_empty()
            .extend(params.path_segments.iter().filter(|s| !s.is_empty()));
        Ok(url)
    }

    pub async fn get<T: DeserializeOwned>(&self, params: &ApiParams) -> Result<T> {
        let url = self.url(params)?;
        let mut last_err = None;

        for attempt in 0..MAX_ATTEMPTS {
            let response = self
                .client
                .get(url.clone())
                .query(&params.query_params)
                .send()
                .await;

            let wait = match response {
                Ok(resp) if resp.status().is_success() => {
                    return resp
                        .json()
                        .await
                        .with_context(|| format!("failed to parse response from {url}"));
                }
                Ok(resp) => {
                    let status = resp.status();
                    let retry_after = retry_after(&resp);
                    let body = resp.text().await.unwrap_or_default();
                    last_err = Some(anyhow!("GET {url} failed: {status} {body}"));

                    if !is_retryable(status) {
                        break;
                    }
                    retry_after.unwrap_or_else(|| backoff(attempt))
                }
                Err(err) => {
                    last_err = Some(anyhow!("GET {url} request error: {err}"));
                    backoff(attempt)
                }
            };

            if attempt + 1 < MAX_ATTEMPTS {
                if wait > MAX_BACKOFF {
                    break;
                }
                sleep(wait).await;
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("unknown request error")))
    }
}

fn is_retryable(status: StatusCode) -> bool {
    status.is_server_error()
        || status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::REQUEST_TIMEOUT
}

fn backoff(attempt: u32) -> Duration {
    BASE_BACKOFF * 2u32.pow(attempt)
}

fn retry_after(resp: &reqwest::Response) -> Option<Duration> {
    resp.headers()
        .get(RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

fn base64_basic_auth(user: &str, pass: &str) -> String {
    use base64::{Engine as _, engine::general_purpose};
    general_purpose::STANDARD.encode(format!("{user}:{pass}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> ApiClient {
        ApiClient::new(
            "https://ftc-api.firstinspires.org/v2.0",
            "user".into(),
            "pass".into(),
        )
        .unwrap()
    }

    #[test]
    fn segments_are_appended_to_the_base_path() {
        let params = ApiParams::new(vec!["2025", "matches", "USTXCMP"]);
        assert_eq!(
            client().url(&params).unwrap().as_str(),
            "https://ftc-api.firstinspires.org/v2.0/2025/matches/USTXCMP"
        );
    }

    #[test]
    fn a_trailing_slash_does_not_double_up() {
        let base = ApiClient::new(
            "https://ftc-api.firstinspires.org/v2.0/",
            "u".into(),
            "p".into(),
        )
        .unwrap();
        let params = ApiParams::new(vec!["2025", "teams"]);
        assert_eq!(
            base.url(&params).unwrap().as_str(),
            "https://ftc-api.firstinspires.org/v2.0/2025/teams"
        );
    }

    #[test]
    fn segments_are_percent_encoded() {
        let params = ApiParams::new(vec!["2025", "events", "a b/c"]);
        let url = client().url(&params).unwrap();
        assert!(url.as_str().ends_with("/2025/events/a%20b%2Fc"), "{url}");
    }

    #[test]
    fn only_the_rejectable_statuses_are_retried() {
        assert!(is_retryable(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_retryable(StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_retryable(StatusCode::TOO_MANY_REQUESTS));
        assert!(!is_retryable(StatusCode::UNAUTHORIZED));
        assert!(!is_retryable(StatusCode::NOT_FOUND));
        assert!(!is_retryable(StatusCode::BAD_REQUEST));
    }

    #[test]
    fn backoff_doubles_each_attempt() {
        assert_eq!(backoff(0), Duration::from_millis(250));
        assert_eq!(backoff(1), Duration::from_millis(500));
        assert_eq!(backoff(2), Duration::from_millis(1000));
    }
}
