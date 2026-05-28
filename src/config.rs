use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::error::BridgeError;

const FLAG_SPECS: &[(&str, &str)] = &[
    ("--bind-addr", "APP_BIND_ADDR"),
    ("--database-url", "DATABASE_URL"),
    ("--raw-archive-dir", "RAW_ARCHIVE_DIR"),
    ("--raw-archive-full", "RAW_ARCHIVE_FULL"),
    ("--processed-artifact-dir", "PROCESSED_ARTIFACT_DIR"),
    ("--sage-wiki-source-dir", "SAGE_WIKI_SOURCE_DIR"),
    ("--wechat-callback-path", "WECHAT_CALLBACK_PATH"),
    (
        "--wechat-encrypted-callback-enabled",
        "WECHAT_ENCRYPTED_CALLBACK_ENABLED",
    ),
    ("--honeypot-reply-enabled", "HONEYPOT_REPLY_ENABLED"),
    ("--honeypot-reply-text", "HONEYPOT_REPLY_TEXT"),
    ("--worker-enabled", "WORKER_ENABLED"),
    ("--worker-interval-ms", "WORKER_INTERVAL_MS"),
    (
        "--worker-processing-timeout-seconds",
        "WORKER_PROCESSING_TIMEOUT_SECONDS",
    ),
    ("--http-timeout-seconds", "HTTP_TIMEOUT_SECONDS"),
    ("--wechat-api-base", "WECHAT_API_BASE"),
    (
        "--wechat-oauth-authorize-base",
        "WECHAT_OAUTH_AUTHORIZE_BASE",
    ),
    ("--max-media-bytes", "MAX_MEDIA_BYTES"),
    (
        "--whitelist-join-redirect-url",
        "WHITELIST_JOIN_REDIRECT_URL",
    ),
    ("--gemini-endpoint-base", "GEMINI_ENDPOINT_BASE"),
    ("--gemini-model", "GEMINI_MODEL"),
    ("--gemini-max-inline-bytes", "GEMINI_MAX_INLINE_BYTES"),
    ("--llm-image-system-prompt", "LLM_IMAGE_SYSTEM_PROMPT"),
    ("--llm-voice-system-prompt", "LLM_VOICE_SYSTEM_PROMPT"),
    ("--llm-video-system-prompt", "LLM_VIDEO_SYSTEM_PROMPT"),
    ("--tencent-lbs-endpoint", "TENCENT_LBS_ENDPOINT"),
    ("--tencent-lbs-get-poi", "TENCENT_LBS_GET_POI"),
    ("--tencent-lbs-radius-meters", "TENCENT_LBS_RADIUS_METERS"),
    ("--jina-reader-endpoint", "JINA_READER_ENDPOINT"),
    ("--wechat-app-id", "WECHAT_APP_ID"),
    ("--wechat-appid", "WECHAT_APP_ID"),
    ("--wechat-app-secret", "WECHAT_APP_SECRET"),
    ("--wechat-appsecret", "WECHAT_APP_SECRET"),
    ("--wechat-token", "WECHAT_TOKEN"),
    ("--wechat-encoding-aes-key", "WECHAT_ENCODING_AES_KEY"),
    ("--admin-view-key", "ADMIN_VIEW_KEY"),
    ("--whitelist-join-key", "WHITELIST_JOIN_KEY"),
    ("--gemini-api-key", "GEMINI_API_KEY"),
    ("--openai-api-key", "OPENAI_API_KEY"),
    ("--anthropic-api-key", "ANTHROPIC_API_KEY"),
    ("--tencent-lbs-key", "TENCENT_LBS_KEY"),
    ("--jina-api-key", "JINA_API_KEY"),
    ("--wechat-admin-openids", "WECHAT_ADMIN_OPENIDS"),
    ("--rust-log", "RUST_LOG"),
];

const HELP: &str = r#"sage-wiki-bridge

Usage:
  sage-wiki-bridge [OPTIONS]

Configuration sources are explicit and ordered:
  CLI flags > --env-file PATH > --use-process-env > built-in defaults.

Source controls:
  --env-file PATH                  Load dotenv-style config from PATH.
  --use-process-env                Read process environment variables.
  --help                           Print this help.

