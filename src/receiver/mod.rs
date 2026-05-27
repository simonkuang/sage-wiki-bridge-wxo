use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    archive::RawArchive,
    error::BridgeError,
    store::{MessageInsert, Store},
    wechat::{OpenIdHash, parse_plain_message, verify_signature},
};

#[derive(Debug, Clone)]
pub struct ReceiverConfig {
    pub wechat_token: String,
    pub callback_path: String,
}

#[derive(Debug, Clone)]
pub struct ReceiverState {
    pub config: ReceiverConfig,
    pub store: Store,
    pub raw_archive: RawArchive,
}

#[derive(Debug, Deserialize)]
pub struct VerifyQuery {
    signature: String,
    timestamp: String,
    nonce: String,
    echostr: String,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    signature: String,
    timestamp: String,
    nonce: String,
}

pub fn router(state: ReceiverState) -> Router {
    let path = state.config.callback_path.clone();
    Router::new()
        .route(&path, get(verify_callback).post(receive_callback))
        .with_state(Arc::new(state))
}

async fn verify_callback(
    State(state): State<Arc<ReceiverState>>,
    Query(query): Query<VerifyQuery>,
) -> Response {
    if verify_signature(
        &state.config.wechat_token,
        &query.timestamp,
        &query.nonce,
        &query.signature,
    ) {
        query.echostr.into_response()
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}

async fn receive_callback(
    State(state): State<Arc<ReceiverState>>,
    Query(query): Query<CallbackQuery>,
    body: String,
) -> Response {
    match receive_plain_message(&state, &query, &body).await {
        Ok(()) => "".into_response(),
        Err(BridgeError::WechatSignatureInvalid) => StatusCode::FORBIDDEN.into_response(),
        Err(err) => {
            tracing::warn!(component = "receiver", error = %err, "callback failed");
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

async fn receive_plain_message(
    state: &ReceiverState,
    query: &CallbackQuery,
    body: &str,
) -> Result<(), BridgeError> {
    if !verify_signature(
        &state.config.wechat_token,
        &query.timestamp,
        &query.nonce,
        &query.signature,
    ) {
        return Err(BridgeError::WechatSignatureInvalid);
    }

    let request_id = request_id(&query.timestamp, &query.nonce);
    let raw_record =
        state
            .raw_archive
            .archive_bytes(&request_id, "callback.xml", body.as_bytes())?;
    let raw_dir = raw_record
        .path
        .as_ref()
        .and_then(|path| path.parent())
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| format!("raw://{request_id}"));

    let message = parse_plain_message(body)?;
    let common = message.common();
    let openid_hash = OpenIdHash::sha256_for_display(&common.from_user_name).to_string();
    let authorized = state
        .store
        .is_openid_whitelisted(common.from_user_name.as_str())
        .await?;
    let supported = message.is_supported();

    let status = if authorized && supported {
        "queued"
    } else {
        "ignored"
    };
    let received_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());

    let message_id = state
        .store
        .insert_message_idempotent(&MessageInsert {
            request_id,
            wechat_msg_id: common
                .msg_id
                .as_ref()
                .map(|msg_id| msg_id.as_str().to_string()),
            to_user_name: common.to_user_name.clone(),
            from_openid: common.from_user_name.as_str().to_string(),
            from_openid_hash: openid_hash,
            create_time: Some(common.create_time),
            received_at: received_at.clone(),
            message_type: message.msg_type().to_string(),
            content_text: message.content_text().map(ToOwned::to_owned),
            authorized,
            status: status.to_string(),
            raw_dir,
        })
        .await?;

    if authorized && supported {
        state
            .store
            .create_job_once(message_id, "process_message", &received_at)
            .await?;
    }

    Ok(())
}

fn request_id(timestamp: &str, nonce: &str) -> String {
    format!("req_{timestamp}_{nonce}")
}

#[allow(dead_code)]
fn _assert_body_send_sync(_: Body) {}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    use crate::{
        archive::RawArchive,
        store::Store,
        wechat::{OpenId, OpenIdHash, signature::calculate_signature},
    };

    use super::*;

    async fn test_state(raw_archive_full: bool) -> ReceiverState {
        let store = Store::connect("sqlite::memory:").await.unwrap();
        store.migrate().await.unwrap();
        store
            .upsert_whitelist(
                "openid-user-1",
                &OpenIdHash::sha256_for_display(&OpenId::new("openid-user-1")).to_string(),
                "test",
            )
            .await
            .unwrap();
        let temp = tempfile::tempdir().unwrap().keep();

        ReceiverState {
            config: ReceiverConfig {
                wechat_token: "bridge-token".to_string(),
                callback_path: "/wechat/callback".to_string(),
            },
            store,
            raw_archive: RawArchive::new(temp, raw_archive_full),
        }
    }

    fn signed_path(path: &str, token: &str, timestamp: &str, nonce: &str) -> String {
        let signature = calculate_signature(token, timestamp, nonce);
        format!("{path}?signature={signature}&timestamp={timestamp}&nonce={nonce}")
    }

    async fn post_fixture(state: ReceiverState, fixture: &str) -> StatusCode {
        let app = router(state);
        let uri = signed_path("/wechat/callback", "bridge-token", "1780000000", "nonce1");
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .body(Body::from(fixture.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        response.status()
    }

    #[tokio::test]
    async fn get_callback_returns_echostr_when_signature_valid() {
        let state = test_state(false).await;
        let app = router(state);
        let signature = calculate_signature("bridge-token", "1780000000", "nonce1");
        let uri = format!(
            "/wechat/callback?signature={signature}&timestamp=1780000000&nonce=nonce1&echostr=hello"
        );

        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024).await.unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(&body[..], b"hello");
    }

    #[tokio::test]
    async fn get_callback_rejects_bad_signature() {
        let state = test_state(false).await;
        let app = router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/wechat/callback?signature=bad&timestamp=1780000000&nonce=nonce1&echostr=hello")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn post_callback_queues_whitelisted_text() {
        let state = test_state(true).await;
        let store = state.store.clone();

        let status =
            post_fixture(state, include_str!("../../tests/fixtures/wechat/text.xml")).await;

        assert_eq!(status, StatusCode::OK);
        let job_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(job_count, 1);
    }

    #[tokio::test]
    async fn post_callback_accepts_all_supported_fixture_types() {
        for fixture in [
            include_str!("../../tests/fixtures/wechat/text.xml"),
            include_str!("../../tests/fixtures/wechat/image.xml"),
            include_str!("../../tests/fixtures/wechat/voice.xml"),
            include_str!("../../tests/fixtures/wechat/video.xml"),
            include_str!("../../tests/fixtures/wechat/shortvideo.xml"),
            include_str!("../../tests/fixtures/wechat/location.xml"),
            include_str!("../../tests/fixtures/wechat/link.xml"),
        ] {
            let status = post_fixture(test_state(false).await, fixture).await;
            assert_eq!(status, StatusCode::OK);
        }
    }

    #[tokio::test]
    async fn post_callback_ignores_non_whitelisted_message() {
        let state = test_state(false).await;
        let store = state.store.clone();
        let xml = include_str!("../../tests/fixtures/wechat/text.xml")
            .replace("openid-user-1", "openid-not-whitelisted");

        let status = post_fixture(state, &xml).await;

        assert_eq!(status, StatusCode::OK);
        let job_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(job_count, 0);
        let message_status: String = sqlx::query_scalar("SELECT status FROM messages LIMIT 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(message_status, "ignored");
    }

    #[tokio::test]
    async fn post_callback_rejects_bad_signature() {
        let state = test_state(false).await;
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/wechat/callback?signature=bad&timestamp=1780000000&nonce=nonce1")
                    .body(Body::from(include_str!(
                        "../../tests/fixtures/wechat/text.xml"
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
