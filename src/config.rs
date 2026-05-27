use std::env;

use crate::error::BridgeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvSecrets {
    pub wechat_app_id: Option<String>,
    pub wechat_app_secret: Option<String>,
    pub wechat_token: Option<String>,
    pub wechat_encoding_aes_key: Option<String>,
    pub admin_view_key: Option<String>,
    pub whitelist_join_key: Option<String>,
    pub gemini_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub tencent_lbs_key: Option<String>,
    pub jina_api_key: Option<String>,
}

impl EnvSecrets {
    pub fn from_env() -> Self {
        Self {
            wechat_app_id: env::var("WECHAT_APP_ID").ok(),
            wechat_app_secret: env::var("WECHAT_APP_SECRET").ok(),
            wechat_token: env::var("WECHAT_TOKEN").ok(),
            wechat_encoding_aes_key: env::var("WECHAT_ENCODING_AES_KEY").ok(),
            admin_view_key: env::var("ADMIN_VIEW_KEY").ok(),
            whitelist_join_key: env::var("WHITELIST_JOIN_KEY").ok(),
            gemini_api_key: env::var("GEMINI_API_KEY").ok(),
            openai_api_key: env::var("OPENAI_API_KEY").ok(),
            anthropic_api_key: env::var("ANTHROPIC_API_KEY").ok(),
            tencent_lbs_key: env::var("TENCENT_LBS_KEY").ok(),
            jina_api_key: env::var("JINA_API_KEY").ok(),
        }
    }

    pub fn require_wechat_token(&self) -> Result<&str, BridgeError> {
        self.wechat_token
            .as_deref()
            .filter(|token| !token.is_empty())
            .ok_or_else(|| BridgeError::Config("WECHAT_TOKEN is required".to_string()))
    }
}
