use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::DefaultBodyLimit,
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
    wechat::{
        IncomingMessage, OpenId, OpenIdHash,
        crypto::{decrypt_callback_message, encrypt_reply_message, parse_encrypted_envelope},
        parse_plain_message, verify_signature,
    },
};

#[derive(Debug, Clone)]
pub struct ReceiverConfig {
    pub wechat_token: String,
    pub callback_path: String,
    pub encrypted_callback_enabled: bool,
    pub wechat_app_id: Option<String>,
    pub wechat_encoding_aes_key: Option<String>,
    pub honeypot_reply_enabled: bool,
    pub honeypot_reply_text: String,
    pub whitelist_join_command: Option<String>,
    pub request_body_limit_bytes: usize,
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
    signature: Option<String>,
    msg_signature: Option<String>,
    timestamp: String,
    nonce: String,
    encrypt_type: Option<String>,
}

pub fn router(state: ReceiverState) -> Router {
    let path = state.config.callback_path.clone();
    Router::new()
        .route(&path, get(verify_callback).post(receive_callback))
        .layer(DefaultBodyLimit::max(state.config.request_body_limit_bytes))
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
    match receive_message(&state, &query, &body).await {
        Ok(Some(reply_xml)) => reply_xml.into_response(),
        Ok(None) => "".into_response(),
        Err(BridgeError::WechatSignatureInvalid) => StatusCode::FORBIDDEN.into_response(),
        Err(err) => {
            tracing::warn!(component = "receiver", error = %err, "callback failed");
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

async fn receive_message(
    state: &ReceiverState,
    query: &CallbackQuery,
    body: &str,
) -> Result<Option<String>, BridgeError> {
    if state.config.encrypted_callback_enabled && query.encrypt_type.as_deref() == Some("aes") {
        return receive_encrypted_message(state, query, body).await;
    }
    receive_plain_message(state, query, body).await
}

async fn receive_plain_message(
    state: &ReceiverState,
    query: &CallbackQuery,
    body: &str,
) -> Result<Option<String>, BridgeError> {
    let signature = query
        .signature
        .as_deref()
        .ok_or(BridgeError::WechatSignatureInvalid)?;
    if !verify_signature(
        &state.config.wechat_token,
        &query.timestamp,
        &query.nonce,
        signature,
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

    process_plain_xml(state, &request_id, &raw_dir, body, None).await
}

async fn receive_encrypted_message(
    state: &ReceiverState,
    query: &CallbackQuery,
    body: &str,
) -> Result<Option<String>, BridgeError> {
    let app_id = state
        .config
        .wechat_app_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            BridgeError::Config("WECHAT_APP_ID is required for encrypted callback".to_string())
        })?;
    let encoding_aes_key = state
        .config
        .wechat_encoding_aes_key
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            BridgeError::Config(
                "WECHAT_ENCODING_AES_KEY is required for encrypted callback".to_string(),
            )
        })?;
    let msg_signature = query
        .msg_signature
        .as_deref()
        .ok_or(BridgeError::WechatSignatureInvalid)?;
    let request_id = request_id(&query.timestamp, &query.nonce);
    let raw_record =
        state
            .raw_archive
            .archive_bytes(&request_id, "callback.encrypted.xml", body.as_bytes())?;
    let raw_dir = raw_record
        .path
        .as_ref()
        .and_then(|path| path.parent())
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| format!("raw://{request_id}"));
    let envelope = parse_encrypted_envelope(body)?;
    let decrypted = decrypt_callback_message(
        &state.config.wechat_token,
        encoding_aes_key,
        app_id,
        &query.timestamp,
        &query.nonce,
        msg_signature,
        &envelope.encrypted_payload,
    )?;

    let reply =
        process_plain_xml(state, &request_id, &raw_dir, &decrypted.xml, Some("aes")).await?;
    if let Some(reply_xml) = reply {
        return Ok(Some(encrypt_reply_message(
            &state.config.wechat_token,
            encoding_aes_key,
            app_id,
            &query.timestamp,
            &query.nonce,
            &reply_xml,
        )?));
    }

    Ok(None)
}

