use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Serialize;

use crate::error::BridgeError;

const FLAG_SPECS: &[(&str, &str)] = &[
    ("--bind-addr", "APP_BIND_ADDR"),
    ("--database-url", "DATABASE_URL"),
    ("--database-max-connections", "DATABASE_MAX_CONNECTIONS"),
    ("--database-min-connections", "DATABASE_MIN_CONNECTIONS"),
    ("--raw-archive-dir", "RAW_ARCHIVE_DIR"),
    ("--raw-archive-full", "RAW_ARCHIVE_FULL"),
    ("--processed-artifact-dir", "PROCESSED_ARTIFACT_DIR"),
    ("--sage-wiki-source-dir", "SAGE_WIKI_SOURCE_DIR"),
    ("--sage-wiki-source-log-dir", "SAGE_WIKI_SOURCE_LOG_DIR"),
    ("--wechat-callback-path", "WECHAT_CALLBACK_PATH"),
    (
        "--wechat-encrypted-callback-enabled",
        "WECHAT_ENCRYPTED_CALLBACK_ENABLED",
    ),
    ("--honeypot-reply-enabled", "HONEYPOT_REPLY_ENABLED"),
    ("--honeypot-reply-text", "HONEYPOT_REPLY_TEXT"),
    ("--worker-enabled", "WORKER_ENABLED"),
    ("--worker-id", "WORKER_ID"),
    ("--bridge-version", "BRIDGE_VERSION"),
    ("--worker-interval-ms", "WORKER_INTERVAL_MS"),
    (
        "--worker-processing-timeout-seconds",
        "WORKER_PROCESSING_TIMEOUT_SECONDS",
    ),
    ("--worker-retry-base-seconds", "WORKER_RETRY_BASE_SECONDS"),
    ("--worker-retry-max-seconds", "WORKER_RETRY_MAX_SECONDS"),
    ("--http-timeout-seconds", "HTTP_TIMEOUT_SECONDS"),
    ("--request-body-limit-bytes", "REQUEST_BODY_LIMIT_BYTES"),
    ("--healthz-path", "HEALTHZ_PATH"),
    ("--readyz-path", "READYZ_PATH"),
    ("--wechat-api-base", "WECHAT_API_BASE"),
    ("--max-media-bytes", "MAX_MEDIA_BYTES"),
    (
        "--wechat-token-refresh-skew-seconds",
        "WECHAT_TOKEN_REFRESH_SKEW_SECONDS",
    ),
    ("--whitelist-join-command", "WHITELIST_JOIN_COMMAND"),
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
    ("--admin-base-path", "ADMIN_BASE_PATH"),
    ("--gemini-api-key", "GEMINI_API_KEY"),
    ("--openai-api-key", "OPENAI_API_KEY"),
    ("--anthropic-api-key", "ANTHROPIC_API_KEY"),
    ("--tencent-lbs-key", "TENCENT_LBS_KEY"),
    ("--jina-api-key", "JINA_API_KEY"),
    ("--wechat-admin-openids", "WECHAT_ADMIN_OPENIDS"),
    ("--rust-log", "RUST_LOG"),
    ("--admin-default-per-page", "ADMIN_DEFAULT_PER_PAGE"),
    ("--admin-max-per-page", "ADMIN_MAX_PER_PAGE"),
];

