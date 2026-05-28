use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;

use crate::{
    error::BridgeError,
    store::{MessageDetail, MessageListQuery, Store},
    wechat::{OpenId, OpenIdHash, oauth::WechatOAuthClient},
};

#[derive(Debug, Clone)]
pub struct AdminState {
    pub store: Store,
    pub base_path: String,
    pub view_key: Option<String>,
    pub whitelist_join_key: Option<String>,
    pub whitelist_join_redirect_url: Option<String>,
    pub oauth_client: Option<WechatOAuthClient>,
    pub default_per_page: u32,
    pub max_per_page: u32,
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    key: Option<String>,
    page: Option<u32>,
    per_page: Option<u32>,
    sort: Option<String>,
    q: Option<String>,
    msg_type: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DetailQuery {
    key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JoinQuery {
    key: Option<String>,
    openid: Option<String>,
    code: Option<String>,
}

pub fn router(state: AdminState) -> Router {
    let messages_path = format!("{}/messages", state.base_path);
    let message_detail_path = format!("{}/messages/{{id}}", state.base_path);
    let whitelist_join_path = format!("{}/whitelist/join", state.base_path);
    Router::new()
        .route(&messages_path, get(list_messages))
        .route(&message_detail_path, get(message_detail))
        .route(&whitelist_join_path, get(join_whitelist))
        .with_state(Arc::new(state))
}

async fn list_messages(
    State(state): State<Arc<AdminState>>,
    Query(query): Query<ListQuery>,
) -> Response {
    if !authorized(query.key.as_deref(), state.view_key.as_deref()) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query
        .per_page
        .unwrap_or(state.default_per_page)
        .clamp(1, state.max_per_page.max(1));
    let sort_desc = !matches!(query.sort.as_deref(), Some("received_at_asc"));
    let keyword = query.q.as_ref().map(|value| value.trim().to_string());
    let message_type = query
        .msg_type
        .as_ref()
        .map(|value| value.trim().to_string());
    let status = query.status.as_ref().map(|value| value.trim().to_string());

    match state
        .store
        .list_messages(&MessageListQuery {
            page,
            per_page,
            keyword: keyword.clone().filter(|value| !value.is_empty()),
            message_type: message_type.clone().filter(|value| !value.is_empty()),
            status: status.clone().filter(|value| !value.is_empty()),
            sort_desc,
        })
        .await
    {
        Ok(result) => Html(render_list_page(
            &query.key.unwrap_or_default(),
            &state.base_path,
            keyword.as_deref().unwrap_or(""),
            message_type.as_deref().unwrap_or(""),
            status.as_deref().unwrap_or(""),
            sort_desc,
            &result,
        ))
        .into_response(),
        Err(err) => error_response(err),
    }
}

async fn message_detail(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<i64>,
    Query(query): Query<DetailQuery>,
) -> Response {
    if !authorized(query.key.as_deref(), state.view_key.as_deref()) {
        return StatusCode::FORBIDDEN.into_response();
    }

    match state.store.get_message_detail(id).await {
        Ok(detail) => Html(render_detail_page(
            &query.key.unwrap_or_default(),
            &state.base_path,
            &detail,
        ))
        .into_response(),
        Err(err) => error_response(err),
    }
}

async fn join_whitelist(
    State(state): State<Arc<AdminState>>,
    Query(query): Query<JoinQuery>,
) -> Response {
    if !authorized(query.key.as_deref(), state.whitelist_join_key.as_deref()) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let openid = if let Some(openid) = query.openid.as_deref().filter(|value| !value.is_empty()) {
        openid.to_string()
    } else if let Some(code) = query.code.as_deref().filter(|value| !value.is_empty()) {
        let Some(oauth_client) = &state.oauth_client else {
            return error_response(BridgeError::Config(
                "WeChat OAuth client not configured".to_string(),
            ));
        };
        match oauth_client.exchange_code(code).await {
            Ok(subject) => subject.openid,
            Err(err) => return error_response(err),
        }
    } else {
        let auth_url = match (&state.oauth_client, &state.whitelist_join_redirect_url) {
            (Some(oauth_client), Some(redirect_url)) => oauth_client
                .authorize_url(redirect_url, "whitelist_join")
                .ok()
                .map(|url| url.to_string()),
            _ => None,
        };
        return Html(render_join_form(
            &query.key.unwrap_or_default(),
            auth_url.as_deref(),
        ))
        .into_response();
    };
    let openid = OpenId::new(openid);
    let openid_hash = OpenIdHash::sha256_for_display(&openid).to_string();

    match state
        .store
        .upsert_whitelist(openid.as_str(), &openid_hash, "admin-join-stub")
        .await
    {
        Ok(()) => Html(format!(
            "<!doctype html><meta charset=\"utf-8\"><title>Whitelisted</title>\
             <h1>Whitelisted</h1><dl><dt>openid_hash</dt><dd>{}</dd></dl>",
            escape_html(&openid_hash)
        ))
        .into_response(),
        Err(err) => error_response(err),
    }
}

fn authorized(provided: Option<&str>, expected: Option<&str>) -> bool {
    let Some(expected) = expected.filter(|value| !value.is_empty()) else {
        return false;
    };
    provided == Some(expected)
}

fn render_list_page(
    key: &str,
    base_path: &str,
    keyword: &str,
    message_type: &str,
    status: &str,
    sort_desc: bool,
    page: &crate::store::MessageListPage,
) -> String {
    let sort = if sort_desc {
        "received_at_desc"
    } else {
        "received_at_asc"
    };
    let previous = page.page.saturating_sub(1).max(1);
    let next = page.page + 1;
    let rows = page
        .items
        .iter()
        .map(|item| {
            format!(
                "<tr><td><a href=\"{}/messages/{}?key={}\">{}</a></td>\
                 <td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_attr(base_path),
                item.id,
                escape_attr(key),
                item.id,
                escape_html(&item.received_at),
                escape_html(&item.message_type),
                escape_html(&item.status),
                escape_html(&item.from_openid_hash),
                escape_html(item.content_preview.as_deref().unwrap_or("")),
                escape_html(item.processed_preview.as_deref().unwrap_or(""))
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Messages</title>{style}\
         <h1>Messages</h1>\
         <form method=\"get\"><input type=\"hidden\" name=\"key\" value=\"{}\">\
         <input name=\"q\" value=\"{}\" placeholder=\"keyword\">\
         <input name=\"msg_type\" value=\"{}\" placeholder=\"type\">\
         <input name=\"status\" value=\"{}\" placeholder=\"status\">\
         <select name=\"sort\"><option value=\"received_at_desc\" {}>newest</option>\
         <option value=\"received_at_asc\" {}>oldest</option></select>\
         <button type=\"submit\">Search</button></form>\
         <p>Total: {} | Page: {}</p>\
         <table><thead><tr><th>ID</th><th>Received</th><th>Type</th><th>Status</th>\
         <th>OpenID Hash</th><th>Original</th><th>Processed</th></tr></thead><tbody>{rows}</tbody></table>\
         <nav><a href=\"{}/messages?key={}&q={}&msg_type={}&status={}&sort={sort}&page={previous}&per_page={}\">Previous</a>\
         <a href=\"{}/messages?key={}&q={}&msg_type={}&status={}&sort={sort}&page={next}&per_page={}\">Next</a></nav>",
        escape_attr(key),
        escape_attr(keyword),
        escape_attr(message_type),
        escape_attr(status),
        if sort_desc { "selected" } else { "" },
        if sort_desc { "" } else { "selected" },
        page.total,
        page.page,
        escape_attr(base_path),
        escape_attr(key),
        escape_attr(keyword),
        escape_attr(message_type),
        escape_attr(status),
        page.per_page,
        escape_attr(base_path),
        escape_attr(key),
        escape_attr(keyword),
        escape_attr(message_type),
        escape_attr(status),
        page.per_page,
        style = STYLE
    )
}

fn render_detail_page(key: &str, base_path: &str, detail: &MessageDetail) -> String {
    let fields = [
        ("id", detail.id.to_string()),
        ("request_id", detail.request_id.clone()),
        (
            "wechat_msg_id",
            detail.wechat_msg_id.clone().unwrap_or_default(),
        ),
        ("from_openid_hash", detail.from_openid_hash.clone()),
        ("received_at", detail.received_at.clone()),
        ("message_type", detail.message_type.clone()),
        ("status", detail.status.clone()),
        ("raw_dir", detail.raw_dir.clone()),
        (
            "source_path",
            detail.source_path.clone().unwrap_or_default(),
        ),
        (
            "processed_at",
            detail.processed_at.clone().unwrap_or_default(),
        ),
        ("media_id", detail.media_id.clone().unwrap_or_default()),
        ("pic_url", detail.pic_url.clone().unwrap_or_default()),
        ("link_url", detail.link_url.clone().unwrap_or_default()),
        (
            "location",
            match (detail.location_lat, detail.location_lng) {
                (Some(lat), Some(lng)) => format!("{lat},{lng}"),
                _ => String::new(),
            },
        ),
    ]
    .into_iter()
    .map(|(key, value)| {
        format!(
            "<dt>{}</dt><dd>{}</dd>",
            escape_html(key),
            escape_html(&value)
        )
    })
    .collect::<Vec<_>>()
    .join("");

    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Message {}</title>{style}\
         <a href=\"{}/messages?key={}\">Back</a><h1>Message {}</h1><dl>{fields}</dl>\
         <h2>Original Text</h2><pre>{}</pre><h2>Processed Text</h2><pre>{}</pre>",
        detail.id,
        escape_attr(base_path),
        escape_attr(key),
        detail.id,
        escape_html(detail.content_text.as_deref().unwrap_or("")),
        escape_html(detail.processed_text.as_deref().unwrap_or("")),
        style = STYLE
    )
}

fn render_join_form(key: &str, auth_url: Option<&str>) -> String {
    let oauth_link = auth_url
        .map(|url| {
            format!(
                "<p><a href=\"{}\">Authorize with WeChat OAuth</a></p>",
                escape_attr(url)
            )
        })
        .unwrap_or_default();
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Join Whitelist</title>{style}\
         <h1>Join Whitelist</h1><form method=\"get\">\
         <input type=\"hidden\" name=\"key\" value=\"{}\">\
         <input name=\"openid\" placeholder=\"openid\"><button type=\"submit\">Join</button></form>{oauth_link}",
        escape_attr(key),
        style = STYLE
    )
}

fn error_response(err: BridgeError) -> Response {
    tracing::warn!(component = "admin", error = %err, "admin request failed");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_attr(value: &str) -> String {
    escape_html(value)
}

const STYLE: &str = r#"<style>
body{font-family:system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;margin:24px;color:#1f2933}
table{border-collapse:collapse;width:100%;font-size:14px}
th,td{border:1px solid #d8dee4;padding:8px;text-align:left;vertical-align:top}
th{background:#f6f8fa}
input,select,button{font:inherit;padding:6px;margin-right:6px}
nav{display:flex;gap:12px;margin-top:16px}
pre{white-space:pre-wrap;border:1px solid #d8dee4;padding:12px;background:#f6f8fa}
dd{margin:0 0 8px 0}
dt{font-weight:700}
</style>"#;

#[cfg(test)]
mod tests {
    use axum::{
        Json, Router as AxumRouter,
        body::{Body, to_bytes},
        http::{Request, StatusCode},
        response::IntoResponse,
        routing::get,
    };
    use serde_json::json;
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    use crate::{
        store::{MessageInsert, Store},
        wechat::oauth::{WechatOAuthClient, WechatOAuthConfig},
    };

    use super::*;

    async fn test_store() -> Store {
        let store = Store::connect("sqlite::memory:").await.unwrap();
        store.migrate().await.unwrap();
        store
            .insert_message_idempotent(&MessageInsert {
                request_id: "req_1".to_string(),
                wechat_msg_id: Some("msg_1".to_string()),
                to_user_name: "gh_bridge".to_string(),
                from_openid: "openid-user-1".to_string(),
                from_openid_hash: "sha256:abc".to_string(),
                create_time: Some(1780000001),
                received_at: "2026-05-27T21:30:15+08:00".to_string(),
                message_type: "text".to_string(),
                content_text: Some("hello list".to_string()),
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
                raw_dir: "data/raw/req_1".to_string(),
            })
            .await
            .unwrap();
        store
    }

    fn admin_state(store: Store) -> AdminState {
        AdminState {
            store,
            base_path: "/admin".to_string(),
            view_key: Some("view-key".to_string()),
            whitelist_join_key: Some("join-key".to_string()),
            whitelist_join_redirect_url: None,
            oauth_client: None,
            default_per_page: 20,
            max_per_page: 100,
        }
    }

    async fn spawn_mock_oauth() -> String {
        async fn handler() -> impl IntoResponse {
            Json(json!({
                "access_token": "oauth-token",
                "expires_in": 7200,
                "openid": "openid-from-code",
                "scope": "snsapi_base"
            }))
        }
        let app = AxumRouter::new().route("/{*path}", get(handler));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn admin_list_requires_key() {
        let app = router(admin_state(test_store().await));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/messages")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn admin_list_renders_messages() {
        let app = router(admin_state(test_store().await));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/messages?key=view-key&q=hello")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert!(html.contains("hello list"));
        assert!(html.contains("/admin/messages/1?key=view-key"));
    }

    #[tokio::test]
    async fn whitelist_join_stub_adds_openid() {
        let store = test_store().await;
        let app = router(admin_state(store.clone()));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/whitelist/join?key=join-key&openid=openid-new")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(store.is_openid_whitelisted("openid-new").await.unwrap());
    }

    #[tokio::test]
    async fn whitelist_join_exchanges_oauth_code() {
        let store = test_store().await;
        let api_base = spawn_mock_oauth().await;
        let app = router(AdminState {
            store: store.clone(),
            base_path: "/admin".to_string(),
            view_key: Some("view-key".to_string()),
            whitelist_join_key: Some("join-key".to_string()),
            whitelist_join_redirect_url: Some(
                "https://bridge.example.com/admin/whitelist/join?key=join-key".to_string(),
            ),
            oauth_client: Some(
                WechatOAuthClient::new(
                    WechatOAuthConfig {
                        app_id: "wx-app-id".to_string(),
                        app_secret: "secret".to_string(),
                        api_base,
                        authorize_base: "https://open.weixin.qq.com/connect/oauth2/authorize"
                            .to_string(),
                    },
                    std::time::Duration::from_secs(5),
                )
                .unwrap(),
            ),
            default_per_page: 20,
            max_per_page: 100,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/whitelist/join?key=join-key&code=oauth-code")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            store
                .is_openid_whitelisted("openid-from-code")
                .await
                .unwrap()
        );
    }
}
