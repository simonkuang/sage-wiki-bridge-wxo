use std::time::Duration;

use reqwest::Client;

use crate::{
    enrich::{
        jina_reader::{JinaReaderOptions, validate_public_http_url},
        tencent_lbs::{LocationCoordinate, TencentLbsOptions},
    },
    error::BridgeError,
    worker::{ExternalClients, WorkerFuture},
};

#[derive(Debug, Clone)]
pub struct HttpExternalClients {
    client: Client,
    tencent_lbs: TencentLbsOptions,
    jina_reader: JinaReaderOptions,
    jina_api_key: Option<String>,
}

impl HttpExternalClients {
    pub fn new(
        tencent_lbs: TencentLbsOptions,
        jina_reader: JinaReaderOptions,
        jina_api_key: Option<String>,
        timeout: Duration,
    ) -> Result<Self, BridgeError> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|err| BridgeError::Config(err.to_string()))?;
        Ok(Self {
            client,
            tencent_lbs,
            jina_reader,
            jina_api_key,
        })
    }
}

impl ExternalClients for HttpExternalClients {
    fn reverse_geocode(&self, latitude: f64, longitude: f64) -> WorkerFuture<String> {
        let client = self.client.clone();
        let options = self.tencent_lbs.clone();
        Box::pin(async move {
            let url = options.build_reverse_geocode_url(LocationCoordinate {
                latitude,
                longitude,
            })?;
            let response = client
                .get(url)
                .send()
                .await
                .map_err(|err| BridgeError::ExternalRequest(err.to_string()))?;
            let status = response.status();
            let body = response
                .text()
                .await
                .map_err(|err| BridgeError::ExternalRequest(err.to_string()))?;
            if !status.is_success() {
                return Err(BridgeError::ExternalRequest(format!(
                    "Tencent LBS returned {status}"
                )));
            }
            Ok(body)
        })
    }

    fn read_link(&self, url: &str) -> WorkerFuture<String> {
        let client = self.client.clone();
        let options = self.jina_reader.clone();
        let api_key = self.jina_api_key.clone();
        let url = url.to_string();
        Box::pin(async move {
            validate_public_http_url(&url)?;
            let reader_url = options.build_reader_url(&url)?;
            let mut request = client.get(reader_url);
            if let Some(api_key) = api_key {
                request = request.bearer_auth(api_key);
            }
            let response = request
                .send()
                .await
                .map_err(|err| BridgeError::ExternalRequest(err.to_string()))?;
            let status = response.status();
            let body = response
                .text()
                .await
                .map_err(|err| BridgeError::ExternalRequest(err.to_string()))?;
            if !status.is_success() {
                return Err(BridgeError::ExternalRequest(format!(
                    "Jina Reader returned {status}"
                )));
            }
            Ok(body)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, sync::Arc};

    use axum::{Router, extract::Request, response::IntoResponse, routing::get};
    use tokio::net::TcpListener;

    use super::*;

    async fn spawn_mock_server() -> SocketAddr {
        async fn handler(request: Request) -> impl IntoResponse {
            let path = request.uri().path().to_string();
            if path.contains("/ws/geocoder/v1") {
                include_str!("../../tests/fixtures/external/tencent_lbs_success.json")
                    .into_response()
            } else {
                include_str!("../../tests/fixtures/external/jina_reader_success.md").into_response()
            }
        }

        let app = Router::new().route("/{*path}", get(handler));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        addr
    }

    #[tokio::test]
    async fn http_clients_call_mock_lbs_and_jina() {
        let addr = spawn_mock_server().await;
        let base = format!("http://{addr}");
        let client = Arc::new(
            HttpExternalClients::new(
                TencentLbsOptions {
                    endpoint: format!("{base}/ws/geocoder/v1/"),
                    key: "test-key".to_string(),
                    get_poi: true,
                    radius_meters: Some(500),
                },
                JinaReaderOptions {
                    endpoint: base.clone(),
                },
                Some("jina-key".to_string()),
                Duration::from_secs(5),
            )
            .unwrap(),
        );

        let lbs = client.reverse_geocode(23.134521, 113.358803).await.unwrap();
        assert!(lbs.contains("\"adcode\": \"440106\""));

        let reader = client
            .read_link("https://example.com/article")
            .await
            .unwrap();
        assert!(reader.contains("Jina Reader"));
    }

    #[tokio::test]
    async fn http_client_rejects_private_link_before_request() {
        let addr = spawn_mock_server().await;
        let base = format!("http://{addr}");
        let client = HttpExternalClients::new(
            TencentLbsOptions {
                endpoint: format!("{base}/ws/geocoder/v1/"),
                key: "test-key".to_string(),
                get_poi: true,
                radius_meters: None,
            },
            JinaReaderOptions { endpoint: base },
            None,
            Duration::from_secs(5),
        )
        .unwrap();

        let err = client
            .read_link("http://127.0.0.1/secret")
            .await
            .unwrap_err();
        assert!(matches!(err, BridgeError::UrlNotAllowed(_)));
    }
}
