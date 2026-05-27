use std::{env, path::PathBuf, time::Duration};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub bind_addr: String,
    pub database_url: String,
    pub raw_archive_dir: PathBuf,
    pub raw_archive_full: bool,
    pub source_dir: PathBuf,
    pub callback_path: String,
    pub honeypot_reply_enabled: bool,
    pub honeypot_reply_text: String,
    pub worker_enabled: bool,
    pub worker_interval: Duration,
    pub http_timeout: Duration,
    pub wechat_api_base: String,
    pub max_media_bytes: u64,
    pub gemini_endpoint_base: String,
    pub gemini_model: String,
    pub gemini_max_inline_bytes: u64,
    pub tencent_lbs_endpoint: String,
    pub tencent_lbs_get_poi: bool,
    pub tencent_lbs_radius_meters: Option<u32>,
    pub jina_reader_endpoint: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, BridgeError> {
        Self::from_lookup(|key| env::var(key).ok())
    }

    fn from_lookup<F>(lookup: F) -> Result<Self, BridgeError>
    where
        F: Fn(&str) -> Option<String>,
    {
        Ok(Self {
            bind_addr: get_string(&lookup, "APP_BIND_ADDR", "127.0.0.1:8080"),
            database_url: get_string(&lookup, "DATABASE_URL", "sqlite://data/bridge.sqlite3"),
            raw_archive_dir: PathBuf::from(get_string(&lookup, "RAW_ARCHIVE_DIR", "data/raw")),
            raw_archive_full: get_bool(&lookup, "RAW_ARCHIVE_FULL", true)?,
            source_dir: PathBuf::from(get_string(&lookup, "SAGE_WIKI_SOURCE_DIR", "source")),
            callback_path: get_string(&lookup, "WECHAT_CALLBACK_PATH", "/wechat/callback"),
            honeypot_reply_enabled: get_bool(&lookup, "HONEYPOT_REPLY_ENABLED", false)?,
            honeypot_reply_text: get_string(&lookup, "HONEYPOT_REPLY_TEXT", "Message received."),
            worker_enabled: get_bool(&lookup, "WORKER_ENABLED", true)?,
            worker_interval: Duration::from_millis(get_u64(&lookup, "WORKER_INTERVAL_MS", 1000)?),
            http_timeout: Duration::from_secs(get_u64(&lookup, "HTTP_TIMEOUT_SECONDS", 30)?),
            wechat_api_base: get_string(&lookup, "WECHAT_API_BASE", "https://api.weixin.qq.com"),
            max_media_bytes: get_u64(&lookup, "MAX_MEDIA_BYTES", 20 * 1024 * 1024)?,
            gemini_endpoint_base: get_string(
                &lookup,
                "GEMINI_ENDPOINT_BASE",
                "https://generativelanguage.googleapis.com",
            ),
            gemini_model: get_string(&lookup, "GEMINI_MODEL", "gemini-2.5-flash"),
            gemini_max_inline_bytes: get_u64(&lookup, "GEMINI_MAX_INLINE_BYTES", 18 * 1024 * 1024)?,
            tencent_lbs_endpoint: get_string(
                &lookup,
                "TENCENT_LBS_ENDPOINT",
                "https://apis.map.qq.com/ws/geocoder/v1/",
            ),
            tencent_lbs_get_poi: get_bool(&lookup, "TENCENT_LBS_GET_POI", true)?,
            tencent_lbs_radius_meters: get_optional_u32(&lookup, "TENCENT_LBS_RADIUS_METERS")?,
            jina_reader_endpoint: get_string(&lookup, "JINA_READER_ENDPOINT", "https://r.jina.ai"),
        })
    }
}

fn get_string<F>(lookup: &F, key: &str, default: &str) -> String
where
    F: Fn(&str) -> Option<String>,
{
    lookup(key)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn get_bool<F>(lookup: &F, key: &str, default: bool) -> Result<bool, BridgeError>
where
    F: Fn(&str) -> Option<String>,
{
    let Some(value) = lookup(key).filter(|value| !value.trim().is_empty()) else {
        return Ok(default);
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(BridgeError::Config(format!("{key} must be a boolean"))),
    }
}

fn get_u64<F>(lookup: &F, key: &str, default: u64) -> Result<u64, BridgeError>
where
    F: Fn(&str) -> Option<String>,
{
    let Some(value) = lookup(key).filter(|value| !value.trim().is_empty()) else {
        return Ok(default);
    };
    value
        .parse::<u64>()
        .map_err(|_| BridgeError::Config(format!("{key} must be a positive integer")))
}

fn get_optional_u32<F>(lookup: &F, key: &str) -> Result<Option<u32>, BridgeError>
where
    F: Fn(&str) -> Option<String>,
{
    let Some(value) = lookup(key).filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    value
        .parse::<u32>()
        .map(Some)
        .map_err(|_| BridgeError::Config(format!("{key} must be a positive integer")))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn config_from_pairs(pairs: &[(&str, &str)]) -> Result<AppConfig, BridgeError> {
        let vars = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<HashMap<_, _>>();
        AppConfig::from_lookup(|key| vars.get(key).cloned())
    }

    #[test]
    fn app_config_uses_conservative_defaults() {
        let config = config_from_pairs(&[]).unwrap();

        assert_eq!(config.bind_addr, "127.0.0.1:8080");
        assert_eq!(config.database_url, "sqlite://data/bridge.sqlite3");
        assert_eq!(config.callback_path, "/wechat/callback");
        assert!(!config.honeypot_reply_enabled);
        assert_eq!(config.honeypot_reply_text, "Message received.");
        assert!(config.worker_enabled);
        assert_eq!(config.worker_interval, Duration::from_millis(1000));
        assert_eq!(config.source_dir, PathBuf::from("source"));
    }

    #[test]
    fn app_config_parses_overrides() {
        let config = config_from_pairs(&[
            ("APP_BIND_ADDR", "0.0.0.0:18080"),
            ("RAW_ARCHIVE_FULL", "false"),
            ("HONEYPOT_REPLY_ENABLED", "true"),
            ("HONEYPOT_REPLY_TEXT", "收到"),
            ("WORKER_INTERVAL_MS", "250"),
            ("TENCENT_LBS_RADIUS_METERS", "500"),
        ])
        .unwrap();

        assert_eq!(config.bind_addr, "0.0.0.0:18080");
        assert!(!config.raw_archive_full);
        assert!(config.honeypot_reply_enabled);
        assert_eq!(config.honeypot_reply_text, "收到");
        assert_eq!(config.worker_interval, Duration::from_millis(250));
        assert_eq!(config.tencent_lbs_radius_meters, Some(500));
    }

    #[test]
    fn app_config_rejects_bad_bool() {
        let err = config_from_pairs(&[("WORKER_ENABLED", "maybe")]).unwrap_err();

        assert!(matches!(err, BridgeError::Config(_)));
    }
}
