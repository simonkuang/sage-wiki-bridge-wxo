use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use url::Url;

use crate::error::BridgeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WechatOAuthConfig {
    pub app_id: String,
    pub app_secret: String,
    pub api_base: String,
    pub authorize_base: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthSubject {
    pub openid: String,
    pub unionid: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WechatOAuthClient {
    client: Client,
    config: WechatOAuthConfig,
}

impl WechatOAuthClient {
    pub fn new(config: WechatOAuthConfig, timeout: Duration) -> Result<Self, BridgeError> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|err| BridgeError::Config(err.to_string()))?;
        Ok(Self { client, config })
    }

    pub fn authorize_url(&self, redirect_uri: &str, state: &str) -> Result<Url, BridgeError> {
        let mut url = Url::parse(&self.config.authorize_base)
            .map_err(|err| BridgeError::Config(err.to_string()))?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("appid", &self.config.app_id);
            query.append_pair("redirect_uri", redirect_uri);
            query.append_pair("response_type", "code");
            query.append_pair("scope", "snsapi_base");
            query.append_pair("state", state);
        }
        url.set_fragment(Some("wechat_redirect"));
        Ok(url)
    }

    pub async fn exchange_code(&self, code: &str) -> Result<OAuthSubject, BridgeError> {
        let url = format!(
            "{}/sns/oauth2/access_token",
            self.config.api_base.trim_end_matches('/')
        );
        let response = self
            .client
            .get(url)
            .query(&[
                ("appid", self.config.app_id.as_str()),
                ("secret", self.config.app_secret.as_str()),
                ("code", code),
                ("grant_type", "authorization_code"),
            ])
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
                "WeChat OAuth returned {status}: {body}"
            )));
        }
        let payload: OAuthAccessTokenResponse = serde_json::from_str(&body)
            .map_err(|err| BridgeError::ExternalPayloadInvalid(err.to_string()))?;
        if let Some(errcode) = payload.errcode {
            return Err(BridgeError::ExternalRequest(format!(
                "WeChat OAuth errcode {errcode}: {}",
                payload.errmsg.unwrap_or_default()
            )));
        }
        let openid = payload.openid.ok_or_else(|| {
            BridgeError::ExternalPayloadInvalid("WeChat OAuth response missing openid".to_string())
        })?;
        Ok(OAuthSubject {
            openid,
            unionid: payload.unionid,
            scope: payload.scope,
        })
    }
}

#[derive(Debug, Deserialize)]
struct OAuthAccessTokenResponse {
    openid: Option<String>,
    unionid: Option<String>,
    scope: Option<String>,
    errcode: Option<i64>,
    errmsg: Option<String>,
}

#[cfg(test)]
mod tests {
    use axum::{Json, Router, response::IntoResponse, routing::get};
    use serde_json::json;
    use tokio::net::TcpListener;

    use super::*;

    async fn spawn_mock_oauth() -> String {
        async fn handler() -> impl IntoResponse {
            Json(json!({
                "access_token": "oauth-token",
                "expires_in": 7200,
                "refresh_token": "refresh-token",
                "openid": "openid-oauth-1",
                "scope": "snsapi_base",
                "unionid": "unionid-1"
            }))
        }
        let app = Router::new().route("/{*path}", get(handler));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[test]
    fn builds_snsapi_base_authorize_url() {
        let client = WechatOAuthClient::new(
            WechatOAuthConfig {
                app_id: "wx-app-id".to_string(),
                app_secret: "secret".to_string(),
                api_base: "https://api.weixin.qq.com".to_string(),
                authorize_base: "https://open.weixin.qq.com/connect/oauth2/authorize".to_string(),
            },
            Duration::from_secs(5),
        )
        .unwrap();

        let url = client
            .authorize_url(
                "https://bridge.example.com/admin/whitelist/join?key=join",
                "join",
            )
            .unwrap();

        assert_eq!(url.fragment(), Some("wechat_redirect"));
        assert!(url.as_str().contains("scope=snsapi_base"));
        assert!(url.as_str().contains("appid=wx-app-id"));
    }

    #[tokio::test]
    async fn exchanges_code_for_openid() {
        let api_base = spawn_mock_oauth().await;
        let client = WechatOAuthClient::new(
            WechatOAuthConfig {
                app_id: "wx-app-id".to_string(),
                app_secret: "secret".to_string(),
                api_base,
                authorize_base: "https://open.weixin.qq.com/connect/oauth2/authorize".to_string(),
            },
            Duration::from_secs(5),
        )
        .unwrap();

        let subject = client.exchange_code("code-1").await.unwrap();

        assert_eq!(subject.openid, "openid-oauth-1");
        assert_eq!(subject.unionid.as_deref(), Some("unionid-1"));
    }
}
