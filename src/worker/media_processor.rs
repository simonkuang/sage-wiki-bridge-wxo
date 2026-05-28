use std::{path::PathBuf, sync::Arc};

use crate::{
    error::BridgeError,
    llm::{LlmMediaRequest, LlmProvider, MediaKind},
    media::{WechatAccessTokenCache, WechatMediaClient},
    preprocess::{
        artifact::{ProcessedArtifact, ProcessedArtifactKind},
        media::{ModelPreprocessOutput, process_model_output},
    },
    store::StoredMessage,
    wechat::MediaId,
    worker::{MediaJobProcessor, WorkerFuture, message_key},
};

#[derive(Clone)]
pub struct GeminiMediaJobProcessor {
    media_client: WechatMediaClient,
    token_cache: WechatAccessTokenCache,
    llm_provider: Arc<dyn LlmProvider>,
    raw_root: PathBuf,
    image_prompt: String,
    voice_prompt: String,
    video_prompt: String,
}

impl GeminiMediaJobProcessor {
    pub fn new(
        media_client: WechatMediaClient,
        llm_provider: Arc<dyn LlmProvider>,
        raw_root: impl Into<PathBuf>,
        token_refresh_skew: std::time::Duration,
    ) -> Self {
        let token_cache = WechatAccessTokenCache::new(media_client.clone(), token_refresh_skew);
        Self {
            media_client,
            token_cache,
            llm_provider,
            raw_root: raw_root.into(),
            image_prompt: "Describe this image for a personal knowledge base.".to_string(),
            voice_prompt: "Transcribe and summarize this voice message.".to_string(),
            video_prompt: "Summarize this video for a personal knowledge base.".to_string(),
        }
    }

    pub fn with_prompts(
        mut self,
        image_prompt: impl Into<String>,
        voice_prompt: impl Into<String>,
        video_prompt: impl Into<String>,
    ) -> Self {
        self.image_prompt = image_prompt.into();
        self.voice_prompt = voice_prompt.into();
        self.video_prompt = video_prompt.into();
        self
    }
}

impl MediaJobProcessor for GeminiMediaJobProcessor {
    fn process_media_message<'a>(
        &'a self,
        message: &'a StoredMessage,
    ) -> WorkerFuture<'a, ProcessedArtifact> {
        Box::pin(async move {
            let media_id = message.media_id.as_ref().ok_or_else(|| {
                BridgeError::ExternalPayloadInvalid("media_id missing".to_string())
            })?;
            let kind = media_kind(&message.message_type)?;
            let artifact_kind = artifact_kind(&message.message_type)?;
            let mime_type = mime_type(&message.message_type, message.voice_format.as_deref());
            let prompt = prompt(&message.message_type, self);
            let target_path = self
                .raw_root
                .join(message_key(message))
                .join(format!("media.original{}", extension_for_mime(mime_type)));
            let access_token = self.token_cache.get_token().await?;
            let downloaded = self
                .media_client
                .download_media(&access_token, &MediaId::new(media_id), &target_path)
                .await?;
            let output = self
                .llm_provider
                .process_media(LlmMediaRequest {
                    kind,
                    path: &downloaded.path,
                    mime_type,
                    system_prompt: prompt,
                })
                .await?;

            Ok(process_model_output(ModelPreprocessOutput {
                message_key: message_key(message),
                kind: artifact_kind,
                markdown_body: output.text,
                provider: output.provider,
                model: output.model,
                raw_payload_paths: vec![downloaded.path],
                processed_payload_paths: Vec::new(),
            }))
        })
    }
}

fn media_kind(message_type: &str) -> Result<MediaKind, BridgeError> {
    match message_type {
        "image" => Ok(MediaKind::Image),
        "voice" => Ok(MediaKind::Voice),
        "video" => Ok(MediaKind::Video),
        "shortvideo" => Ok(MediaKind::ShortVideo),
        other => Err(BridgeError::MessageUnsupported(other.to_string())),
    }
}

fn artifact_kind(message_type: &str) -> Result<ProcessedArtifactKind, BridgeError> {
    match message_type {
        "image" => Ok(ProcessedArtifactKind::Image),
        "voice" => Ok(ProcessedArtifactKind::Voice),
        "video" => Ok(ProcessedArtifactKind::Video),
        "shortvideo" => Ok(ProcessedArtifactKind::ShortVideo),
        other => Err(BridgeError::MessageUnsupported(other.to_string())),
    }
}

