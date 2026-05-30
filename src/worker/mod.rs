use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

pub mod media_processor;

use crate::{
    archive::ProcessedArtifactStore,
    enrich::tencent_lbs::extract_location_summary,
    error::BridgeError,
    preprocess::{
        artifact::{ProcessedArtifact, ProcessedArtifactKind},
        link::process_link,
        location::process_location,
    },
    source::{SourceMetadata, SourceWriter},
    store::{Store, StoredMessage},
    wechat::message::{CommonFields, LinkMessage, LocationMessage},
    wechat::{OpenId, UrlString, WechatMsgId},
};
use time::{Duration as TimeDuration, OffsetDateTime, format_description::well_known::Rfc3339};
use url::Url;

pub type WorkerFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, BridgeError>> + Send + 'a>>;

pub trait ExternalClients: Send + Sync {
    fn reverse_geocode(&self, latitude: f64, longitude: f64) -> WorkerFuture<'static, String>;
    fn read_link(&self, url: &str) -> WorkerFuture<'static, String>;
}

pub trait MediaJobProcessor: Send + Sync {
    fn process_media_message<'a>(
        &'a self,
        message: &'a StoredMessage,
    ) -> WorkerFuture<'a, ProcessedArtifact>;
}

#[derive(Debug, Default)]
pub struct NoopExternalClients;

impl ExternalClients for NoopExternalClients {
    fn reverse_geocode(&self, _latitude: f64, _longitude: f64) -> WorkerFuture<'static, String> {
        Box::pin(async {
            Err(BridgeError::ExternalPayloadInvalid(
                "reverse geocode client not configured".to_string(),
            ))
        })
    }

    fn read_link(&self, _url: &str) -> WorkerFuture<'static, String> {
        Box::pin(async {
            Err(BridgeError::ExternalPayloadInvalid(
                "jina reader client not configured".to_string(),
            ))
        })
    }
}

#[derive(Debug, Default)]
pub struct NoopMediaJobProcessor;

impl MediaJobProcessor for NoopMediaJobProcessor {
    fn process_media_message<'a>(
        &'a self,
        message: &'a StoredMessage,
    ) -> WorkerFuture<'a, ProcessedArtifact> {
        Box::pin(async move {
            Err(BridgeError::MessageUnsupported(format!(
                "media processor not configured for {}",
                message.message_type
            )))
        })
    }
}

#[derive(Clone)]
pub struct Worker {
    store: Store,
    source_writer: SourceWriter,
    processed_artifact_store: Option<ProcessedArtifactStore>,
    external_clients: Arc<dyn ExternalClients>,
    media_processor: Arc<dyn MediaJobProcessor>,
    worker_id: String,
    bridge_version: String,
    retry_policy: RetryPolicy,
}

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub base_delay: std::time::Duration,
    pub max_delay: std::time::Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            base_delay: std::time::Duration::from_secs(10),
            max_delay: std::time::Duration::from_secs(300),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkOutcome {
    NoJob,
    Done { job_id: i64, source_path: PathBuf },
}

impl Worker {
    pub fn new(
        store: Store,
        source_writer: SourceWriter,
        worker_id: impl Into<String>,
        bridge_version: impl Into<String>,
    ) -> Self {
        Self {
            store,
            source_writer,
            processed_artifact_store: None,
            external_clients: Arc::new(NoopExternalClients),
            media_processor: Arc::new(NoopMediaJobProcessor),
            worker_id: worker_id.into(),
            bridge_version: bridge_version.into(),
            retry_policy: RetryPolicy::default(),
        }
    }

    pub fn with_external_clients(
        store: Store,
        source_writer: SourceWriter,
        external_clients: Arc<dyn ExternalClients>,
        worker_id: impl Into<String>,
        bridge_version: impl Into<String>,
    ) -> Self {
        Self {
            store,
            source_writer,
            processed_artifact_store: None,
            external_clients,
            media_processor: Arc::new(NoopMediaJobProcessor),
            worker_id: worker_id.into(),
            bridge_version: bridge_version.into(),
            retry_policy: RetryPolicy::default(),
        }
    }

