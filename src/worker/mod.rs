use std::path::PathBuf;

use crate::{
    error::BridgeError,
    preprocess::artifact::{ProcessedArtifact, ProcessedArtifactKind},
    source::{SourceMetadata, SourceWriter},
    store::{Store, StoredMessage},
};

#[derive(Debug, Clone)]
pub struct Worker {
    store: Store,
    source_writer: SourceWriter,
    worker_id: String,
    bridge_version: String,
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
            worker_id: worker_id.into(),
            bridge_version: bridge_version.into(),
        }
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
                self.store.mark_job_failed(job.id, &err.to_string()).await?;
                Err(err)
            }
        }
    }

    async fn process_claimed_job(
        &self,
        job_id: i64,
        message_id: i64,
    ) -> Result<PathBuf, BridgeError> {
        let message = self.store.get_message(message_id).await?;
        let artifact = artifact_from_stored_message(&message)?;
        let metadata = metadata_from_stored_message(&message, &self.bridge_version);
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
}

fn artifact_from_stored_message(message: &StoredMessage) -> Result<ProcessedArtifact, BridgeError> {
    match message.message_type.as_str() {
        "text" => Ok(ProcessedArtifact::new(
            message_key(message),
            ProcessedArtifactKind::Text,
            message.content_text.clone().unwrap_or_default(),
        )),
        other => Err(BridgeError::MessageUnsupported(format!(
            "worker does not process {other} yet"
        ))),
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

#[cfg(test)]
mod tests {
    use crate::store::MessageInsert;

    use super::*;

    async fn store_with_text_job() -> Store {
        let store = Store::connect("sqlite::memory:").await.unwrap();
        store.migrate().await.unwrap();
        let message_id = store
            .insert_message_idempotent(&MessageInsert {
                request_id: "req_1".to_string(),
                wechat_msg_id: Some("msg_text_1".to_string()),
                to_user_name: "gh_bridge".to_string(),
                from_openid: "openid-user-1".to_string(),
                from_openid_hash: "sha256:abc".to_string(),
                create_time: Some(1780000001),
                received_at: "2026-05-27T21:30:15+08:00".to_string(),
                message_type: "text".to_string(),
                content_text: Some("hello worker".to_string()),
                authorized: true,
                status: "queued".to_string(),
                raw_dir: "data/raw/msg_text_1".to_string(),
            })
            .await
            .unwrap();
        store
            .create_job_once(message_id, "process_message", "2026-05-27T21:30:15+08:00")
            .await
            .unwrap();
        store
    }

    #[tokio::test]
    async fn worker_processes_text_job_to_source() {
        let store = store_with_text_job().await;
        let source_dir = tempfile::tempdir().unwrap();
        let worker = Worker::new(
            store.clone(),
            SourceWriter::new(source_dir.path()),
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
        assert!(source.contains("hello worker"));

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
}