async fn process_plain_xml(
    state: &ReceiverState,
    request_id: &str,
    raw_dir: &str,
    plain_xml: &str,
    encrypt_type: Option<&str>,
) -> Result<Option<String>, BridgeError> {
    let message = parse_plain_message(plain_xml)?;
    let common = message.common();
    let openid_hash = OpenIdHash::sha256_for_display(&common.from_user_name).to_string();
    let authorized = state
        .store
        .is_openid_whitelisted(common.from_user_name.as_str())
        .await?;
    let supported = message.is_supported();
    let whitelist_join_requested =
        is_whitelist_join_command(&message, state.config.whitelist_join_command.as_deref());

    let status = if whitelist_join_requested {
        "whitelisted"
    } else if authorized && supported {
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
            request_id: request_id.to_string(),
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
            media_id: media_id(&message),
            thumb_media_id: thumb_media_id(&message),
            pic_url: pic_url(&message),
            voice_format: voice_format(&message),
            voice_recognition: voice_recognition(&message),
            location_lat: location_lat(&message),
            location_lng: location_lng(&message),
            location_scale: location_scale(&message),
            location_label: location_label(&message),
            link_title: link_title(&message),
            link_description: link_description(&message),
            link_url: link_url(&message),
            authorized: authorized || whitelist_join_requested,
            status: status.to_string(),
            raw_dir: raw_dir.to_string(),
        })
        .await?;

    if whitelist_join_requested {
        let openid_hash = OpenIdHash::sha256_for_display(&common.from_user_name).to_string();
        state
            .store
            .upsert_whitelist(
                common.from_user_name.as_str(),
                &openid_hash,
                "wechat-magic-command",
            )
            .await?;
        tracing::info!(
            component = "receiver",
            message_id,
            openid_hash = %OpenIdHash::sha256_for_display(&common.from_user_name),
            encrypt_type = encrypt_type.unwrap_or("plain"),
            "openid added to whitelist by magic command"
        );
        return Ok(None);
    }

    if authorized && supported {
        state
            .store
            .create_job_once(message_id, "process_message", &received_at)
            .await?;
    }

    if !authorized && state.config.honeypot_reply_enabled {
        tracing::info!(
            component = "receiver",
            message_id,
            openid_hash = %OpenIdHash::sha256_for_display(&common.from_user_name),
            encrypt_type = encrypt_type.unwrap_or("plain"),
            "honeypot reply generated"
        );
        return Ok(Some(passive_text_reply(
            &common.from_user_name,
            &common.to_user_name,
            &state.config.honeypot_reply_text,
        )));
    }

    Ok(None)
}

fn passive_text_reply(to_user: &OpenId, from_user: &str, content: &str) -> String {
    format!(
        "<xml><ToUserName><![CDATA[{}]]></ToUserName><FromUserName><![CDATA[{}]]></FromUserName><CreateTime>{}</CreateTime><MsgType><![CDATA[text]]></MsgType><Content><![CDATA[{}]]></Content></xml>",
        to_user.as_str(),
        from_user,
        OffsetDateTime::now_utc().unix_timestamp(),
        sanitize_cdata(content)
    )
}

fn sanitize_cdata(value: &str) -> String {
    value.replace("]]>", "]]]]><![CDATA[>")
}

fn is_whitelist_join_command(message: &IncomingMessage, command: Option<&str>) -> bool {
    let Some(command) = command.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    matches!(
        message,
        IncomingMessage::Text(text) if text.content.trim() == command
    )
}

fn media_id(message: &IncomingMessage) -> Option<String> {
    match message {
        IncomingMessage::Image(message) => Some(message.media_id.as_str().to_string()),
        IncomingMessage::Voice(message) => Some(message.media_id.as_str().to_string()),
        IncomingMessage::Video(message) | IncomingMessage::ShortVideo(message) => {
            Some(message.media_id.as_str().to_string())
        }
        _ => None,
    }
}

