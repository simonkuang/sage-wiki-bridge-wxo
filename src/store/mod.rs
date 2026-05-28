use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::str::FromStr;

use crate::error::BridgeError;

#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

#[derive(Debug, Clone, PartialEq)]
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
    pub media_id: Option<String>,
    pub thumb_media_id: Option<String>,
    pub pic_url: Option<String>,
    pub voice_format: Option<String>,
    pub voice_recognition: Option<String>,
    pub location_lat: Option<f64>,
    pub location_lng: Option<f64>,
    pub location_scale: Option<i32>,
    pub location_label: Option<String>,
    pub link_title: Option<String>,
    pub link_description: Option<String>,
    pub link_url: Option<String>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct StoredMessage {
    pub id: i64,
    pub wechat_msg_id: Option<String>,
    pub from_openid_hash: String,
    pub create_time: Option<i64>,
    pub received_at: String,
    pub message_type: String,
    pub content_text: Option<String>,
    pub media_id: Option<String>,
    pub thumb_media_id: Option<String>,
    pub pic_url: Option<String>,
    pub voice_format: Option<String>,
    pub voice_recognition: Option<String>,
    pub location_lat: Option<f64>,
    pub location_lng: Option<f64>,
    pub location_scale: Option<i32>,
    pub location_label: Option<String>,
    pub link_title: Option<String>,
    pub link_description: Option<String>,
    pub link_url: Option<String>,
    pub raw_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageListQuery {
    pub page: u32,
    pub per_page: u32,
    pub keyword: Option<String>,
    pub message_type: Option<String>,
    pub status: Option<String>,
    pub sort_desc: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageListItem {
    pub id: i64,
    pub received_at: String,
    pub message_type: String,
    pub from_openid_hash: String,
    pub status: String,
    pub content_preview: Option<String>,
    pub processed_preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageListPage {
    pub items: Vec<MessageListItem>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MessageDetail {
    pub id: i64,
    pub request_id: String,
    pub wechat_msg_id: Option<String>,
    pub from_openid_hash: String,
    pub create_time: Option<i64>,
    pub received_at: String,
    pub message_type: String,
    pub content_text: Option<String>,
    pub media_id: Option<String>,
    pub thumb_media_id: Option<String>,
    pub pic_url: Option<String>,
    pub voice_format: Option<String>,
    pub voice_recognition: Option<String>,
    pub location_lat: Option<f64>,
    pub location_lng: Option<f64>,
    pub location_scale: Option<i32>,
    pub location_label: Option<String>,
    pub link_title: Option<String>,
    pub link_description: Option<String>,
    pub link_url: Option<String>,
    pub authorized: bool,
    pub status: String,
    pub raw_dir: String,
    pub source_path: Option<String>,
    pub processed_text: Option<String>,
    pub processed_at: Option<String>,
}

impl Store {
    pub async fn connect(database_url: &str) -> Result<Self, BridgeError> {
        Self::connect_with_pool_options(database_url, 4, 1).await
    }

    pub async fn connect_with_pool_options(
        database_url: &str,
        max_connections: u32,
        min_connections: u32,
    ) -> Result<Self, BridgeError> {
        let options = SqliteConnectOptions::from_str(database_url)
            .map_err(|err| BridgeError::Database(err.to_string()))?
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(max_connections)
            .min_connections(min_connections.min(max_connections))
            .connect_with(options)
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
              create_time, received_at, message_type, content_text,
              media_id, thumb_media_id, pic_url, voice_format, voice_recognition,
              location_lat, location_lng, location_scale, location_label,
              link_title, link_description, link_url,
              authorized, status, raw_dir
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)
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
        .bind(&message.media_id)
        .bind(&message.thumb_media_id)
        .bind(&message.pic_url)
        .bind(&message.voice_format)
        .bind(&message.voice_recognition)
        .bind(message.location_lat)
        .bind(message.location_lng)
        .bind(message.location_scale)
        .bind(&message.location_label)
        .bind(&message.link_title)
        .bind(&message.link_description)
        .bind(&message.link_url)
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

    pub async fn requeue_stale_processing_jobs(
        &self,
        locked_before: &str,
        next_run_at: &str,
    ) -> Result<u64, BridgeError> {
        let result = sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'pending',
                next_run_at = ?2,
                locked_at = NULL,
                locked_by = NULL,
                last_error = 'requeued after processing timeout',
                updated_at = CURRENT_TIMESTAMP
            WHERE status = 'processing'
              AND locked_at IS NOT NULL
              AND locked_at <= ?1
            "#,
        )
        .bind(locked_before)
        .bind(next_run_at)
        .execute(&self.pool)
        .await
        .map_err(|err| BridgeError::Database(err.to_string()))?;

        Ok(result.rows_affected())
    }

    pub async fn get_message(&self, message_id: i64) -> Result<StoredMessage, BridgeError> {
        let row = sqlx::query(
            r#"
            SELECT id, wechat_msg_id, from_openid_hash, create_time, received_at,
                   message_type, content_text,
                   media_id, thumb_media_id, pic_url, voice_format, voice_recognition,
                   location_lat, location_lng, location_scale, location_label,
                   link_title, link_description, link_url,
                   raw_dir
            FROM messages
            WHERE id = ?1
            "#,
        )
        .bind(message_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| BridgeError::Database(err.to_string()))?;

        Ok(StoredMessage {
            id: row.get("id"),
            wechat_msg_id: row.get("wechat_msg_id"),
            from_openid_hash: row.get("from_openid_hash"),
            create_time: row.get("create_time"),
            received_at: row.get("received_at"),
            message_type: row.get("message_type"),
            content_text: row.get("content_text"),
            media_id: row.get("media_id"),
            thumb_media_id: row.get("thumb_media_id"),
            pic_url: row.get("pic_url"),
            voice_format: row.get("voice_format"),
            voice_recognition: row.get("voice_recognition"),
            location_lat: row.get("location_lat"),
            location_lng: row.get("location_lng"),
            location_scale: row.get("location_scale"),
            location_label: row.get("location_label"),
            link_title: row.get("link_title"),
            link_description: row.get("link_description"),
            link_url: row.get("link_url"),
            raw_dir: row.get("raw_dir"),
        })
    }

    pub async fn mark_message_source_written(
        &self,
        message_id: i64,
        source_path: &str,
        processed_text: &str,
    ) -> Result<(), BridgeError> {
        sqlx::query(
            r#"
            UPDATE messages
            SET status = 'source_written',
                source_path = ?2,
                processed_text = ?3,
                processed_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#,
        )
        .bind(message_id)
        .bind(source_path)
        .bind(processed_text)
        .execute(&self.pool)
        .await
        .map_err(|err| BridgeError::Database(err.to_string()))?;

        Ok(())
    }

    pub async fn mark_job_done(&self, job_id: i64) -> Result<(), BridgeError> {
        sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'done',
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|err| BridgeError::Database(err.to_string()))?;
        Ok(())
    }

    pub async fn mark_job_failed(&self, job_id: i64, error: &str) -> Result<(), BridgeError> {
        sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'failed',
                last_error = ?2,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(|err| BridgeError::Database(err.to_string()))?;
        Ok(())
    }

    pub async fn mark_job_retry_or_failed(
        &self,
        job_id: i64,
        error: &str,
        next_run_at: &str,
    ) -> Result<(), BridgeError> {
        sqlx::query(
            r#"
            UPDATE jobs
            SET status = CASE
                    WHEN attempts < max_attempts THEN 'pending'
                    ELSE 'failed'
                END,
                next_run_at = CASE
                    WHEN attempts < max_attempts THEN ?3
                    ELSE next_run_at
                END,
                locked_at = NULL,
                locked_by = NULL,
                last_error = ?2,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .bind(error)
        .bind(next_run_at)
        .execute(&self.pool)
        .await
        .map_err(|err| BridgeError::Database(err.to_string()))?;

        sqlx::query(
            r#"
            UPDATE messages
            SET status = 'failed',
                error_kind = 'worker',
                error_message = ?2,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = (
                SELECT message_id
                FROM jobs
                WHERE id = ?1
                  AND status = 'failed'
            )
            "#,
        )
        .bind(job_id)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(|err| BridgeError::Database(err.to_string()))?;
        Ok(())
    }

    pub async fn list_messages(
        &self,
        query: &MessageListQuery,
    ) -> Result<MessageListPage, BridgeError> {
        let page = query.page.max(1);
        let per_page = query.per_page.clamp(1, 100);
        let offset = i64::from((page - 1) * per_page);
        let limit = i64::from(per_page);
        let keyword = query.keyword.as_deref().filter(|value| !value.is_empty());
        let keyword_like = keyword.map(|value| format!("%{value}%"));
        let message_type = query
            .message_type
            .as_deref()
            .filter(|value| !value.is_empty());
        let status = query.status.as_deref().filter(|value| !value.is_empty());

        let total: i64 = sqlx::query_scalar(message_filter_count_sql())
            .bind(&keyword_like)
            .bind(message_type)
            .bind(status)
            .fetch_one(&self.pool)
            .await
            .map_err(|err| BridgeError::Database(err.to_string()))?;

        let sql = if query.sort_desc {
            list_messages_sql("DESC")
        } else {
            list_messages_sql("ASC")
        };
        let rows = sqlx::query(&sql)
            .bind(&keyword_like)
            .bind(message_type)
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| BridgeError::Database(err.to_string()))?;

        Ok(MessageListPage {
            items: rows
                .into_iter()
                .map(|row| MessageListItem {
                    id: row.get("id"),
                    received_at: row.get("received_at"),
                    message_type: row.get("message_type"),
                    from_openid_hash: row.get("from_openid_hash"),
                    status: row.get("status"),
                    content_preview: row.get("content_preview"),
                    processed_preview: row.get("processed_preview"),
                })
                .collect(),
            total,
            page,
            per_page,
        })
    }

    pub async fn get_message_detail(&self, message_id: i64) -> Result<MessageDetail, BridgeError> {
        let row = sqlx::query(
            r#"
            SELECT id, request_id, wechat_msg_id, from_openid_hash, create_time, received_at,
                   message_type, content_text,
                   media_id, thumb_media_id, pic_url, voice_format, voice_recognition,
                   location_lat, location_lng, location_scale, location_label,
                   link_title, link_description, link_url,
                   authorized, status, raw_dir, source_path, processed_text, processed_at
            FROM messages
            WHERE id = ?1
            "#,
        )
        .bind(message_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| BridgeError::Database(err.to_string()))?;

        Ok(MessageDetail {
            id: row.get("id"),
            request_id: row.get("request_id"),
            wechat_msg_id: row.get("wechat_msg_id"),
            from_openid_hash: row.get("from_openid_hash"),
            create_time: row.get("create_time"),
            received_at: row.get("received_at"),
            message_type: row.get("message_type"),
            content_text: row.get("content_text"),
            media_id: row.get("media_id"),
            thumb_media_id: row.get("thumb_media_id"),
            pic_url: row.get("pic_url"),
            voice_format: row.get("voice_format"),
            voice_recognition: row.get("voice_recognition"),
            location_lat: row.get("location_lat"),
            location_lng: row.get("location_lng"),
            location_scale: row.get("location_scale"),
            location_label: row.get("location_label"),
            link_title: row.get("link_title"),
            link_description: row.get("link_description"),
            link_url: row.get("link_url"),
            authorized: row.get::<i64, _>("authorized") == 1,
            status: row.get("status"),
            raw_dir: row.get("raw_dir"),
            source_path: row.get("source_path"),
            processed_text: row.get("processed_text"),
            processed_at: row.get("processed_at"),
        })
    }
}

fn message_filter_count_sql() -> &'static str {
    r#"
    SELECT COUNT(*)
    FROM messages
    WHERE (?1 IS NULL OR content_text LIKE ?1 OR processed_text LIKE ?1 OR link_url LIKE ?1 OR location_label LIKE ?1 OR media_id LIKE ?1 OR from_openid_hash LIKE ?1)
      AND (?2 IS NULL OR message_type = ?2)
      AND (?3 IS NULL OR status = ?3)
    "#
}

fn list_messages_sql(order: &str) -> String {
    format!(
        r#"
        SELECT id, received_at, message_type, from_openid_hash, status,
               substr(COALESCE(content_text, link_url, location_label, media_id, ''), 1, 160) AS content_preview,
               substr(COALESCE(processed_text, ''), 1, 160) AS processed_preview
        FROM messages
        WHERE (?1 IS NULL OR content_text LIKE ?1 OR processed_text LIKE ?1 OR link_url LIKE ?1 OR location_label LIKE ?1 OR media_id LIKE ?1 OR from_openid_hash LIKE ?1)
          AND (?2 IS NULL OR message_type = ?2)
          AND (?3 IS NULL OR status = ?3)
        ORDER BY received_at {order}, id {order}
        LIMIT ?4 OFFSET ?5
        "#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_store() -> Store {
        let store = Store::connect("sqlite::memory:").await.unwrap();
        store.migrate().await.unwrap();
        store
    }

    #[tokio::test]
    async fn connect_creates_missing_database_file() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("bridge.sqlite3");
        let database_url = format!("sqlite://{}", db_path.display());

        let store = Store::connect(&database_url).await.unwrap();
        store.migrate().await.unwrap();

        assert!(db_path.exists());
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
    async fn list_messages_filters_by_keyword_type_and_status() {
        let store = test_store().await;
        store
            .insert_message_idempotent(&MessageInsert {
                request_id: "req_text".to_string(),
                wechat_msg_id: Some("msg_text".to_string()),
                content_text: Some("alpha text".to_string()),
                message_type: "text".to_string(),
                status: "queued".to_string(),
                ..message("msg_text")
            })
            .await
            .unwrap();
        let image_id = store
            .insert_message_idempotent(&MessageInsert {
                request_id: "req_image".to_string(),
                wechat_msg_id: Some("msg_image".to_string()),
                content_text: None,
                media_id: Some("alpha-media".to_string()),
                message_type: "image".to_string(),
                status: "source_written".to_string(),
                ..message("msg_image")
            })
            .await
            .unwrap();

        let page = store
            .list_messages(&MessageListQuery {
                page: 1,
                per_page: 20,
                keyword: Some("alpha".to_string()),
                message_type: Some("image".to_string()),
                status: Some("source_written".to_string()),
                sort_desc: true,
            })
            .await
            .unwrap();

        assert_eq!(page.total, 1);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, image_id);
        assert_eq!(page.items[0].message_type, "image");
        assert_eq!(page.items[0].status, "source_written");
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

    #[tokio::test]
    async fn marks_message_source_written_and_job_done() {
        let store = test_store().await;
        let message_id = store
            .insert_message_idempotent(&message("msg_1"))
            .await
            .unwrap();
        let job_id = store
            .create_job_once(message_id, "process_message", "2026-05-27T21:30:15+08:00")
            .await
            .unwrap();

        store
            .mark_message_source_written(message_id, "/tmp/source.md", "hello")
            .await
            .unwrap();
        store.mark_job_done(job_id).await.unwrap();

        let status: String = sqlx::query_scalar("SELECT status FROM messages WHERE id = ?1")
            .bind(message_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
        let job_status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = ?1")
            .bind(job_id)
            .fetch_one(store.pool())
            .await
            .unwrap();

        assert_eq!(status, "source_written");
        assert_eq!(job_status, "done");
    }

    #[tokio::test]
    async fn failed_job_is_requeued_until_max_attempts() {
        let store = test_store().await;
        let message_id = store
            .insert_message_idempotent(&message("msg_1"))
            .await
            .unwrap();
        let job_id = store
            .create_job_once(message_id, "process_message", "2026-05-27T21:30:15+08:00")
            .await
            .unwrap();

        store
            .claim_next_job("worker-1", "2026-05-27T21:30:16+08:00")
            .await
            .unwrap();
        store
            .mark_job_retry_or_failed(job_id, "temporary failure", "2026-05-27T21:30:26+08:00")
            .await
            .unwrap();

        let row = sqlx::query(
            "SELECT status, attempts, next_run_at, locked_by, last_error FROM jobs WHERE id = ?1",
        )
        .bind(job_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(row.get::<String, _>("status"), "pending");
        assert_eq!(row.get::<i64, _>("attempts"), 1);
        assert_eq!(
            row.get::<String, _>("next_run_at"),
            "2026-05-27T21:30:26+08:00"
        );
        assert_eq!(row.get::<Option<String>, _>("locked_by"), None);
        assert_eq!(row.get::<String, _>("last_error"), "temporary failure");
    }

    #[tokio::test]
    async fn failed_job_stays_failed_at_max_attempts() {
        let store = test_store().await;
        let message_id = store
            .insert_message_idempotent(&message("msg_1"))
            .await
            .unwrap();
        let job_id = store
            .create_job_once(message_id, "process_message", "2026-05-27T21:30:15+08:00")
            .await
            .unwrap();
        sqlx::query("UPDATE jobs SET attempts = 3 WHERE id = ?1")
            .bind(job_id)
            .execute(store.pool())
            .await
            .unwrap();

        store
            .mark_job_retry_or_failed(job_id, "permanent failure", "2026-05-27T21:35:00+08:00")
            .await
            .unwrap();

        let row = sqlx::query("SELECT status, next_run_at, last_error FROM jobs WHERE id = ?1")
            .bind(job_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>("status"), "failed");
        assert_eq!(
            row.get::<String, _>("next_run_at"),
            "2026-05-27T21:30:15+08:00"
        );
        assert_eq!(row.get::<String, _>("last_error"), "permanent failure");

        let message_row =
            sqlx::query("SELECT status, error_kind, error_message FROM messages WHERE id = ?1")
                .bind(message_id)
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(message_row.get::<String, _>("status"), "failed");
        assert_eq!(message_row.get::<String, _>("error_kind"), "worker");
        assert_eq!(
            message_row.get::<String, _>("error_message"),
            "permanent failure"
        );
    }

    #[tokio::test]
    async fn requeues_stale_processing_jobs() {
        let store = test_store().await;
        let message_id = store
            .insert_message_idempotent(&message("msg_1"))
            .await
            .unwrap();
        let stale_job_id = store
            .create_job_once(message_id, "process_message", "2026-05-27T21:30:15+08:00")
            .await
            .unwrap();
        store
            .claim_next_job("worker-1", "2026-05-27T21:30:16+08:00")
            .await
            .unwrap();
        let fresh_message_id = store
            .insert_message_idempotent(&message("msg_2"))
            .await
            .unwrap();
        let fresh_job_id = store
            .create_job_once(
                fresh_message_id,
                "process_message",
                "2026-05-27T21:30:15+08:00",
            )
            .await
            .unwrap();
        store
            .claim_next_job("worker-2", "2026-05-27T21:39:30+08:00")
            .await
            .unwrap();

        let count = store
            .requeue_stale_processing_jobs("2026-05-27T21:35:00+08:00", "2026-05-27T21:45:00+08:00")
            .await
            .unwrap();

        assert_eq!(count, 1);
        let stale = sqlx::query(
            "SELECT status, next_run_at, locked_by, last_error FROM jobs WHERE id = ?1",
        )
        .bind(stale_job_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(stale.get::<String, _>("status"), "pending");
        assert_eq!(
            stale.get::<String, _>("next_run_at"),
            "2026-05-27T21:45:00+08:00"
        );
        assert_eq!(stale.get::<Option<String>, _>("locked_by"), None);
        assert_eq!(
            stale.get::<String, _>("last_error"),
            "requeued after processing timeout"
        );
        let fresh_status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = ?1")
            .bind(fresh_job_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(fresh_status, "processing");
    }
}