    pub fn with_processors(
        store: Store,
        source_writer: SourceWriter,
        external_clients: Arc<dyn ExternalClients>,
        media_processor: Arc<dyn MediaJobProcessor>,
        worker_id: impl Into<String>,
        bridge_version: impl Into<String>,
    ) -> Self {
        Self {
            store,
            source_writer,
            processed_artifact_store: None,
            external_clients,
            media_processor,
            worker_id: worker_id.into(),
            bridge_version: bridge_version.into(),
            retry_policy: RetryPolicy::default(),
        }
    }

    pub fn with_processed_artifact_store(
        mut self,
        processed_artifact_store: ProcessedArtifactStore,
    ) -> Self {
        self.processed_artifact_store = Some(processed_artifact_store);
        self
    }

    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    pub async fn process_next(&self, now: &str) -> Result<WorkOutcome, BridgeError> {
        let Some(job) = self.store.claim_next_job(&self.worker_id, now).await? else {
            return Ok(WorkOutcome::NoJob);
        };

        let result = self.process_claimed_job(job.id, job.message_id).await;
        match result {
            Ok(source_path) => {
                self.store.mark_job_done(job.id).await?;
                Ok(WorkOutcome::Done {
                    job_id: job.id,
                    source_path,
                })
            }
            Err(err) => {
                let next_run_at = next_retry_at(now, job.attempts, self.retry_policy);
                self.store
                    .mark_job_retry_or_failed(job.id, &err.to_string(), &next_run_at)
                    .await?;
                Err(err)
            }
        }
    }

    pub async fn requeue_stale_processing_jobs(
        &self,
        now: &str,
        processing_timeout: std::time::Duration,
    ) -> Result<u64, BridgeError> {
        let base =
            OffsetDateTime::parse(now, &Rfc3339).unwrap_or_else(|_| OffsetDateTime::now_utc());
        let timeout_seconds = processing_timeout.as_secs().min(i64::MAX as u64) as i64;
        let locked_before = (base - TimeDuration::seconds(timeout_seconds))
            .format(&Rfc3339)
            .unwrap_or_else(|_| now.to_string());
        self.store
            .requeue_stale_processing_jobs(&locked_before, now)
            .await
    }

    async fn process_claimed_job(
        &self,
        job_id: i64,
        message_id: i64,
    ) -> Result<PathBuf, BridgeError> {
        let message = self.store.get_message(message_id).await?;
        let artifact = self.artifact_from_stored_message(&message).await?;
        self.save_processed_artifact(&artifact)?;
        let mut metadata = metadata_from_stored_message(&message, &self.bridge_version);
        if artifact.provider.is_some() {
            metadata.provider = artifact.provider.clone();
        }
        if artifact.model.is_some() {
            metadata.model = artifact.model.clone();
        }
        if artifact.external_service.is_some() {
            metadata.external_service = artifact.external_service.clone();
        }
        let result = self.source_writer.write_source(&artifact, &metadata)?;
        self.store
            .mark_message_source_written(
                message_id,
                &result.path.display().to_string(),
                &artifact.markdown_body,
            )
            .await?;
        tracing::info!(
            component = "worker",
            job_id,
            message_id,
            source_path = %result.path.display(),
            "message processed"
        );
        Ok(result.path)
    }

    fn save_processed_artifact(&self, artifact: &ProcessedArtifact) -> Result<(), BridgeError> {
        let Some(store) = &self.processed_artifact_store else {
            return Ok(());
        };
        let record = store.save_artifact(
            &artifact.message_key,
            "processed.md",
            artifact.markdown_body.as_bytes(),
        )?;
        if let Some(path) = record.path {
            tracing::debug!(
                component = "worker",
                message_key = %artifact.message_key,
                processed_artifact_path = %path.display(),
                "processed artifact saved"
            );
        }
        Ok(())
    }

