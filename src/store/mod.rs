use sqlx::{Row, SqlitePool, sqlite::SqlitePoolOptions};

use crate::error::BridgeError;

#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageInsert {
    pub request_id: String,
    pub wechat_msg_id: Option<String>,
    pub to_user_name: String,
    pub from_openid: String,
    pub from_openid_hash: String,
    pub create_time: Option<i64>,
    pub received_at: String,
    pub message_type: String,
    pub content_text: Option<String>,
    pub authorized: bool,
    pub status: String,
    pub raw_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: i64,
    pub message_id: i64,
    pub job_type: String,
    pub status: String,
    pub attempts: i64,
}

impl Store {
    pub async fn connect(database_url: &str) -> Result<Self, BridgeError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .min_connections(1)
            .connect(database_url)
            .await
            .map_err(|err| BridgeError::Database(err.to_string()))?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<(), BridgeError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|err| BridgeError::Database(err.to_string()))
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn upsert_whitelist(
        &self,
        openid: &str,
        openid_hash: &str,
        source: &str,
    ) -> Result<(), BridgeError> {
        sqlx::query(
            r#"
            INSERT INTO whitelist_subjects (openid, openid_hash, source, enabled)
            VALUES (?1, ?2, ?3, 1)
            ON CONFLICT(openid) DO UPDATE SET
              openid_hash = excluded.openid_hash,
              source = excluded.source,
              enabled = 1,
              updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(openid)
        .bind(openid_hash)
        .bind(source)
        .execute(&self.pool)
        .await
        .map_err(|err| BridgeError::Database(err.to_string()))?;
        Ok(())
    }

    pub async fn is_openid_whitelisted(&self, openid: &str) -> Result<bool, BridgeError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM whitelist_subjects WHERE openid = ?1 AND enabled = 1",
        )
        .bind(openid)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| BridgeError::Database(err.to_string()))?;
        Ok(count > 0)
    }

    pub async fn insert_message_idempotent(
        &self,
        message: &MessageInsert,
    ) -> Result<i64, BridgeError> {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO messages (
              request_id, wechat_msg_id, to_user_name, from_openid, from_openid_hash,
              create_time, received_at, message_type, content_text, authorized, status, raw_dir
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
        )
        .bind(&message.request_id)
        .bind(&message.wechat_msg_id)
        .bind(&message.to_user_name)
        .bind(&message.from_openid)
        .bind(&message.from_openid_hash)
        .bind(message.create_time)
        .bind(&message.received_at)
        .bind(&message.message_type)
        .bind(&message.content_text)
        .bind(if message.authorized { 1 } else { 0 })
        .bind(&message.status)
        .bind(&message.raw_dir)
        .execute(&self.pool)
        .await
        .map_err(|err| BridgeError::Database(err.to_string()))?;

        let id: i64 = if let Some(msg_id) = &message.wechat_msg_id {
            sqlx::query_scalar("SELECT id FROM messages WHERE wechat_msg_id = ?1")
                .bind(msg_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|err| BridgeError::Database(err.to_string()))?
        } else {
            sqlx::query_scalar("SELECT last_insert_rowid()")
                .fetch_one(&self.pool)
                .await
                .map_err(|err| BridgeError::Database(err.to_string()))?
        };

        Ok(id)
    }

    pub async fn create_job_once(
        &self,
        message_id: i64,
        job_type: &str,
        next_run_at: &str,
    ) -> Result<i64, BridgeError> {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO jobs (message_id, job_type, status, next_run_at)
            VALUES (?1, ?2, 'pending', ?3)
            "#,
        )
        .bind(message_id)
        .bind(job_type)
        .bind(next_run_at)
        .execute(&self.pool)
        .await
        .map_err(|err| BridgeError::Database(err.to_string()))?;

        let id: i64 =
            sqlx::query_scalar("SELECT id FROM jobs WHERE message_id = ?1 AND job_type = ?2")
                .bind(message_id)
                .bind(job_type)
                .fetch_one(&self.pool)
                .await
                .map_err(|err| BridgeError::Database(err.to_string()))?;
        Ok(id)
    }

    pub async fn claim_next_job(
        &self,
        worker_id: &str,
        now: &str,
    ) -> Result<Option<Job>, BridgeError> {
        let row = sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'processing',
                locked_at = ?1,
                locked_by = ?2,
                attempts = attempts + 1,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = (
              SELECT id FROM jobs
              WHERE status = 'pending'
                AND next_run_at <= ?1
              ORDER BY next_run_at ASC, id ASC
              LIMIT 1
            )
            RETURNING id, message_id, job_type, status, attempts
            "#,
        )
        .bind(now)
        .bind(worker_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| BridgeError::Database(err.to_string()))?;

        Ok(row.map(|row| Job {
            id: row.get("id"),
            message_id: row.get("message_id"),
            job_type: row.get("job_type"),
            status: row.get("status"),
            attempts: row.get("attempts"),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_store() -> Store {
        let store = Store::connect("sqlite::memory:").await.unwrap();
        store.migrate().await.unwrap();
        store
    }

    fn message(msg_id: &str) -> MessageInsert {
        MessageInsert {
            request_id: "req_1".to_string(),
            wechat_msg_id: Some(msg_id.to_string()),
            to_user_name: "gh_bridge".to_string(),
            from_openid: "openid-user-1".to_string(),
            from_openid_hash: "sha256:abc".to_string(),
            create_time: Some(1780000001),
            received_at: "2026-05-27T21:30:15+08:00".to_string(),
            message_type: "text".to_string(),
            content_text: Some("hello".to_string()),
            authorized: true,
            status: "queued".to_string(),
            raw_dir: "data/raw/msg".to_string(),
        }
    }

    #[tokio::test]
    async fn whitelist_round_trip() {
        let store = test_store().await;

        assert!(!store.is_openid_whitelisted("openid-user-1").await.unwrap());
        store
            .upsert_whitelist("openid-user-1", "sha256:abc", "test")
            .await
            .unwrap();

        assert!(store.is_openid_whitelisted("openid-user-1").await.unwrap());
    }

    #[tokio::test]
    async fn insert_message_is_idempotent_by_msg_id() {
        let store = test_store().await;

        let first = store
            .insert_message_idempotent(&message("msg_1"))
            .await
            .unwrap();
        let second = store
            .insert_message_idempotent(&message("msg_1"))
            .await
            .unwrap();

        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn create_job_once_is_idempotent() {
        let store = test_store().await;
        let message_id = store
            .insert_message_idempotent(&message("msg_1"))
            .await
            .unwrap();

        let first = store
            .create_job_once(message_id, "process_message", "2026-05-27T21:30:15+08:00")
            .await
            .unwrap();
        let second = store
            .create_job_once(message_id, "process_message", "2026-05-27T21:30:15+08:00")
            .await
            .unwrap();

        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn claim_next_job_moves_pending_to_processing() {
        let store = test_store().await;
        let message_id = store
            .insert_message_idempotent(&message("msg_1"))
            .await
            .unwrap();
        let job_id = store
            .create_job_once(message_id, "process_message", "2026-05-27T21:30:15+08:00")
            .await
            .unwrap();

        let job = store
            .claim_next_job("worker-1", "2026-05-27T21:30:16+08:00")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(job.id, job_id);
        assert_eq!(job.status, "processing");
        assert_eq!(job.attempts, 1);

        let none = store
            .claim_next_job("worker-1", "2026-05-27T21:30:17+08:00")
            .await
            .unwrap();
        assert!(none.is_none());
    }
}