const ENV_ALIAS_SPECS: &[(&str, &str)] = &[
    ("BRIDGE_BIND_ADDR", "APP_BIND_ADDR"),
    ("BRIDGE_DATABASE_URL", "DATABASE_URL"),
    (
        "BRIDGE_DATABASE_MAX_CONNECTIONS",
        "DATABASE_MAX_CONNECTIONS",
    ),
    (
        "BRIDGE_DATABASE_MIN_CONNECTIONS",
        "DATABASE_MIN_CONNECTIONS",
    ),
    ("BRIDGE_RAW_ARCHIVE_DIR", "RAW_ARCHIVE_DIR"),
    ("BRIDGE_RAW_ARCHIVE_FULL", "RAW_ARCHIVE_FULL"),
    ("BRIDGE_PROCESSED_ARTIFACT_DIR", "PROCESSED_ARTIFACT_DIR"),
    ("BRIDGE_SAGE_WIKI_SOURCE_DIR", "SAGE_WIKI_SOURCE_DIR"),
    (
        "BRIDGE_SAGE_WIKI_SOURCE_LOG_DIR",
        "SAGE_WIKI_SOURCE_LOG_DIR",
    ),
    ("BRIDGE_WECHAT_CALLBACK_PATH", "WECHAT_CALLBACK_PATH"),
    (
        "BRIDGE_WECHAT_ENCRYPTED_CALLBACK_ENABLED",
        "WECHAT_ENCRYPTED_CALLBACK_ENABLED",
    ),
    ("BRIDGE_HONEYPOT_REPLY_ENABLED", "HONEYPOT_REPLY_ENABLED"),
    ("BRIDGE_HONEYPOT_REPLY_TEXT", "HONEYPOT_REPLY_TEXT"),
    ("BRIDGE_WORKER_ENABLED", "WORKER_ENABLED"),
    ("BRIDGE_WORKER_ID", "WORKER_ID"),
    ("BRIDGE_APP_VERSION", "BRIDGE_VERSION"),
    ("BRIDGE_BRIDGE_VERSION", "BRIDGE_VERSION"),
    ("BRIDGE_WORKER_INTERVAL_MS", "WORKER_INTERVAL_MS"),
    (
        "BRIDGE_WORKER_PROCESSING_TIMEOUT_SECONDS",
        "WORKER_PROCESSING_TIMEOUT_SECONDS",
    ),
    (
        "BRIDGE_WORKER_RETRY_BASE_SECONDS",
        "WORKER_RETRY_BASE_SECONDS",
    ),
    (
        "BRIDGE_WORKER_RETRY_MAX_SECONDS",
        "WORKER_RETRY_MAX_SECONDS",
    ),
    ("BRIDGE_HTTP_TIMEOUT_SECONDS", "HTTP_TIMEOUT_SECONDS"),
    (
        "BRIDGE_REQUEST_BODY_LIMIT_BYTES",
        "REQUEST_BODY_LIMIT_BYTES",
    ),
    ("BRIDGE_HEALTHZ_PATH", "HEALTHZ_PATH"),
    ("BRIDGE_READYZ_PATH", "READYZ_PATH"),
    ("BRIDGE_WECHAT_API_BASE", "WECHAT_API_BASE"),
    ("BRIDGE_MAX_MEDIA_BYTES", "MAX_MEDIA_BYTES"),
    (
        "BRIDGE_WECHAT_TOKEN_REFRESH_SKEW_SECONDS",
        "WECHAT_TOKEN_REFRESH_SKEW_SECONDS",
    ),
    ("BRIDGE_WHITELIST_JOIN_COMMAND", "WHITELIST_JOIN_COMMAND"),
    ("BRIDGE_GEMINI_ENDPOINT_BASE", "GEMINI_ENDPOINT_BASE"),
    ("BRIDGE_GEMINI_MODEL", "GEMINI_MODEL"),
    ("BRIDGE_GEMINI_MAX_INLINE_BYTES", "GEMINI_MAX_INLINE_BYTES"),
    ("BRIDGE_LLM_IMAGE_SYSTEM_PROMPT", "LLM_IMAGE_SYSTEM_PROMPT"),
    ("BRIDGE_LLM_VOICE_SYSTEM_PROMPT", "LLM_VOICE_SYSTEM_PROMPT"),
    ("BRIDGE_LLM_VIDEO_SYSTEM_PROMPT", "LLM_VIDEO_SYSTEM_PROMPT"),
    ("BRIDGE_TENCENT_LBS_ENDPOINT", "TENCENT_LBS_ENDPOINT"),
    ("BRIDGE_TENCENT_LBS_GET_POI", "TENCENT_LBS_GET_POI"),
    (
        "BRIDGE_TENCENT_LBS_RADIUS_METERS",
        "TENCENT_LBS_RADIUS_METERS",
    ),
    ("BRIDGE_JINA_READER_ENDPOINT", "JINA_READER_ENDPOINT"),
    ("BRIDGE_RUST_LOG", "RUST_LOG"),
    ("BRIDGE_ADMIN_BASE_PATH", "ADMIN_BASE_PATH"),
    ("BRIDGE_ADMIN_DEFAULT_PER_PAGE", "ADMIN_DEFAULT_PER_PAGE"),
    ("BRIDGE_ADMIN_MAX_PER_PAGE", "ADMIN_MAX_PER_PAGE"),
    ("BRIDGE_WECHAT_APP_ID", "WECHAT_APP_ID"),
    ("BRIDGE_WECHAT_APPID", "WECHAT_APP_ID"),
    ("BRIDGE_WECHAT_APP_SECRET", "WECHAT_APP_SECRET"),
    ("BRIDGE_WECHAT_APPSECRET", "WECHAT_APP_SECRET"),
    ("BRIDGE_WECHAT_TOKEN", "WECHAT_TOKEN"),
    ("BRIDGE_WECHAT_ENCODING_AES_KEY", "WECHAT_ENCODING_AES_KEY"),
    ("BRIDGE_ADMIN_VIEW_KEY", "ADMIN_VIEW_KEY"),
    ("BRIDGE_GEMINI_API_KEY", "GEMINI_API_KEY"),
    ("BRIDGE_OPENAI_API_KEY", "OPENAI_API_KEY"),
    ("BRIDGE_ANTHROPIC_API_KEY", "ANTHROPIC_API_KEY"),
    ("BRIDGE_TENCENT_LBS_KEY", "TENCENT_LBS_KEY"),
    ("BRIDGE_JINA_API_KEY", "JINA_API_KEY"),
    ("BRIDGE_WECHAT_ADMIN_OPENIDS", "WECHAT_ADMIN_OPENIDS"),
];