Core:
  --rust-log VALUE
  --bind-addr VALUE
  --database-url VALUE
  --raw-archive-dir VALUE
  --raw-archive-full true|false
  --processed-artifact-dir VALUE
  --sage-wiki-source-dir VALUE
  --http-timeout-seconds VALUE

WeChat:
  --wechat-token VALUE
  --wechat-app-id VALUE
  --wechat-app-secret VALUE
  --wechat-encoding-aes-key VALUE
  --wechat-callback-path VALUE
  --wechat-encrypted-callback-enabled true|false
  --wechat-api-base VALUE
  --wechat-oauth-authorize-base VALUE
  --wechat-admin-openids VALUE

Admin:
  --admin-view-key VALUE
  --whitelist-join-key VALUE
  --whitelist-join-redirect-url VALUE
  --honeypot-reply-enabled true|false
  --honeypot-reply-text VALUE

Worker and external services:
  --worker-enabled true|false
  --worker-interval-ms VALUE
  --worker-processing-timeout-seconds VALUE
  --max-media-bytes VALUE
  --gemini-api-key VALUE
  --gemini-endpoint-base VALUE
  --gemini-model VALUE
  --gemini-max-inline-bytes VALUE
  --llm-image-system-prompt VALUE
  --llm-voice-system-prompt VALUE
  --llm-video-system-prompt VALUE
  --openai-api-key VALUE
  --anthropic-api-key VALUE
  --tencent-lbs-key VALUE
  --tencent-lbs-endpoint VALUE
  --tencent-lbs-get-poi true|false
  --tencent-lbs-radius-meters VALUE
  --jina-api-key VALUE
  --jina-reader-endpoint VALUE
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub app: AppConfig,
    pub secrets: EnvSecrets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliConfig {
    pub env_file: Option<PathBuf>,
    pub use_process_env: bool,
    pub help: bool,
    values: HashMap<String, String>,
}

impl CliConfig {
    pub fn parse<I, S>(args: I) -> Result<Self, BridgeError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into).skip(1).peekable();
        let mut config = Self {
            env_file: None,
            use_process_env: false,
            help: false,
            values: HashMap::new(),
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => config.help = true,
                "--use-process-env" => config.use_process_env = true,
                "--env-file" => {
                    let value = next_arg_value(&mut args, "--env-file")?;
                    config.env_file = Some(PathBuf::from(value));
                }
                _ if arg.starts_with("--") => {
                    let (flag, inline_value) = split_flag_value(&arg);
                    let Some((_, key)) = FLAG_SPECS.iter().find(|(known, _)| *known == flag) else {
                        return Err(BridgeError::Config(format!("unknown option: {flag}")));
                    };
                    let value = match inline_value {
                        Some(value) => value.to_string(),
                        None => next_arg_value(&mut args, flag)?,
                    };
                    config.values.insert((*key).to_string(), value);
                }
                _ => {
                    return Err(BridgeError::Config(format!("unexpected argument: {arg}")));
                }
            }
        }

        Ok(config)
    }

    pub fn help_text() -> &'static str {
        HELP
    }
}

pub fn runtime_config_from_args<I, S>(args: I) -> Result<Option<RuntimeConfig>, BridgeError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let cli = CliConfig::parse(args)?;
    if cli.help {
        println!("{}", CliConfig::help_text());
        return Ok(None);
    }

    let mut values = HashMap::new();
    if cli.use_process_env {
        values.extend(env::vars().filter(|(_, value)| !value.trim().is_empty()));
    }
    if let Some(env_file) = cli.env_file.as_deref() {
        values.extend(load_env_file(env_file)?);
    }
    values.extend(cli.values);

    Ok(Some(RuntimeConfig {
        app: AppConfig::from_lookup(|key| values.get(key).cloned())?,
        secrets: EnvSecrets::from_lookup(|key| values.get(key).cloned()),
    }))
}

fn next_arg_value<I>(args: &mut std::iter::Peekable<I>, flag: &str) -> Result<String, BridgeError>
where
    I: Iterator<Item = String>,
{
    let Some(value) = args.next() else {
        return Err(BridgeError::Config(format!("{flag} requires a value")));
    };
    if value.starts_with("--") {
        return Err(BridgeError::Config(format!("{flag} requires a value")));
    }
    Ok(value)
}