    fn save_named_processed_payload(
        &self,
        message: &StoredMessage,
        filename: &str,
        bytes: &[u8],
    ) -> Result<Option<PathBuf>, BridgeError> {
        let Some(store) = &self.processed_artifact_store else {
            return Ok(None);
        };
        let record = store.save_artifact(&message_key(message), filename, bytes)?;
        Ok(record.path)
    }

    async fn artifact_from_stored_message(
        &self,
        message: &StoredMessage,
    ) -> Result<ProcessedArtifact, BridgeError> {
        match message.message_type.as_str() {
            "text" => self.text_artifact_from_stored_message(message).await,
            "location" => {
                let latitude = message.location_lat.ok_or_else(|| {
                    BridgeError::ExternalPayloadInvalid("location_lat missing".to_string())
                })?;
                let longitude = message.location_lng.ok_or_else(|| {
                    BridgeError::ExternalPayloadInvalid("location_lng missing".to_string())
                })?;
                let json = self
                    .external_clients
                    .reverse_geocode(latitude, longitude)
                    .await?;
                let summary = extract_location_summary(&json)?;
                let raw_json_path = self.save_named_processed_payload(
                    message,
                    "tencent-lbs.json",
                    json.as_bytes(),
                )?;
                let location = LocationMessage {
                    common: common_from_stored_message(message),
                    latitude,
                    longitude,
                    scale: message.location_scale,
                    label: message.location_label.clone(),
                };
                Ok(process_location(&location, &summary, raw_json_path))
            }
            "link" => {
                let url = message.link_url.as_deref().ok_or_else(|| {
                    BridgeError::ExternalPayloadInvalid("link_url missing".to_string())
                })?;
                let markdown = self.external_clients.read_link(url).await?;
                let link = LinkMessage {
                    common: common_from_stored_message(message),
                    title: message.link_title.clone(),
                    description: message.link_description.clone(),
                    url: UrlString::new(url),
                };
                Ok(process_link(&link, &markdown, None))
            }
            "image" | "voice" | "video" | "shortvideo" => {
                self.media_processor.process_media_message(message).await
            }
            other => Err(BridgeError::MessageUnsupported(format!(
                "worker does not process {other} yet"
            ))),
        }
    }

    async fn text_artifact_from_stored_message(
        &self,
        message: &StoredMessage,
    ) -> Result<ProcessedArtifact, BridgeError> {
        let content = message.content_text.clone().unwrap_or_default();
        let urls = extract_http_urls(&content);
        if urls.is_empty() {
            return Ok(ProcessedArtifact::new(
                message_key(message),
                ProcessedArtifactKind::Text,
                content,
            ));
        }

        let mut body = String::new();
        body.push_str("## Original Text\n\n");
        body.push_str(content.trim());
        body.push_str("\n\n## Reader Content\n\n");

        let mut payload_paths = Vec::new();
        for (index, url) in urls.iter().enumerate() {
            let markdown = self.external_clients.read_link(url).await?;
            if let Some(path) = self.save_named_processed_payload(
                message,
                &format!("text-link-{}.jina-reader.md", index + 1),
                markdown.as_bytes(),
            )? {
                payload_paths.push(path);
            }
            body.push_str(&format!("### {}\n\n", url));
            body.push_str(markdown.trim());
            body.push_str("\n\n");
        }

        let mut artifact =
            ProcessedArtifact::new(message_key(message), ProcessedArtifactKind::Text, body);
        artifact.external_service = Some("jina_reader".to_string());
        artifact.processed_payload_paths = payload_paths;
        Ok(artifact)
    }
}

fn extract_http_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut offset = 0;
    while offset < text.len() {
        let remaining = &text[offset..];
        let Some(relative_start) = find_next_url_start(remaining) else {
            break;
        };
        let start = offset + relative_start;
        let after_start = &text[start..];
        let relative_end = after_start
            .char_indices()
            .find_map(|(index, ch)| url_delimiter(ch).then_some(index))
            .unwrap_or(after_start.len());
        let raw = &after_start[..relative_end];
        let candidate = trim_url_token(raw);
        if is_http_url(candidate) && !urls.iter().any(|url| url == candidate) {
            urls.push(candidate.to_string());
        }
        offset = start + relative_end.max(1);
    }
    urls
}