fn mime_type(message_type: &str, voice_format: Option<&str>) -> &'static str {
    match message_type {
        "image" => "image/jpeg",
        "voice" => match voice_format {
            Some("mp3") => "audio/mpeg",
            Some("wav") => "audio/wav",
            _ => "audio/amr",
        },
        "video" | "shortvideo" => "video/mp4",
        _ => "application/octet-stream",
    }
}

fn prompt<'a>(message_type: &str, processor: &'a GeminiMediaJobProcessor) -> &'a str {
    match message_type {
        "image" => &processor.image_prompt,
        "voice" => &processor.voice_prompt,
        "video" | "shortvideo" => &processor.video_prompt,
        _ => "",
    }
}

fn extension_for_mime(mime: &str) -> &'static str {
    match mime {
        "image/jpeg" => ".jpg",
        "audio/mpeg" => ".mp3",
        "audio/wav" => ".wav",
        "audio/amr" => ".amr",
        "video/mp4" => ".mp4",
        _ => ".bin",
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, time::Duration};

    use axum::{Router, extract::Request, response::IntoResponse, routing::get};
    use tokio::net::TcpListener;

    use crate::{
        llm::{LlmFuture, LlmOutput},
        media::WechatApiConfig,
    };

    use super::*;

    #[derive(Debug)]
    struct FakeLlmProvider;

    impl LlmProvider for FakeLlmProvider {
        fn process_media<'a>(
            &'a self,
            request: crate::llm::LlmMediaRequest<'a>,
        ) -> LlmFuture<'a, LlmOutput> {
            Box::pin(async move {
                assert_eq!(request.mime_type, "image/jpeg");
                assert_eq!(request.system_prompt, "describe image for test");
                Ok(LlmOutput {
                    provider: "gemini".to_string(),
                    model: "gemini-test".to_string(),
                    text: "fake image summary".to_string(),
                })
            })
        }
    }

    async fn spawn_mock_wechat_media() -> String {
        async fn handler(request: Request) -> impl IntoResponse {
            if request.uri().path() == "/cgi-bin/token" {
                r#"{"access_token":"access-token-1","expires_in":7200}"#.into_response()
            } else {
                b"fake-image".as_slice().into_response()
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

    fn image_message() -> StoredMessage {
        StoredMessage {
            id: 1,
            wechat_msg_id: Some("msg_image_1".to_string()),
            from_openid_hash: "sha256:abc".to_string(),
            create_time: Some(1780000001),
            received_at: "2026-05-27T21:30:15+08:00".to_string(),
            message_type: "image".to_string(),
            content_text: None,
            media_id: Some("media-image-1".to_string()),
            thumb_media_id: None,
            pic_url: Some("https://mmbiz.qpic.cn/example.jpg".to_string()),
            voice_format: None,
            voice_recognition: None,
            location_lat: None,
            location_lng: None,
            location_scale: None,
            location_label: None,
            link_title: None,
            link_description: None,
            link_url: None,
            raw_dir: "data/raw/msg_image_1".to_string(),
        }
    }

    #[tokio::test]
    async fn gemini_media_processor_downloads_and_processes_image() {
        let api_base = spawn_mock_wechat_media().await;
        let raw_root = tempfile::tempdir().unwrap();
        let processor = GeminiMediaJobProcessor::new(
            WechatMediaClient::new(
                WechatApiConfig {
                    api_base,
                    app_id: "app-id".to_string(),
                    app_secret: "secret".to_string(),
                },
                Duration::from_secs(5),
                1024,
            )
            .unwrap(),
            Arc::new(FakeLlmProvider),
            raw_root.path(),
            Duration::from_secs(300),
        )
        .with_prompts(
            "describe image for test",
            "transcribe voice for test",
            "summarize video for test",
        );

        let artifact = processor
            .process_media_message(&image_message())
            .await
            .unwrap();

        assert_eq!(artifact.kind, ProcessedArtifactKind::Image);
        assert_eq!(artifact.provider.as_deref(), Some("gemini"));
        assert_eq!(artifact.markdown_body, "fake image summary");
        assert_eq!(artifact.raw_payload_paths.len(), 1);
        assert_eq!(
            fs::read(&artifact.raw_payload_paths[0]).unwrap(),
            b"fake-image"
        );
    }

    #[test]
    fn mime_type_maps_voice_format() {
        assert_eq!(mime_type("voice", Some("mp3")), "audio/mpeg");
        assert_eq!(mime_type("voice", Some("amr")), "audio/amr");
    }
}