fn thumb_media_id(message: &IncomingMessage) -> Option<String> {
    match message {
        IncomingMessage::Video(message) | IncomingMessage::ShortVideo(message) => message
            .thumb_media_id
            .as_ref()
            .map(|media_id| media_id.as_str().to_string()),
        _ => None,
    }
}

fn pic_url(message: &IncomingMessage) -> Option<String> {
    match message {
        IncomingMessage::Image(message) => message.pic_url.clone(),
        _ => None,
    }
}

fn voice_format(message: &IncomingMessage) -> Option<String> {
    match message {
        IncomingMessage::Voice(message) => message.format.clone(),
        _ => None,
    }
}

fn voice_recognition(message: &IncomingMessage) -> Option<String> {
    match message {
        IncomingMessage::Voice(message) => message.recognition.clone(),
        _ => None,
    }
}

fn location_lat(message: &IncomingMessage) -> Option<f64> {
    match message {
        IncomingMessage::Location(message) => Some(message.latitude),
        _ => None,
    }
}

fn location_lng(message: &IncomingMessage) -> Option<f64> {
    match message {
        IncomingMessage::Location(message) => Some(message.longitude),
        _ => None,
    }
}

fn location_scale(message: &IncomingMessage) -> Option<i32> {
    match message {
        IncomingMessage::Location(message) => message.scale,
        _ => None,
    }
}

fn location_label(message: &IncomingMessage) -> Option<String> {
    match message {
        IncomingMessage::Location(message) => message.label.clone(),
        _ => None,
    }
}

fn link_title(message: &IncomingMessage) -> Option<String> {
    match message {
        IncomingMessage::Link(message) => message.title.clone(),
        _ => None,
    }
}

fn link_description(message: &IncomingMessage) -> Option<String> {
    match message {
        IncomingMessage::Link(message) => message.description.clone(),
        _ => None,
    }
}

