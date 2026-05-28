use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::Client;
use serde::Deserialize;

use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::{error::BridgeError, wechat::MediaId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WechatApiConfig {
    pub api_base: String,
    pub app_id: String,
    pub app_secret: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessToken {
    pub token: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedMedia {
    pub path: PathBuf,
    pub content_type: Option<String>,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub struct WechatMediaClient {
    client: Client,
    config: WechatApiConfig,
    max_media_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct WechatAccessTokenCache {
    client: WechatMediaClient,
    cached: Arc<Mutex<Option<CachedAccessToken>>>,
    refresh_skew: Duration,
}

#[derive(Debug, Clone)]
struct CachedAccessToken {
    token: String,
    expires_at: Instant,
}

impl WechatAccessTokenCache {
    pub fn new(client: WechatMediaClient, refresh_skew: Duration) -> Self {
        Self {
            client,
            cached: Arc::new(Mutex::new(None)),
            refresh_skew,
        }
    }

    pub async fn get_token(&self) -> Result<String, BridgeError> {
        if let Some(token) = self.valid_cached_token().await {
            return Ok(token);
        }

        let fetched = self.client.fetch_access_token().await?;
        let ttl = Duration::from_secs(fetched.expires_in).saturating_sub(self.refresh_skew);
        let cached = CachedAccessToken {
            token: fetched.token,
            expires_at: Instant::now() + ttl,
        };
        let token = cached.token.clone();
        *self.cached.lock().await = Some(cached);
        Ok(token)
    }

    async fn valid_cached_token(&self) -> Option<String> {
        let cached = self.cached.lock().await;
        cached
            .as_ref()
            .filter(|token| Instant::now() < token.expires_at)
            .map(|token| token.token.clone())
    }
}

impl WechatMediaClient {
    pub fn new(
        config: WechatApiConfig,
        timeout: Duration,
        max_media_bytes: u64,
    ) -> Result<Self, BridgeError> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|err| BridgeError::Config(err.to_string()))?;
        Ok(Self {
            client,
            config,
            max_media_bytes,
        })
    }

    pub async fn fetch_access_token(&self) -> Result<AccessToken, BridgeError> {
        let url = format!(
            "{}/cgi-bin/token",
            self.config.api_base.trim_end_matches('/')
        );
        let response = self
            .client
            .get(url)
            .query(&[
                ("grant_type", "client_credential"),
                ("appid", self.config.app_id.as_str()),
                ("secret", self.config.app_secret.as_str()),
            ])
            .send()
            .await
            .map_err(|err| BridgeError::ExternalRequest(err.to_string()))?;
        if !response.status().is_success() {
            return Err(BridgeError::ExternalRequest(format!(
                "WeChat token API returned {}",
                response.status()
            )));
        }
        let payload: TokenResponse = response
            .json()
            .await
            .map_err(|err| BridgeError::ExternalPayloadInvalid(err.to_string()))?;
        Ok(AccessToken {
            token: payload.access_token,
            expires_in: payload.expires_in,
        })
    }

    pub async fn download_media(
        &self,
        access_token: &str,
        media_id: &MediaId,
        target_path: &Path,
    ) -> Result<DownloadedMedia, BridgeError> {
        let url = format!(
            "{}/cgi-bin/media/get",
            self.config.api_base.trim_end_matches('/')
        );
        let mut response = self
            .client
            .get(url)
            .query(&[
                ("access_token", access_token),
                ("media_id", media_id.as_str()),
            ])
            .send()
            .await
            .map_err(|err| BridgeError::ExternalRequest(err.to_string()))?;
        if !response.status().is_success() {
            return Err(BridgeError::ExternalRequest(format!(
                "WeChat media API returned {}",
                response.status()
            )));
        }

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp_path = target_path.with_extension("download.tmp");
        let mut file = fs::File::create(&tmp_path)?;
        let mut hasher = Sha256::new();
        let mut size_bytes = 0_u64;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|err| BridgeError::ExternalRequest(err.to_string()))?
        {
            size_bytes += chunk.len() as u64;
            if size_bytes > self.max_media_bytes {
                return Err(BridgeError::ExternalPayloadInvalid(format!(
                    "media exceeds max size {}",
                    self.max_media_bytes
                )));
            }
            file.write_all(&chunk)?;
            hasher.update(&chunk);
        }
        file.sync_all()?;
        fs::rename(&tmp_path, target_path)?;

        Ok(DownloadedMedia {
            path: target_path.to_path_buf(),
            content_type: response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
            size_bytes,
            sha256: format!("{:x}", hasher.finalize()),
        })
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{
        Router,
        extract::Request,
        http::{HeaderValue, header::CONTENT_TYPE},
        response::{IntoResponse, Response},
        routing::get,
    };
    use tokio::net::TcpListener;

    use super::*;

    async fn spawn_mock_wechat() -> String {
        async fn handler(request: Request) -> Response {
            let path = request.uri().path();
            if path == "/cgi-bin/token" {
                r#"{"access_token":"access-token-1","expires_in":7200}"#.into_response()
            } else {
                let mut response = b"fake-media-bytes".as_slice().into_response();
                response
                    .headers_mut()
                    .insert(CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
                response
            }
        }

        let app = Router::new().route("/{*path}", get(handler));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    async fn spawn_counted_token_wechat() -> (String, Arc<AtomicUsize>) {
        let token_calls = Arc::new(AtomicUsize::new(0));
        let token_calls_for_handler = token_calls.clone();
        async fn handler(request: Request, token_calls: Arc<AtomicUsize>) -> Response {
            let path = request.uri().path();
            if path == "/cgi-bin/token" {
                let count = token_calls.fetch_add(1, Ordering::SeqCst) + 1;
                format!(r#"{{"access_token":"access-token-{count}","expires_in":7200}}"#)
                    .into_response()
            } else {
                b"fake-media-bytes".as_slice().into_response()
            }
        }

        let app = Router::new().route(
            "/{*path}",
            get(move |request| handler(request, token_calls_for_handler.clone())),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{addr}"), token_calls)
    }

    #[tokio::test]
    async fn fetches_access_token_from_mock_wechat() {
        let api_base = spawn_mock_wechat().await;
        let client = WechatMediaClient::new(
            WechatApiConfig {
                api_base,
                app_id: "app-id".to_string(),
                app_secret: "secret".to_string(),
            },
            Duration::from_secs(5),
            1024,
        )
        .unwrap();

        let token = client.fetch_access_token().await.unwrap();

        assert_eq!(token.token, "access-token-1");
        assert_eq!(token.expires_in, 7200);
    }

    #[tokio::test]
    async fn access_token_cache_reuses_valid_token() {
        let (api_base, token_calls) = spawn_counted_token_wechat().await;
        let client = WechatMediaClient::new(
            WechatApiConfig {
                api_base,
                app_id: "app-id".to_string(),
                app_secret: "secret".to_string(),
            },
            Duration::from_secs(5),
            1024,
        )
        .unwrap();
        let cache = WechatAccessTokenCache::new(client, Duration::from_secs(300));

        let first = cache.get_token().await.unwrap();
        let second = cache.get_token().await.unwrap();

        assert_eq!(first, "access-token-1");
        assert_eq!(second, "access-token-1");
        assert_eq!(token_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn downloads_media_stream_to_file() {
        let api_base = spawn_mock_wechat().await;
        let client = WechatMediaClient::new(
            WechatApiConfig {
                api_base,
                app_id: "app-id".to_string(),
                app_secret: "secret".to_string(),
            },
            Duration::from_secs(5),
            1024,
        )
        .unwrap();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("media.original");

        let media = client
            .download_media("access-token-1", &MediaId::new("media-id-1"), &path)
            .await
            .unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"fake-media-bytes");
        assert_eq!(media.content_type.as_deref(), Some("image/jpeg"));
        assert_eq!(media.size_bytes, 16);
        assert_eq!(media.path, path);
    }

    #[tokio::test]
    async fn rejects_media_over_size_limit() {
        let api_base = spawn_mock_wechat().await;
        let client = WechatMediaClient::new(
            WechatApiConfig {
                api_base,
                app_id: "app-id".to_string(),
                app_secret: "secret".to_string(),
            },
            Duration::from_secs(5),
            4,
        )
        .unwrap();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("media.original");

        let err = client
            .download_media("access-token-1", &MediaId::new("media-id-1"), &path)
            .await
            .unwrap_err();

        assert!(matches!(err, BridgeError::ExternalPayloadInvalid(_)));
    }
}
