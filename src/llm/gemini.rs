use std::{fs, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    error::BridgeError,
    llm::{LlmFuture, LlmMediaRequest, LlmOutput, LlmProvider},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiConfig {
    pub endpoint_base: String,
    pub api_key: String,
    pub model: String,
    pub max_inline_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct GeminiClient {
    client: Client,
    config: GeminiConfig,
}

impl GeminiClient {
    pub fn new(config: GeminiConfig, timeout: Duration) -> Result<Self, BridgeError> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|err| BridgeError::Config(err.to_string()))?;
        Ok(Self { client, config })
    }

    fn generate_url(&self) -> String {
        format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.config.endpoint_base.trim_end_matches('/'),
            self.config.model,
            self.config.api_key
        )
    }
}

impl LlmProvider for GeminiClient {
    fn process_media<'a>(&'a self, request: LlmMediaRequest<'a>) -> LlmFuture<'a, LlmOutput> {
        Box::pin(async move {
            let metadata = fs::metadata(request.path)?;
            if metadata.len() > self.config.max_inline_bytes {
                return Err(BridgeError::ExternalPayloadInvalid(format!(
                    "media exceeds Gemini inline limit {}",
                    self.config.max_inline_bytes
                )));
            }

            let bytes = fs::read(request.path)?;
            let payload = GeminiGenerateRequest {
                contents: vec![GeminiContent {
                    role: "user",
                    parts: vec![
                        GeminiPart::Text {
                            text: request.system_prompt.to_string(),
                        },
                        GeminiPart::InlineData {
                            inline_data: GeminiInlineData {
                                mime_type: request.mime_type.to_string(),
                                data: STANDARD.encode(bytes),
                            },
                        },
                    ],
                }],
            };

            let response = self
                .client
                .post(self.generate_url())
                .json(&payload)
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
                    "Gemini returned {status}: {body}"
                )));
            }

            let parsed: GeminiGenerateResponse = serde_json::from_str(&body)
                .map_err(|err| BridgeError::ExternalPayloadInvalid(err.to_string()))?;
            let text = parsed
                .candidates
                .into_iter()
                .flat_map(|candidate| candidate.content.parts)
                .filter_map(|part| part.text)
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string();
            if text.is_empty() {
                return Err(BridgeError::ExternalPayloadInvalid(
                    "Gemini response contained no text".to_string(),
                ));
            }

            Ok(LlmOutput {
                provider: "gemini".to_string(),
                model: self.config.model.clone(),
                text,
            })
        })
    }
}

#[derive(Debug, Serialize)]
struct GeminiGenerateRequest {
    contents: Vec<GeminiContent>,
}

#[derive(Debug, Serialize)]
struct GeminiContent {
    role: &'static str,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum GeminiPart {
    Text { text: String },
    InlineData { inline_data: GeminiInlineData },
}

#[derive(Debug, Serialize)]
struct GeminiInlineData {
    mime_type: String,
    data: String,
}

#[derive(Debug, Deserialize)]
struct GeminiGenerateResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiResponseContent,
}

#[derive(Debug, Deserialize)]
struct GeminiResponseContent {
    parts: Vec<GeminiResponsePart>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponsePart {
    text: Option<String>,
}

#[cfg(test)]
mod tests {
    use axum::{Json, Router, response::IntoResponse, routing::post};
    use serde_json::{Value, json};
    use tokio::net::TcpListener;

    use crate::llm::{LlmMediaRequest, MediaKind};

    use super::*;

    async fn spawn_mock_gemini() -> String {
        async fn handler(Json(payload): Json<Value>) -> impl IntoResponse {
            assert_eq!(payload["contents"][0]["parts"][0]["text"], "describe this");
            assert_eq!(
                payload["contents"][0]["parts"][1]["inline_data"]["mime_type"],
                "image/jpeg"
            );
            Json(json!({
                "candidates": [
                    {
                        "content": {
                            "parts": [
                                { "text": "A concise Gemini description." }
                            ]
                        }
                    }
                ]
            }))
        }

        let app = Router::new().route("/{*path}", post(handler));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn gemini_client_processes_inline_media_with_mock_server() {
        let endpoint_base = spawn_mock_gemini().await;
        let temp = tempfile::tempdir().unwrap();
        let media_path = temp.path().join("image.jpg");
        fs::write(&media_path, b"fake image").unwrap();
        let client = GeminiClient::new(
            GeminiConfig {
                endpoint_base,
                api_key: "test-key".to_string(),
                model: "gemini-test".to_string(),
                max_inline_bytes: 1024,
            },
            Duration::from_secs(5),
        )
        .unwrap();

        let output = client
            .process_media(LlmMediaRequest {
                kind: MediaKind::Image,
                path: &media_path,
                mime_type: "image/jpeg",
                system_prompt: "describe this",
            })
            .await
            .unwrap();

        assert_eq!(output.provider, "gemini");
        assert_eq!(output.model, "gemini-test");
        assert_eq!(output.text, "A concise Gemini description.");
    }

    #[tokio::test]
    async fn gemini_client_rejects_large_inline_media() {
        let temp = tempfile::tempdir().unwrap();
        let media_path = temp.path().join("image.jpg");
        fs::write(&media_path, b"fake image").unwrap();
        let client = GeminiClient::new(
            GeminiConfig {
                endpoint_base: "http://127.0.0.1:1".to_string(),
                api_key: "test-key".to_string(),
                model: "gemini-test".to_string(),
                max_inline_bytes: 4,
            },
            Duration::from_secs(5),
        )
        .unwrap();

        let err = client
            .process_media(LlmMediaRequest {
                kind: MediaKind::Image,
                path: &media_path,
                mime_type: "image/jpeg",
                system_prompt: "describe this",
            })
            .await
            .unwrap_err();

        assert!(matches!(err, BridgeError::ExternalPayloadInvalid(_)));
    }
}