fn split_flag_value(arg: &str) -> (&str, Option<&str>) {
    arg.split_once('=')
        .map(|(flag, value)| (flag, Some(value)))
        .unwrap_or((arg, None))
}

fn load_env_file(path: &Path) -> Result<HashMap<String, String>, BridgeError> {
    let content = fs::read_to_string(path).map_err(|err| {
        BridgeError::Config(format!("failed to read env file {}: {err}", path.display()))
    })?;
    let mut values = HashMap::new();
    for (index, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(BridgeError::Config(format!(
                "invalid env file {} line {}: missing '='",
                path.display(),
                index + 1
            )));
        };
        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            return Err(BridgeError::Config(format!(
                "invalid env file {} line {}: invalid key",
                path.display(),
                index + 1
            )));
        }
        values.insert(key.to_string(), unquote_env_value(value.trim()).to_string());
    }
    Ok(values)
}

fn unquote_env_value(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}

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
    pub admin_openids: Vec<String>,
}

impl EnvSecrets {
    pub fn from_env() -> Self {
        Self::from_lookup(|key| env::var(key).ok())
    }

    fn from_lookup<F>(lookup: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        Self {
            wechat_app_id: first_lookup(&lookup, &["WECHAT_APP_ID", "WECHAT_APPID"]),
            wechat_app_secret: first_lookup(&lookup, &["WECHAT_APP_SECRET", "WECHAT_APPSECRET"]),
            wechat_token: get_optional_from_lookup(&lookup, "WECHAT_TOKEN"),
            wechat_encoding_aes_key: get_optional_from_lookup(&lookup, "WECHAT_ENCODING_AES_KEY"),
            admin_view_key: get_optional_from_lookup(&lookup, "ADMIN_VIEW_KEY"),
            whitelist_join_key: get_optional_from_lookup(&lookup, "WHITELIST_JOIN_KEY"),
            gemini_api_key: get_optional_from_lookup(&lookup, "GEMINI_API_KEY"),
            openai_api_key: get_optional_from_lookup(&lookup, "OPENAI_API_KEY"),
            anthropic_api_key: get_optional_from_lookup(&lookup, "ANTHROPIC_API_KEY"),
            tencent_lbs_key: get_optional_from_lookup(&lookup, "TENCENT_LBS_KEY"),
            jina_api_key: get_optional_from_lookup(&lookup, "JINA_API_KEY"),
            admin_openids: parse_list_env(
                get_optional_from_lookup(&lookup, "WECHAT_ADMIN_OPENIDS").as_deref(),
            ),
        }
    }

    pub fn require_wechat_token(&self) -> Result<&str, BridgeError> {
        self.wechat_token
            .as_deref()
            .filter(|token| !token.is_empty())
            .ok_or_else(|| BridgeError::Config("WECHAT_TOKEN is required".to_string()))
    }
}

fn first_lookup<F>(lookup: &F, keys: &[&str]) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    keys.iter()
        .find_map(|key| lookup(key))
        .filter(|value| !value.trim().is_empty())
}

fn get_optional_from_lookup<F>(lookup: &F, key: &str) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    lookup(key).filter(|value| !value.trim().is_empty())
}