fn link_url(message: &IncomingMessage) -> Option<String> {
    match message {
        IncomingMessage::Link(message) => Some(message.url.as_str().to_string()),
        _ => None,
    }
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
        wechat::{
            OpenId, OpenIdHash,
            crypto::{
                decrypt_callback_message, encrypt_callback_message_for_test,
                parse_encrypted_envelope,
            },
            signature::{calculate_encrypted_signature, calculate_signature},
        },
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
                encrypted_callback_enabled: false,
                wechat_app_id: None,
                wechat_encoding_aes_key: None,
                honeypot_reply_enabled: false,
                honeypot_reply_text: "Message received.".to_string(),
                whitelist_join_command: None,
                request_body_limit_bytes: 2 * 1024 * 1024,
            },
            store,
            raw_archive: RawArchive::new(temp, raw_archive_full),
        }
    }

    fn signed_path(path: &str, token: &str, timestamp: &str, nonce: &str) -> String {
        let signature = calculate_signature(token, timestamp, nonce);
        format!("{path}?signature={signature}&timestamp={timestamp}&nonce={nonce}")
    }

    const TEST_AES_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const TEST_APP_ID: &str = "wx1234567890abcdef";

    fn encrypted_body_and_path(plain_xml: &str) -> (String, String) {
        let (encrypted_payload, signature) = encrypt_callback_message_for_test(
            "bridge-token",
            TEST_AES_KEY,
            TEST_APP_ID,
            "1780000000",
            "nonce1",
            plain_xml,
        )
        .unwrap();
        let body = format!("<xml><Encrypt><![CDATA[{encrypted_payload}]]></Encrypt></xml>");
        let uri = format!(
            "/wechat/callback?encrypt_type=aes&msg_signature={signature}&timestamp=1780000000&nonce=nonce1"
        );
        (body, uri)
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
    async fn post_callback_magic_command_whitelists_sender_without_job() {
        let mut state = test_state(false).await;
        state.config.whitelist_join_command = Some("/sage-wiki-join".to_string());
        let store = state.store.clone();
        let xml = include_str!("../../tests/fixtures/wechat/text.xml")
            .replace("openid-user-1", "openid-new-admin")
            .replace("把这条知识存进 sage-wiki", " /sage-wiki-join ");

        let status = post_fixture(state, &xml).await;

        assert_eq!(status, StatusCode::OK);
        assert!(
            store
                .is_openid_whitelisted("openid-new-admin")
                .await
                .unwrap()
        );
        let job_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(job_count, 0);
        let message_status: String = sqlx::query_scalar("SELECT status FROM messages LIMIT 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(message_status, "whitelisted");
        let authorized: bool = sqlx::query_scalar("SELECT authorized FROM messages LIMIT 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert!(authorized);
    }

    #[tokio::test]
    async fn post_callback_honeypot_replies_to_non_whitelisted_message() {
        let mut state = test_state(false).await;
        state.config.honeypot_reply_enabled = true;
        state.config.honeypot_reply_text = "收到".to_string();
        let app = router(state);
        let uri = signed_path("/wechat/callback", "bridge-token", "1780000000", "nonce1");
        let xml = include_str!("../../tests/fixtures/wechat/text.xml")
            .replace("openid-user-1", "openid-not-whitelisted");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .body(Body::from(xml))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let reply = String::from_utf8(body.to_vec()).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert!(reply.contains("<MsgType><![CDATA[text]]></MsgType>"));
        assert!(reply.contains("<Content><![CDATA[收到]]></Content>"));
        assert!(reply.contains("<ToUserName><![CDATA[openid-not-whitelisted]]></ToUserName>"));
    }

    #[tokio::test]
    async fn post_encrypted_callback_queues_whitelisted_text() {
        let mut state = test_state(true).await;
        state.config.encrypted_callback_enabled = true;
        state.config.wechat_app_id = Some(TEST_APP_ID.to_string());
        state.config.wechat_encoding_aes_key = Some(TEST_AES_KEY.to_string());
        let store = state.store.clone();
        let app = router(state);
        let (body, uri) =
            encrypted_body_and_path(include_str!("../../tests/fixtures/wechat/text.xml"));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let job_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(job_count, 1);
        let raw_dir: String = sqlx::query_scalar("SELECT raw_dir FROM messages LIMIT 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert!(raw_dir.contains("req_1780000000_nonce1"));
    }

    #[tokio::test]
    async fn post_encrypted_callback_encrypts_honeypot_reply() {
        let mut state = test_state(false).await;
        state.config.encrypted_callback_enabled = true;
        state.config.wechat_app_id = Some(TEST_APP_ID.to_string());
        state.config.wechat_encoding_aes_key = Some(TEST_AES_KEY.to_string());
        state.config.honeypot_reply_enabled = true;
        state.config.honeypot_reply_text = "收到".to_string();
        let app = router(state);
        let plain_xml = include_str!("../../tests/fixtures/wechat/text.xml")
            .replace("openid-user-1", "openid-not-whitelisted");
        let (body, uri) = encrypted_body_and_path(&plain_xml);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let encrypted_reply = String::from_utf8(body.to_vec()).unwrap();
        let envelope = parse_encrypted_envelope(&encrypted_reply).unwrap();
        let signature = calculate_encrypted_signature(
            "bridge-token",
            "1780000000",
            "nonce1",
            &envelope.encrypted_payload,
        );
        let decrypted = decrypt_callback_message(
            "bridge-token",
            TEST_AES_KEY,
            TEST_APP_ID,
            "1780000000",
            "nonce1",
            &signature,
            &envelope.encrypted_payload,
        )
        .unwrap();

        assert_eq!(status, StatusCode::OK);
        assert!(
            decrypted
                .xml
                .contains("<Content><![CDATA[收到]]></Content>")
        );
        assert!(
            decrypted
                .xml
                .contains("<ToUserName><![CDATA[openid-not-whitelisted]]></ToUserName>")
        );
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