const HELP: &str = r#"sage-wiki-bridge

Usage:
  sage-wiki-bridge [OPTIONS]
  sage-wiki-bridge version [OPTIONS]
  sage-wiki-bridge status [OPTIONS]
  sage-wiki-bridge doctor [OPTIONS]
  sage-wiki-bridge health [OPTIONS]
  sage-wiki-bridge ready [OPTIONS]

Configuration sources are explicit and ordered:
  CLI flags > --env-file PATH > --use-process-env > built-in defaults.

Source controls:
  --env-file PATH
      Load dotenv-style config from PATH. Default: not loaded.
  --use-process-env
      Read process environment variables. Default: false.
  --help
      Print this help.
  --version
      Print the package version and exit.
  -V
      Print detailed version, resolved config, and config sources, then exit.

Core:
  --rust-log VALUE
      Default: info,sage_wiki_bridge=debug
  --bind-addr VALUE
      Default: 127.0.0.1:8080
  --database-url VALUE
      Default: sqlite://data/bridge.sqlite3
  --database-max-connections VALUE
      Default: 4
  --database-min-connections VALUE
      Default: 1
  --raw-archive-dir VALUE
      Default: data/raw
  --raw-archive-full true|false
      Default: true
  --processed-artifact-dir VALUE
      Default: data/processed
  --sage-wiki-source-dir VALUE
      Default: source
  --sage-wiki-source-log-dir VALUE
      Default: data/source-log
  --http-timeout-seconds VALUE
      Default: 30
  --request-body-limit-bytes VALUE
      Default: 2097152
  --healthz-path VALUE
      Default: /healthz
  --readyz-path VALUE
      Default: /readyz

WeChat:
  --wechat-token VALUE
      Default: none. Required for callback signature verification.
  --wechat-app-id VALUE
      Default: none.
  --wechat-app-secret VALUE
      Default: none.
  --wechat-encoding-aes-key VALUE
      Default: none. Required only when encrypted callback is enabled.
  --wechat-callback-path VALUE
      Default: /wechat/callback
  --wechat-encrypted-callback-enabled true|false
      Default: false
  --wechat-api-base VALUE
      Default: https://api.weixin.qq.com
  --wechat-admin-openids VALUE
      Default: empty list
  --wechat-token-refresh-skew-seconds VALUE
      Default: 300

Admin:
  --admin-base-path VALUE
      Default: /admin
  --admin-view-key VALUE
      Default: none.
  --whitelist-join-command VALUE
      Default: empty. When set, an exact text message match adds the sender OpenID to the whitelist.
  --admin-default-per-page VALUE
      Default: 20
  --admin-max-per-page VALUE
      Default: 100
  --honeypot-reply-enabled true|false
      Default: false
  --honeypot-reply-text VALUE
      Default: Message received.