fn find_next_url_start(value: &str) -> Option<usize> {
    match (value.find("https://"), value.find("http://")) {
        (Some(https), Some(http)) => Some(https.min(http)),
        (Some(https), None) => Some(https),
        (None, Some(http)) => Some(http),
        (None, None) => None,
    }
}

fn url_delimiter(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '<' | '>'
                | '"'
                | '\''
                | '“'
                | '”'
                | '‘'
                | '’'
                | '，'
                | '。'
                | '；'
                | '：'
                | '！'
                | '？'
                | '、'
        )
}

fn trim_url_token(value: &str) -> &str {
    value.trim_matches(|ch: char| {
        matches!(
            ch,
            '.' | ','
                | ';'
                | ':'
                | '!'
                | '?'
                | ')'
                | ']'
                | '}'
                | '，'
                | '。'
                | '；'
                | '：'
                | '！'
                | '？'
                | '）'
                | '】'
                | '》'
                | '、'
        )
    })
}

fn is_http_url(value: &str) -> bool {
    Url::parse(value)
        .map(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
        .unwrap_or(false)
}

fn common_from_stored_message(message: &StoredMessage) -> CommonFields {
    CommonFields {
        to_user_name: String::new(),
        from_user_name: OpenId::new("stored"),
        create_time: message.create_time.unwrap_or_default(),
        msg_id: message.wechat_msg_id.clone().map(WechatMsgId::new),
    }
}

fn metadata_from_stored_message(message: &StoredMessage, bridge_version: &str) -> SourceMetadata {
    SourceMetadata {
        wechat_msg_id: message.wechat_msg_id.clone(),
        message_type: message.message_type.clone(),
        received_at: message.received_at.clone(),
        wechat_create_time: message.create_time,
        openid_hash: message.from_openid_hash.clone(),
        raw_dir: Some(message.raw_dir.clone()),
        provider: None,
        model: None,
        external_service: None,
        bridge_version: bridge_version.to_string(),
    }
}

fn message_key(message: &StoredMessage) -> String {
    message
        .wechat_msg_id
        .clone()
        .unwrap_or_else(|| format!("message_{}", message.id))
}

fn next_retry_at(now: &str, attempts: i64, retry_policy: RetryPolicy) -> String {
    let base = OffsetDateTime::parse(now, &Rfc3339).unwrap_or_else(|_| OffsetDateTime::now_utc());
    let delay_seconds = retry_delay_seconds(attempts, retry_policy);
    (base + TimeDuration::seconds(delay_seconds))
        .format(&Rfc3339)
        .unwrap_or_else(|_| now.to_string())
}

fn retry_delay_seconds(attempts: i64, retry_policy: RetryPolicy) -> i64 {
    let exponent = attempts.saturating_sub(1).clamp(0, 5) as u32;
    let base = retry_policy.base_delay.as_secs().min(i64::MAX as u64) as i64;
    let max = retry_policy.max_delay.as_secs().min(i64::MAX as u64) as i64;
    base.saturating_mul(2_i64.pow(exponent)).min(max)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sqlx::Row;

    use crate::store::MessageInsert;

    use super::*;

    #[derive(Debug, Clone)]
    struct FakeExternalClients;
    #[derive(Debug, Clone)]
    struct FakeMediaProcessor;

    impl ExternalClients for FakeExternalClients {
        fn reverse_geocode(
            &self,
            _latitude: f64,
            _longitude: f64,
        ) -> WorkerFuture<'static, String> {
            Box::pin(async {
                Ok(
                    include_str!("../../tests/fixtures/external/tencent_lbs_success.json")
                        .to_string(),
                )
            })
        }

        fn read_link(&self, _url: &str) -> WorkerFuture<'static, String> {
            Box::pin(async {
                Ok(
                    include_str!("../../tests/fixtures/external/jina_reader_success.md")
                        .to_string(),
                )
            })
        }
    }

    impl MediaJobProcessor for FakeMediaProcessor {
        fn process_media_message<'a>(
            &'a self,
            message: &'a StoredMessage,
        ) -> WorkerFuture<'a, ProcessedArtifact> {
            Box::pin(async move {
                let kind = match message.message_type.as_str() {
                    "image" => ProcessedArtifactKind::Image,
                    "voice" => ProcessedArtifactKind::Voice,
                    "video" => ProcessedArtifactKind::Video,
                    "shortvideo" => ProcessedArtifactKind::ShortVideo,
                    other => {
                        return Err(BridgeError::MessageUnsupported(other.to_string()));
                    }
                };
                let mut artifact = ProcessedArtifact::new(
                    message_key(message),
                    kind,
                    format!(
                        "processed media {}",
                        message.media_id.as_deref().unwrap_or("")
                    ),
                );
                artifact.provider = Some("gemini".to_string());
                artifact.model = Some("gemini-test".to_string());
                Ok(artifact)
            })
        }
    }

    async fn store_with_job(message: MessageInsert) -> Store {
        let store = Store::connect("sqlite::memory:").await.unwrap();
        store.migrate().await.unwrap();
        let message_id = store.insert_message_idempotent(&message).await.unwrap();
        store
            .create_job_once(message_id, "process_message", "2026-05-27T21:30:15+08:00")
            .await
            .unwrap();
        store
    }

    fn text_message() -> MessageInsert {
        MessageInsert {
            request_id: "req_1".to_string(),
            wechat_msg_id: Some("msg_text_1".to_string()),
            to_user_name: "gh_bridge".to_string(),
            from_openid: "openid-user-1".to_string(),
            from_openid_hash: "sha256:abc".to_string(),
            create_time: Some(1780000001),
            received_at: "2026-05-27T21:30:15+08:00".to_string(),
            message_type: "text".to_string(),
            content_text: Some("hello worker".to_string()),
            media_id: None,
            thumb_media_id: None,
            pic_url: None,
            voice_format: None,
            voice_recognition: None,
            location_lat: None,
            location_lng: None,
            location_scale: None,
            location_label: None,
            link_title: None,
            link_description: None,
            link_url: None,
            authorized: true,
            status: "queued".to_string(),
            raw_dir: "data/raw/msg_text_1".to_string(),
        }
    }

    fn location_message() -> MessageInsert {
        MessageInsert {
            request_id: "req_2".to_string(),
            wechat_msg_id: Some("msg_location_1".to_string()),
            message_type: "location".to_string(),
            location_lat: Some(23.134521),
            location_lng: Some(113.358803),
            location_scale: Some(16),
            location_label: Some("广东省广州市天河区示例路".to_string()),
            raw_dir: "data/raw/msg_location_1".to_string(),
            ..text_message()
        }
    }

    fn link_message() -> MessageInsert {
        MessageInsert {
            request_id: "req_3".to_string(),
            wechat_msg_id: Some("msg_link_1".to_string()),
            message_type: "link".to_string(),
            link_title: Some("示例文章".to_string()),
            link_description: Some("测试链接".to_string()),
            link_url: Some("https://example.com/article".to_string()),
            raw_dir: "data/raw/msg_link_1".to_string(),
            ..text_message()
        }
    }

    fn image_message() -> MessageInsert {
        MessageInsert {
            request_id: "req_4".to_string(),
            wechat_msg_id: Some("msg_image_1".to_string()),
            message_type: "image".to_string(),
            content_text: None,
            media_id: Some("media-image-1".to_string()),
            pic_url: Some("https://mmbiz.qpic.cn/example.jpg".to_string()),
            raw_dir: "data/raw/msg_image_1".to_string(),
            ..text_message()
        }
    }

    #[tokio::test]
    async fn worker_processes_text_job_to_source() {
        let store = store_with_job(text_message()).await;
        let source_dir = tempfile::tempdir().unwrap();
        let processed_dir = tempfile::tempdir().unwrap();
        let worker = Worker::new(
            store.clone(),
            SourceWriter::new(source_dir.path()),
            "worker-1",
            "0.1.0",
        )
        .with_processed_artifact_store(ProcessedArtifactStore::new(processed_dir.path()));

        let outcome = worker
            .process_next("2026-05-27T21:30:16+08:00")
            .await
            .unwrap();

        let WorkOutcome::Done { source_path, .. } = outcome else {
            panic!("expected done");
        };
        let source = std::fs::read_to_string(source_path).unwrap();
        assert!(source.contains("hello worker"));
        let processed =
            std::fs::read_to_string(processed_dir.path().join("msg_text_1").join("processed.md"))
                .unwrap();
        assert_eq!(processed, "hello worker");

        let message_status: String = sqlx::query_scalar("SELECT status FROM messages LIMIT 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
        let job_status: String = sqlx::query_scalar("SELECT status FROM jobs LIMIT 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(message_status, "source_written");
        assert_eq!(job_status, "done");
    }

    #[tokio::test]
    async fn worker_expands_urls_inside_text_with_jina_reader() {
        let store = store_with_job(MessageInsert {
            content_text: Some(
                "你看看这个嘛\n\nhttps://example.com/article\n\n我觉得够判断了".to_string(),
            ),
            ..text_message()
        })
        .await;
        let source_dir = tempfile::tempdir().unwrap();
        let processed_dir = tempfile::tempdir().unwrap();
        let worker = Worker::with_external_clients(
            store,
            SourceWriter::new(source_dir.path()),
            Arc::new(FakeExternalClients),
            "worker-1",
            "0.1.0",
        )
        .with_processed_artifact_store(ProcessedArtifactStore::new(processed_dir.path()));

        let outcome = worker
            .process_next("2026-05-27T21:30:16+08:00")
            .await
            .unwrap();

        let WorkOutcome::Done { source_path, .. } = outcome else {
            panic!("expected done");
        };
        let source = std::fs::read_to_string(source_path).unwrap();
        assert!(source.contains("## Original Text"));
        assert!(source.contains("## Reader Content"));
        assert!(source.contains("https://example.com/article"));
        assert!(source.contains("这是 Jina Reader 返回的 Markdown 内容"));
        let reader_payload = std::fs::read_to_string(
            processed_dir
                .path()
                .join("msg_text_1")
                .join("text-link-1.jina-reader.md"),
        )
        .unwrap();
        assert!(reader_payload.contains("这是 Jina Reader 返回的 Markdown 内容"));
    }

    #[test]
    fn extracts_urls_from_text() {
        assert_eq!(
            extract_http_urls("看这个：https://example.com/a?b=1。再看 https://example.org/x)."),
            vec![
                "https://example.com/a?b=1".to_string(),
                "https://example.org/x".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn worker_returns_no_job_when_queue_empty() {
        let store = Store::connect("sqlite::memory:").await.unwrap();
        store.migrate().await.unwrap();
        let source_dir = tempfile::tempdir().unwrap();
        let worker = Worker::new(
            store,
            SourceWriter::new(source_dir.path()),
            "worker-1",
            "0.1.0",
        );

        let outcome = worker
            .process_next("2026-05-27T21:30:16+08:00")
            .await
            .unwrap();

        assert_eq!(outcome, WorkOutcome::NoJob);
    }

    #[tokio::test]
    async fn worker_requeues_failed_job_with_backoff() {
        let store = store_with_job(location_message()).await;
        let source_dir = tempfile::tempdir().unwrap();
        let worker = Worker::new(
            store.clone(),
            SourceWriter::new(source_dir.path()),
            "worker-1",
            "0.1.0",
        );

        let err = worker
            .process_next("2026-05-27T21:30:16+08:00")
            .await
            .unwrap_err();
        assert!(matches!(err, BridgeError::ExternalPayloadInvalid(_)));

        let row = sqlx::query("SELECT status, attempts, next_run_at, last_error FROM jobs LIMIT 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>("status"), "pending");
        assert_eq!(row.get::<i64, _>("attempts"), 1);
        assert_eq!(
            row.get::<String, _>("next_run_at"),
            "2026-05-27T21:30:26+08:00"
        );
        assert!(
            row.get::<String, _>("last_error")
                .contains("reverse geocode client not configured")
        );

        let outcome = worker
            .process_next("2026-05-27T21:30:25+08:00")
            .await
            .unwrap();
        assert_eq!(outcome, WorkOutcome::NoJob);
    }

    #[tokio::test]
    async fn worker_requeues_stale_processing_jobs() {
        let store = store_with_job(text_message()).await;
        store
            .claim_next_job("worker-1", "2026-05-27T21:30:16+08:00")
            .await
            .unwrap();
        let source_dir = tempfile::tempdir().unwrap();
        let worker = Worker::new(
            store.clone(),
            SourceWriter::new(source_dir.path()),
            "worker-2",
            "0.1.0",
        );

        let count = worker
            .requeue_stale_processing_jobs(
                "2026-05-27T21:45:16+08:00",
                std::time::Duration::from_secs(900),
            )
            .await
            .unwrap();

        assert_eq!(count, 1);
        let outcome = worker
            .process_next("2026-05-27T21:45:17+08:00")
            .await
            .unwrap();
        assert!(matches!(outcome, WorkOutcome::Done { .. }));
    }

    #[tokio::test]
    async fn worker_processes_location_job_to_source() {
        let store = store_with_job(location_message()).await;
        let source_dir = tempfile::tempdir().unwrap();
        let processed_dir = tempfile::tempdir().unwrap();
        let worker = Worker::with_external_clients(
            store,
            SourceWriter::new(source_dir.path()),
            Arc::new(FakeExternalClients),
            "worker-1",
            "0.1.0",
        )
        .with_processed_artifact_store(ProcessedArtifactStore::new(processed_dir.path()));

        let outcome = worker
            .process_next("2026-05-27T21:30:16+08:00")
            .await
            .unwrap();

        let WorkOutcome::Done { source_path, .. } = outcome else {
            panic!("expected done");
        };
        let source = std::fs::read_to_string(source_path).unwrap();
        assert!(source.contains("Adcode: 440106"));
        assert!(source.contains("Coordinates: 23.134521, 113.358803"));
        let lbs_json = std::fs::read_to_string(
            processed_dir
                .path()
                .join("msg_location_1")
                .join("tencent-lbs.json"),
        )
        .unwrap();
        assert!(lbs_json.contains("\"adcode\": \"440106\""));
    }

    #[tokio::test]
    async fn worker_processes_link_job_to_source() {
        let store = store_with_job(link_message()).await;
        let source_dir = tempfile::tempdir().unwrap();
        let worker = Worker::with_external_clients(
            store,
            SourceWriter::new(source_dir.path()),
            Arc::new(FakeExternalClients),
            "worker-1",
            "0.1.0",
        );

        let outcome = worker
            .process_next("2026-05-27T21:30:16+08:00")
            .await
            .unwrap();

        let WorkOutcome::Done { source_path, .. } = outcome else {
            panic!("expected done");
        };
        let source = std::fs::read_to_string(source_path).unwrap();
        assert!(source.contains("## Reader Content"));
        assert!(source.contains("这是 Jina Reader 返回的 Markdown 内容"));
    }

    #[tokio::test]
    async fn worker_processes_media_job_through_media_processor() {
        let store = store_with_job(image_message()).await;
        let source_dir = tempfile::tempdir().unwrap();
        let worker = Worker::with_processors(
            store,
            SourceWriter::new(source_dir.path()),
            Arc::new(FakeExternalClients),
            Arc::new(FakeMediaProcessor),
            "worker-1",
            "0.1.0",
        );

        let outcome = worker
            .process_next("2026-05-27T21:30:16+08:00")
            .await
            .unwrap();

        let WorkOutcome::Done { source_path, .. } = outcome else {
            panic!("expected done");
        };
        let source = std::fs::read_to_string(source_path).unwrap();
        assert!(source.contains("processed media media-image-1"));
        assert!(source.contains("provider: \"gemini\""));
        assert!(source.contains("model: \"gemini-test\""));
    }
}