fn parse_list_env(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or("")
        .split([',', '\n', ';'])
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub bind_addr: String,
    pub database_url: String,
    pub raw_archive_dir: PathBuf,
    pub raw_archive_full: bool,
    pub processed_artifact_dir: PathBuf,
    pub source_dir: PathBuf,
    pub callback_path: String,
    pub encrypted_callback_enabled: bool,
    pub honeypot_reply_enabled: bool,
    pub honeypot_reply_text: String,
    pub worker_enabled: bool,
    pub worker_interval: Duration,
    pub worker_processing_timeout: Duration,
    pub http_timeout: Duration,
    pub wechat_api_base: String,
    pub wechat_oauth_authorize_base: String,
    pub max_media_bytes: u64,
    pub whitelist_join_redirect_url: Option<String>,
    pub gemini_endpoint_base: String,
    pub gemini_model: String,
    pub gemini_max_inline_bytes: u64,
    pub llm_image_system_prompt: String,
    pub llm_voice_system_prompt: String,
    pub llm_video_system_prompt: String,
    pub tencent_lbs_endpoint: String,
    pub tencent_lbs_get_poi: bool,
    pub tencent_lbs_radius_meters: Option<u32>,
    pub jina_reader_endpoint: String,
    pub log_filter: String,
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
            processed_artifact_dir: PathBuf::from(get_string(
                &lookup,
                "PROCESSED_ARTIFACT_DIR",
                "data/processed",
            )),
            source_dir: PathBuf::from(get_string(&lookup, "SAGE_WIKI_SOURCE_DIR", "source")),
            callback_path: get_string(&lookup, "WECHAT_CALLBACK_PATH", "/wechat/callback"),
            encrypted_callback_enabled: get_bool(
                &lookup,
                "WECHAT_ENCRYPTED_CALLBACK_ENABLED",
                false,
            )?,
            honeypot_reply_enabled: get_bool(&lookup, "HONEYPOT_REPLY_ENABLED", false)?,
            honeypot_reply_text: get_string(&lookup, "HONEYPOT_REPLY_TEXT", "Message received."),
            worker_enabled: get_bool(&lookup, "WORKER_ENABLED", true)?,
            worker_interval: Duration::from_millis(get_u64(&lookup, "WORKER_INTERVAL_MS", 1000)?),
            worker_processing_timeout: Duration::from_secs(get_u64(
                &lookup,
                "WORKER_PROCESSING_TIMEOUT_SECONDS",
                15 * 60,
            )?),
            http_timeout: Duration::from_secs(get_u64(&lookup, "HTTP_TIMEOUT_SECONDS", 30)?),
            wechat_api_base: get_string(&lookup, "WECHAT_API_BASE", "https://api.weixin.qq.com"),
            wechat_oauth_authorize_base: get_string(
                &lookup,
                "WECHAT_OAUTH_AUTHORIZE_BASE",
                "https://open.weixin.qq.com/connect/oauth2/authorize",
            ),
            max_media_bytes: get_u64(&lookup, "MAX_MEDIA_BYTES", 20 * 1024 * 1024)?,
            whitelist_join_redirect_url: get_optional_string(
                &lookup,
                "WHITELIST_JOIN_REDIRECT_URL",
            ),
            gemini_endpoint_base: get_string(
                &lookup,
                "GEMINI_ENDPOINT_BASE",
                "https://generativelanguage.googleapis.com",
            ),
            gemini_model: get_string(&lookup, "GEMINI_MODEL", "gemini-2.5-flash"),
            gemini_max_inline_bytes: get_u64(&lookup, "GEMINI_MAX_INLINE_BYTES", 18 * 1024 * 1024)?,
            llm_image_system_prompt: get_string(
                &lookup,
                "LLM_IMAGE_SYSTEM_PROMPT",
                "Describe this image for a personal knowledge base.",
            ),
            llm_voice_system_prompt: get_string(
                &lookup,
                "LLM_VOICE_SYSTEM_PROMPT",
                "Transcribe and summarize this voice message.",
            ),
            llm_video_system_prompt: get_string(
                &lookup,
                "LLM_VIDEO_SYSTEM_PROMPT",
                "Summarize this video for a personal knowledge base.",
            ),
            tencent_lbs_endpoint: get_string(
                &lookup,
                "TENCENT_LBS_ENDPOINT",
                "https://apis.map.qq.com/ws/geocoder/v1/",
            ),
            tencent_lbs_get_poi: get_bool(&lookup, "TENCENT_LBS_GET_POI", true)?,
            tencent_lbs_radius_meters: get_optional_u32(&lookup, "TENCENT_LBS_RADIUS_METERS")?,
            jina_reader_endpoint: get_string(&lookup, "JINA_READER_ENDPOINT", "https://r.jina.ai"),
            log_filter: get_string(&lookup, "RUST_LOG", "info,sage_wiki_bridge=debug"),
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

fn get_optional_string<F>(lookup: &F, key: &str) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    lookup(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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
    fn cli_flags_build_runtime_config_without_implicit_env() {
        let runtime = runtime_config_from_args([
            "sage-wiki-bridge",
            "--wechat-token",
            "token-from-cli",
            "--bind-addr",
            "0.0.0.0:18080",
            "--worker-enabled",
            "false",
        ])
        .unwrap()
        .unwrap();

        assert_eq!(
            runtime.secrets.wechat_token.as_deref(),
            Some("token-from-cli")
        );
        assert_eq!(runtime.app.bind_addr, "0.0.0.0:18080");
        assert!(!runtime.app.worker_enabled);
        assert_eq!(runtime.app.database_url, "sqlite://data/bridge.sqlite3");
    }

    #[test]
    fn cli_overrides_explicit_env_file() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            temp.path(),
            "WECHAT_TOKEN=token-from-file\nAPP_BIND_ADDR=127.0.0.1:9999\n",
        )
        .unwrap();
        let env_file = temp.path().to_string_lossy().to_string();

        let runtime = runtime_config_from_args([
            "sage-wiki-bridge",
            "--env-file",
            env_file.as_str(),
            "--bind-addr",
            "127.0.0.1:18081",
        ])
        .unwrap()
        .unwrap();

        assert_eq!(
            runtime.secrets.wechat_token.as_deref(),
            Some("token-from-file")
        );
        assert_eq!(runtime.app.bind_addr, "127.0.0.1:18081");
    }

    #[test]
    fn help_returns_no_runtime_config() {
        let runtime = runtime_config_from_args(["sage-wiki-bridge", "--help"]).unwrap();

        assert!(runtime.is_none());
    }

    #[test]
    fn app_config_uses_conservative_defaults() {
        let config = config_from_pairs(&[]).unwrap();

        assert_eq!(config.bind_addr, "127.0.0.1:8080");
        assert_eq!(config.database_url, "sqlite://data/bridge.sqlite3");
        assert_eq!(
            config.processed_artifact_dir,
            PathBuf::from("data/processed")
        );
        assert_eq!(config.callback_path, "/wechat/callback");
        assert_eq!(config.log_filter, "info,sage_wiki_bridge=debug");
        assert!(!config.encrypted_callback_enabled);
        assert!(!config.honeypot_reply_enabled);
        assert_eq!(config.honeypot_reply_text, "Message received.");
        assert!(config.worker_enabled);
        assert_eq!(config.worker_interval, Duration::from_millis(1000));
        assert_eq!(
            config.worker_processing_timeout,
            Duration::from_secs(15 * 60)
        );
        assert_eq!(config.source_dir, PathBuf::from("source"));
    }

    #[test]
    fn app_config_parses_overrides() {
        let config = config_from_pairs(&[
            ("APP_BIND_ADDR", "0.0.0.0:18080"),
            ("RAW_ARCHIVE_FULL", "false"),
            ("WECHAT_ENCRYPTED_CALLBACK_ENABLED", "true"),
            ("HONEYPOT_REPLY_ENABLED", "true"),
            ("HONEYPOT_REPLY_TEXT", "收到"),
            ("WORKER_INTERVAL_MS", "250"),
            ("WORKER_PROCESSING_TIMEOUT_SECONDS", "120"),
            ("LLM_IMAGE_SYSTEM_PROMPT", "看图总结"),
            ("TENCENT_LBS_RADIUS_METERS", "500"),
        ])
        .unwrap();

        assert_eq!(config.bind_addr, "0.0.0.0:18080");
        assert!(!config.raw_archive_full);
        assert!(config.encrypted_callback_enabled);
        assert!(config.honeypot_reply_enabled);
        assert_eq!(config.honeypot_reply_text, "收到");
        assert_eq!(config.worker_interval, Duration::from_millis(250));
        assert_eq!(config.worker_processing_timeout, Duration::from_secs(120));
        assert_eq!(config.llm_image_system_prompt, "看图总结");
        assert_eq!(config.tencent_lbs_radius_meters, Some(500));
    }

    #[test]
    fn app_config_rejects_bad_bool() {
        let err = config_from_pairs(&[("WORKER_ENABLED", "maybe")]).unwrap_err();

        assert!(matches!(err, BridgeError::Config(_)));
    }

    #[test]
    fn parses_list_env_values() {
        assert_eq!(
            parse_list_env(Some("openid-1, openid-2\nopenid-3;openid-4")),
            vec!["openid-1", "openid-2", "openid-3", "openid-4"]
        );
        assert!(parse_list_env(None).is_empty());
    }
}