Worker and external services:
  --worker-enabled true|false
      Default: true
  --worker-id VALUE
      Default: worker-main
  --bridge-version VALUE
      Default: {CARGO_PKG_VERSION}
  --worker-interval-ms VALUE
      Default: 1000
  --worker-processing-timeout-seconds VALUE
      Default: 900
  --worker-retry-base-seconds VALUE
      Default: 10
  --worker-retry-max-seconds VALUE
      Default: 300
  --max-media-bytes VALUE
      Default: 20971520
  --gemini-api-key VALUE
      Default: none.
  --gemini-endpoint-base VALUE
      Default: https://generativelanguage.googleapis.com
  --gemini-model VALUE
      Default: gemini-2.5-flash
  --gemini-max-inline-bytes VALUE
      Default: 18874368
  --llm-image-system-prompt VALUE
      Default: Describe this image for a personal knowledge base.
  --llm-voice-system-prompt VALUE
      Default: Transcribe and summarize this voice message.
  --llm-video-system-prompt VALUE
      Default: Summarize this video for a personal knowledge base.
  --openai-api-key VALUE
      Default: none.
  --anthropic-api-key VALUE
      Default: none.
  --tencent-lbs-key VALUE
      Default: none.
  --tencent-lbs-endpoint VALUE
      Default: https://apis.map.qq.com/ws/geocoder/v1/
  --tencent-lbs-get-poi true|false
      Default: true
  --tencent-lbs-radius-meters VALUE
      Default: empty.
  --jina-api-key VALUE
      Default: none.
  --jina-reader-endpoint VALUE
      Default: https://r.jina.ai
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub app: AppConfig,
    pub secrets: EnvSecrets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfigReport {
    pub runtime: RuntimeConfig,
    pub entries: Vec<ConfigReportEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfigReportEntry {
    pub key: &'static str,
    pub flag: &'static str,
    pub value: String,
    pub source: String,
    pub secret: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliConfig {
    pub env_file: Option<PathBuf>,
    pub use_process_env: bool,
    pub help: bool,
    pub version: bool,
    pub verbose_version: bool,
    pub command: Option<CliCommand>,
    values: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliCommand {
    Version,
    Status,
    Doctor,
    Health,
    Ready,
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
            version: false,
            verbose_version: false,
            command: None,
            values: HashMap::new(),
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "version" if config.command.is_none() => config.command = Some(CliCommand::Version),
                "status" if config.command.is_none() => config.command = Some(CliCommand::Status),
                "doctor" if config.command.is_none() => config.command = Some(CliCommand::Doctor),
                "health" if config.command.is_none() => config.command = Some(CliCommand::Health),
                "ready" if config.command.is_none() => config.command = Some(CliCommand::Ready),
                "--help" | "-h" => config.help = true,
                "--version" => config.version = true,
                "-V" => config.verbose_version = true,
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

    pub fn help_text() -> String {
        HELP.replace("{CARGO_PKG_VERSION}", env!("CARGO_PKG_VERSION"))
    }
}

pub fn runtime_config_from_args<I, S>(args: I) -> Result<Option<RuntimeConfig>, BridgeError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let report = runtime_config_report_from_args(args)?;
    Ok(report.map(|report| report.runtime))
}

pub fn runtime_config_report_from_args<I, S>(
    args: I,
) -> Result<Option<RuntimeConfigReport>, BridgeError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let cli = CliConfig::parse(args)?;
    if cli.help {
        println!("{}", CliConfig::help_text());
        return Ok(None);
    }
    if cli.version || cli.command == Some(CliCommand::Version) {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(None);
    }

    let mut resolved = ResolvedValues::default();
    if cli.use_process_env {
        for (key, value) in env::vars().filter(|(_, value)| !value.trim().is_empty()) {
            resolved.insert_input(key, value, "process-env".to_string());
        }
    }
    if let Some(env_file) = cli.env_file.as_deref() {
        for (key, value) in load_env_file(env_file)? {
            resolved.insert_input(key, value, format!("env-file:{}", env_file.display()));
        }
    }
    for (key, value) in cli.values {
        resolved.insert(key, value, "cli".to_string());
    }

    let runtime = RuntimeConfig {
        app: AppConfig::from_lookup(|key| resolved.values.get(key).cloned())?,
        secrets: EnvSecrets::from_lookup(|key| resolved.values.get(key).cloned()),
    };
    let entries = config_report_entries(&runtime, &resolved);

    Ok(Some(RuntimeConfigReport { runtime, entries }))
}

#[derive(Default)]
struct ResolvedValues {
    values: HashMap<String, String>,
    sources: HashMap<String, String>,
}

impl ResolvedValues {
    fn insert(&mut self, key: String, value: String, source: String) {
        self.values.insert(key.clone(), value);
        self.sources.insert(key, source);
    }

    fn insert_input(&mut self, key: String, value: String, source: String) {
        match canonical_env_key(&key) {
            Some((canonical, alias)) => {
                let source = if alias {
                    format!("{source} ({key})")
                } else {
                    source
                };
                self.insert(canonical.to_string(), value, source);
            }
            None => self.insert(key, value, source),
        }
    }

    fn source(&self, key: &str) -> String {
        self.sources
            .get(key)
            .cloned()
            .unwrap_or_else(|| "default".to_string())
    }
}

fn canonical_env_key(key: &str) -> Option<(&'static str, bool)> {
    if let Some((_, canonical)) = ENV_ALIAS_SPECS.iter().find(|(alias, _)| *alias == key) {
        return Some((*canonical, true));
    }
    if let Some(canonical) = canonical_special_key(key) {
        return Some((canonical, false));
    }
    None
}

fn canonical_special_key(key: &str) -> Option<&'static str> {
    match key {
        "WECHAT_APPID" => Some("WECHAT_APP_ID"),
        "WECHAT_APPSECRET" => Some("WECHAT_APP_SECRET"),
        "BRIDGE_VERSION" => Some("BRIDGE_VERSION"),
        _ => FLAG_SPECS
            .iter()
            .find_map(|(_, canonical)| (*canonical == key).then_some(*canonical)),
    }
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
    pub database_max_connections: u32,
    pub database_min_connections: u32,
    pub raw_archive_dir: PathBuf,
    pub raw_archive_full: bool,
    pub processed_artifact_dir: PathBuf,
    pub source_dir: PathBuf,
    pub source_log_dir: PathBuf,
    pub callback_path: String,
    pub encrypted_callback_enabled: bool,
    pub honeypot_reply_enabled: bool,
    pub honeypot_reply_text: String,
    pub worker_enabled: bool,
    pub worker_id: String,
    pub bridge_version: String,
    pub worker_interval: Duration,
    pub worker_processing_timeout: Duration,
    pub worker_retry_base: Duration,
    pub worker_retry_max: Duration,
    pub http_timeout: Duration,
    pub request_body_limit_bytes: usize,
    pub healthz_path: String,
    pub readyz_path: String,
    pub wechat_api_base: String,
    pub max_media_bytes: u64,
    pub wechat_token_refresh_skew: Duration,
    pub whitelist_join_command: Option<String>,
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
    pub admin_base_path: String,
    pub admin_default_per_page: u32,
    pub admin_max_per_page: u32,
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
            database_max_connections: get_u32(&lookup, "DATABASE_MAX_CONNECTIONS", 4)?,
            database_min_connections: get_u32(&lookup, "DATABASE_MIN_CONNECTIONS", 1)?,
            raw_archive_dir: PathBuf::from(get_string(&lookup, "RAW_ARCHIVE_DIR", "data/raw")),
            raw_archive_full: get_bool(&lookup, "RAW_ARCHIVE_FULL", true)?,
            processed_artifact_dir: PathBuf::from(get_string(
                &lookup,
                "PROCESSED_ARTIFACT_DIR",
                "data/processed",
            )),
            source_dir: PathBuf::from(get_string(&lookup, "SAGE_WIKI_SOURCE_DIR", "source")),
            source_log_dir: PathBuf::from(get_string(
                &lookup,
                "SAGE_WIKI_SOURCE_LOG_DIR",
                "data/source-log",
            )),
            callback_path: get_string(&lookup, "WECHAT_CALLBACK_PATH", "/wechat/callback"),
            encrypted_callback_enabled: get_bool(
                &lookup,
                "WECHAT_ENCRYPTED_CALLBACK_ENABLED",
                false,
            )?,
            honeypot_reply_enabled: get_bool(&lookup, "HONEYPOT_REPLY_ENABLED", false)?,
            honeypot_reply_text: get_string(&lookup, "HONEYPOT_REPLY_TEXT", "Message received."),
            worker_enabled: get_bool(&lookup, "WORKER_ENABLED", true)?,
            worker_id: get_string(&lookup, "WORKER_ID", "worker-main"),
            bridge_version: get_string(&lookup, "BRIDGE_VERSION", env!("CARGO_PKG_VERSION")),
            worker_interval: Duration::from_millis(get_u64(&lookup, "WORKER_INTERVAL_MS", 1000)?),
            worker_processing_timeout: Duration::from_secs(get_u64(
                &lookup,
                "WORKER_PROCESSING_TIMEOUT_SECONDS",
                15 * 60,
            )?),
            worker_retry_base: Duration::from_secs(get_u64(
                &lookup,
                "WORKER_RETRY_BASE_SECONDS",
                10,
            )?),
            worker_retry_max: Duration::from_secs(get_u64(
                &lookup,
                "WORKER_RETRY_MAX_SECONDS",
                300,
            )?),
            http_timeout: Duration::from_secs(get_u64(&lookup, "HTTP_TIMEOUT_SECONDS", 30)?),
            request_body_limit_bytes: get_usize(
                &lookup,
                "REQUEST_BODY_LIMIT_BYTES",
                2 * 1024 * 1024,
            )?,
            healthz_path: get_string(&lookup, "HEALTHZ_PATH", "/healthz"),
            readyz_path: get_string(&lookup, "READYZ_PATH", "/readyz"),
            wechat_api_base: get_string(&lookup, "WECHAT_API_BASE", "https://api.weixin.qq.com"),
            max_media_bytes: get_u64(&lookup, "MAX_MEDIA_BYTES", 20 * 1024 * 1024)?,
            wechat_token_refresh_skew: Duration::from_secs(get_u64(
                &lookup,
                "WECHAT_TOKEN_REFRESH_SKEW_SECONDS",
                300,
            )?),
            whitelist_join_command: get_optional_string(&lookup, "WHITELIST_JOIN_COMMAND"),
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
            admin_base_path: get_string(&lookup, "ADMIN_BASE_PATH", "/admin"),
            admin_default_per_page: get_u32(&lookup, "ADMIN_DEFAULT_PER_PAGE", 20)?,
            admin_max_per_page: get_u32(&lookup, "ADMIN_MAX_PER_PAGE", 100)?,
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

fn get_u32<F>(lookup: &F, key: &str, default: u32) -> Result<u32, BridgeError>
where
    F: Fn(&str) -> Option<String>,
{
    let Some(value) = lookup(key).filter(|value| !value.trim().is_empty()) else {
        return Ok(default);
    };
    value
        .parse::<u32>()
        .map_err(|_| BridgeError::Config(format!("{key} must be a positive integer")))
}

fn get_usize<F>(lookup: &F, key: &str, default: usize) -> Result<usize, BridgeError>
where
    F: Fn(&str) -> Option<String>,
{
    let Some(value) = lookup(key).filter(|value| !value.trim().is_empty()) else {
        return Ok(default);
    };
    value
        .parse::<usize>()
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

fn config_report_entries(
    runtime: &RuntimeConfig,
    resolved: &ResolvedValues,
) -> Vec<ConfigReportEntry> {
    let app = &runtime.app;
    let secrets = &runtime.secrets;
    let mut entries = vec![
        entry(
            "APP_BIND_ADDR",
            "--bind-addr",
            &app.bind_addr,
            resolved,
            false,
        ),
        entry(
            "DATABASE_URL",
            "--database-url",
            &app.database_url,
            resolved,
            false,
        ),
        entry_u32(
            "DATABASE_MAX_CONNECTIONS",
            "--database-max-connections",
            app.database_max_connections,
            resolved,
        ),
        entry_u32(
            "DATABASE_MIN_CONNECTIONS",
            "--database-min-connections",
            app.database_min_connections,
            resolved,
        ),
        entry_path(
            "RAW_ARCHIVE_DIR",
            "--raw-archive-dir",
            &app.raw_archive_dir,
            resolved,
        ),
        entry_bool(
            "RAW_ARCHIVE_FULL",
            "--raw-archive-full",
            app.raw_archive_full,
            resolved,
        ),
        entry_path(
            "PROCESSED_ARTIFACT_DIR",
            "--processed-artifact-dir",
            &app.processed_artifact_dir,
            resolved,
        ),
        entry_path(
            "SAGE_WIKI_SOURCE_DIR",
            "--sage-wiki-source-dir",
            &app.source_dir,
            resolved,
        ),
        entry_path(
            "SAGE_WIKI_SOURCE_LOG_DIR",
            "--sage-wiki-source-log-dir",
            &app.source_log_dir,
            resolved,
        ),
        entry(
            "WECHAT_CALLBACK_PATH",
            "--wechat-callback-path",
            &app.callback_path,
            resolved,
            false,
        ),
        entry_bool(
            "WECHAT_ENCRYPTED_CALLBACK_ENABLED",
            "--wechat-encrypted-callback-enabled",
            app.encrypted_callback_enabled,
            resolved,
        ),
        entry_bool(
            "HONEYPOT_REPLY_ENABLED",
            "--honeypot-reply-enabled",
            app.honeypot_reply_enabled,
            resolved,
        ),
        entry(
            "HONEYPOT_REPLY_TEXT",
            "--honeypot-reply-text",
            &app.honeypot_reply_text,
            resolved,
            false,
        ),
        entry_bool(
            "WORKER_ENABLED",
            "--worker-enabled",
            app.worker_enabled,
            resolved,
        ),
        entry("WORKER_ID", "--worker-id", &app.worker_id, resolved, false),
        entry(
            "BRIDGE_VERSION",
            "--bridge-version",
            &app.bridge_version,
            resolved,
            false,
        ),
        entry_u64(
            "WORKER_INTERVAL_MS",
            "--worker-interval-ms",
            app.worker_interval.as_millis() as u64,
            resolved,
        ),
        entry_u64(
            "WORKER_PROCESSING_TIMEOUT_SECONDS",
            "--worker-processing-timeout-seconds",
            app.worker_processing_timeout.as_secs(),
            resolved,
        ),
        entry_u64(
            "WORKER_RETRY_BASE_SECONDS",
            "--worker-retry-base-seconds",
            app.worker_retry_base.as_secs(),
            resolved,
        ),
        entry_u64(
            "WORKER_RETRY_MAX_SECONDS",
            "--worker-retry-max-seconds",
            app.worker_retry_max.as_secs(),
            resolved,
        ),
        entry_u64(
            "HTTP_TIMEOUT_SECONDS",
            "--http-timeout-seconds",
            app.http_timeout.as_secs(),
            resolved,
        ),
        entry(
            "REQUEST_BODY_LIMIT_BYTES",
            "--request-body-limit-bytes",
            &app.request_body_limit_bytes.to_string(),
            resolved,
            false,
        ),
        entry(
            "HEALTHZ_PATH",
            "--healthz-path",
            &app.healthz_path,
            resolved,
            false,
        ),
        entry(
            "READYZ_PATH",
            "--readyz-path",
            &app.readyz_path,
            resolved,
            false,
        ),
        entry(
            "WECHAT_API_BASE",
            "--wechat-api-base",
            &app.wechat_api_base,
            resolved,
            false,
        ),
        entry_u64(
            "MAX_MEDIA_BYTES",
            "--max-media-bytes",
            app.max_media_bytes,
            resolved,
        ),
        entry_u64(
            "WECHAT_TOKEN_REFRESH_SKEW_SECONDS",
            "--wechat-token-refresh-skew-seconds",
            app.wechat_token_refresh_skew.as_secs(),
            resolved,
        ),
        entry_opt(
            "WHITELIST_JOIN_COMMAND",
            "--whitelist-join-command",
            app.whitelist_join_command.as_deref(),
            resolved,
            false,
        ),
        entry(
            "GEMINI_ENDPOINT_BASE",
            "--gemini-endpoint-base",
            &app.gemini_endpoint_base,
            resolved,
            false,
        ),
        entry(
            "GEMINI_MODEL",
            "--gemini-model",
            &app.gemini_model,
            resolved,
            false,
        ),
        entry_u64(
            "GEMINI_MAX_INLINE_BYTES",
            "--gemini-max-inline-bytes",
            app.gemini_max_inline_bytes,
            resolved,
        ),
        entry(
            "LLM_IMAGE_SYSTEM_PROMPT",
            "--llm-image-system-prompt",
            &app.llm_image_system_prompt,
            resolved,
            false,
        ),
        entry(
            "LLM_VOICE_SYSTEM_PROMPT",
            "--llm-voice-system-prompt",
            &app.llm_voice_system_prompt,
            resolved,
            false,
        ),
        entry(
            "LLM_VIDEO_SYSTEM_PROMPT",
            "--llm-video-system-prompt",
            &app.llm_video_system_prompt,
            resolved,
            false,
        ),
        entry(
            "TENCENT_LBS_ENDPOINT",
            "--tencent-lbs-endpoint",
            &app.tencent_lbs_endpoint,
            resolved,
            false,
        ),
        entry_bool(
            "TENCENT_LBS_GET_POI",
            "--tencent-lbs-get-poi",
            app.tencent_lbs_get_poi,
            resolved,
        ),
        ConfigReportEntry {
            key: "TENCENT_LBS_RADIUS_METERS",
            flag: "--tencent-lbs-radius-meters",
            value: app
                .tencent_lbs_radius_meters
                .map(|value| value.to_string())
                .unwrap_or_default(),
            source: resolved.source("TENCENT_LBS_RADIUS_METERS"),
            secret: false,
        },
        entry(
            "JINA_READER_ENDPOINT",
            "--jina-reader-endpoint",
            &app.jina_reader_endpoint,
            resolved,
            false,
        ),
        entry("RUST_LOG", "--rust-log", &app.log_filter, resolved, false),
        entry(
            "ADMIN_BASE_PATH",
            "--admin-base-path",
            &app.admin_base_path,
            resolved,
            false,
        ),
        entry_u32(
            "ADMIN_DEFAULT_PER_PAGE",
            "--admin-default-per-page",
            app.admin_default_per_page,
            resolved,
        ),
        entry_u32(
            "ADMIN_MAX_PER_PAGE",
            "--admin-max-per-page",
            app.admin_max_per_page,
            resolved,
        ),
    ];

    entries.extend([
        secret_entry(
            "WECHAT_TOKEN",
            "--wechat-token",
            secrets.wechat_token.as_deref(),
            resolved,
        ),
        secret_entry(
            "WECHAT_APP_ID",
            "--wechat-app-id",
            secrets.wechat_app_id.as_deref(),
            resolved,
        ),
        secret_entry(
            "WECHAT_APP_SECRET",
            "--wechat-app-secret",
            secrets.wechat_app_secret.as_deref(),
            resolved,
        ),
        secret_entry(
            "WECHAT_ENCODING_AES_KEY",
            "--wechat-encoding-aes-key",
            secrets.wechat_encoding_aes_key.as_deref(),
            resolved,
        ),
        secret_entry(
            "ADMIN_VIEW_KEY",
            "--admin-view-key",
            secrets.admin_view_key.as_deref(),
            resolved,
        ),
        secret_entry(
            "GEMINI_API_KEY",
            "--gemini-api-key",
            secrets.gemini_api_key.as_deref(),
            resolved,
        ),
        secret_entry(
            "OPENAI_API_KEY",
            "--openai-api-key",
            secrets.openai_api_key.as_deref(),
            resolved,
        ),
        secret_entry(
            "ANTHROPIC_API_KEY",
            "--anthropic-api-key",
            secrets.anthropic_api_key.as_deref(),
            resolved,
        ),
        secret_entry(
            "TENCENT_LBS_KEY",
            "--tencent-lbs-key",
            secrets.tencent_lbs_key.as_deref(),
            resolved,
        ),
        secret_entry(
            "JINA_API_KEY",
            "--jina-api-key",
            secrets.jina_api_key.as_deref(),
            resolved,
        ),
        ConfigReportEntry {
            key: "WECHAT_ADMIN_OPENIDS",
            flag: "--wechat-admin-openids",
            value: format!("<redacted:{}>", secrets.admin_openids.len()),
            source: resolved.source("WECHAT_ADMIN_OPENIDS"),
            secret: true,
        },
    ]);

    entries
}

fn entry(
    key: &'static str,
    flag: &'static str,
    value: &str,
    resolved: &ResolvedValues,
    secret: bool,
) -> ConfigReportEntry {
    ConfigReportEntry {
        key,
        flag,
        value: value.to_string(),
        source: resolved.source(key),
        secret,
    }
}

fn entry_opt(
    key: &'static str,
    flag: &'static str,
    value: Option<&str>,
    resolved: &ResolvedValues,
    secret: bool,
) -> ConfigReportEntry {
    entry(key, flag, value.unwrap_or(""), resolved, secret)
}

fn entry_path(
    key: &'static str,
    flag: &'static str,
    value: &Path,
    resolved: &ResolvedValues,
) -> ConfigReportEntry {
    entry(key, flag, &value.display().to_string(), resolved, false)
}

fn entry_bool(
    key: &'static str,
    flag: &'static str,
    value: bool,
    resolved: &ResolvedValues,
) -> ConfigReportEntry {
    entry(
        key,
        flag,
        if value { "true" } else { "false" },
        resolved,
        false,
    )
}

fn entry_u32(
    key: &'static str,
    flag: &'static str,
    value: u32,
    resolved: &ResolvedValues,
) -> ConfigReportEntry {
    entry(key, flag, &value.to_string(), resolved, false)
}

fn entry_u64(
    key: &'static str,
    flag: &'static str,
    value: u64,
    resolved: &ResolvedValues,
) -> ConfigReportEntry {
    entry(key, flag, &value.to_string(), resolved, false)
}

fn secret_entry(
    key: &'static str,
    flag: &'static str,
    value: Option<&str>,
    resolved: &ResolvedValues,
) -> ConfigReportEntry {
    ConfigReportEntry {
        key,
        flag,
        value: match value {
            Some(value) if !value.is_empty() => format!("<redacted:{}>", value.len()),
            _ => String::new(),
        },
        source: resolved.source(key),
        secret: true,
    }
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
    fn version_returns_no_runtime_config() {
        let runtime = runtime_config_from_args(["sage-wiki-bridge", "--version"]).unwrap();

        assert!(runtime.is_none());
    }

    #[test]
    fn parses_status_command() {
        let cli = CliConfig::parse(["sage-wiki-bridge", "status"]).unwrap();

        assert_eq!(cli.command, Some(CliCommand::Status));
    }

    #[test]
    fn parses_operations_commands() {
        assert_eq!(
            CliConfig::parse(["sage-wiki-bridge", "doctor"])
                .unwrap()
                .command,
            Some(CliCommand::Doctor)
        );
        assert_eq!(
            CliConfig::parse(["sage-wiki-bridge", "health"])
                .unwrap()
                .command,
            Some(CliCommand::Health)
        );
        assert_eq!(
            CliConfig::parse(["sage-wiki-bridge", "ready"])
                .unwrap()
                .command,
            Some(CliCommand::Ready)
        );
    }

    #[test]
    fn env_file_supports_bridge_prefixed_aliases() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            temp.path(),
            "BRIDGE_BIND_ADDR=127.0.0.1:19090\nBRIDGE_WECHAT_TOKEN=token-from-bridge-prefix\n",
        )
        .unwrap();
        let env_file = temp.path().to_string_lossy().to_string();

        let report =
            runtime_config_report_from_args(["sage-wiki-bridge", "--env-file", env_file.as_str()])
                .unwrap()
                .unwrap();

        assert_eq!(report.runtime.app.bind_addr, "127.0.0.1:19090");
        assert_eq!(
            report.runtime.secrets.wechat_token.as_deref(),
            Some("token-from-bridge-prefix")
        );
        let bind_addr = report
            .entries
            .iter()
            .find(|entry| entry.key == "APP_BIND_ADDR")
            .unwrap();
        assert!(bind_addr.source.contains("BRIDGE_BIND_ADDR"));
    }

    #[test]
    fn config_report_tracks_sources() {
        let report =
            runtime_config_report_from_args(["sage-wiki-bridge", "--bind-addr", "0.0.0.0:18080"])
                .unwrap()
                .unwrap();

        let bind_addr = report
            .entries
            .iter()
            .find(|entry| entry.key == "APP_BIND_ADDR")
            .unwrap();
        let database_url = report
            .entries
            .iter()
            .find(|entry| entry.key == "DATABASE_URL")
            .unwrap();

        assert_eq!(bind_addr.value, "0.0.0.0:18080");
        assert_eq!(bind_addr.source, "cli");
        assert_eq!(database_url.value, "sqlite://data/bridge.sqlite3");
        assert_eq!(database_url.source, "default");
    }

    #[test]
    fn help_text_documents_defaults() {
        let help = CliConfig::help_text();

        assert!(help.contains("--bind-addr VALUE\n      Default: 127.0.0.1:8080"));
        assert!(help.contains("--database-url VALUE\n      Default: sqlite://data/bridge.sqlite3"));
        assert!(help.contains("--sage-wiki-source-log-dir VALUE\n      Default: data/source-log"));
        assert!(help.contains("--worker-enabled true|false\n      Default: true"));
        assert!(help.contains("--wechat-token VALUE\n      Default: none."));
        assert!(help.contains("--whitelist-join-command VALUE\n      Default: empty."));
        assert!(help.contains(&format!(
            "--bridge-version VALUE\n      Default: {}",
            env!("CARGO_PKG_VERSION")
        )));
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
        assert_eq!(config.whitelist_join_command, None);
        assert!(config.worker_enabled);
        assert_eq!(config.worker_interval, Duration::from_millis(1000));
        assert_eq!(
            config.worker_processing_timeout,
            Duration::from_secs(15 * 60)
        );
        assert_eq!(config.source_dir, PathBuf::from("source"));
        assert_eq!(config.source_log_dir, PathBuf::from("data/source-log"));
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
            ("WHITELIST_JOIN_COMMAND", "/sage-wiki-join"),
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
        assert_eq!(
            config.whitelist_join_command.as_deref(),
            Some("/sage-wiki-join")
        );
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
