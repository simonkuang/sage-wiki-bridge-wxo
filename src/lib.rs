pub mod admin;
pub mod archive;
pub mod config;
pub mod enrich;
pub mod error;
pub mod health;
pub mod llm;
pub mod media;
pub mod preprocess;
pub mod receiver;
pub mod source;
pub mod status;
pub mod store;
pub mod telemetry;
pub mod wechat;
pub mod worker;

use std::{env, fs, path::Path, sync::Arc, time::Duration};

use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use axum::{
    body::Body,
    http::Request,
    middleware::{self, Next},
    response::Response,
};

use crate::{
    admin::AdminState,
    archive::{ProcessedArtifactStore, RawArchive},
    config::{
        AppConfig, CliCommand, CliConfig, ConfigReportEntry, EnvSecrets, RuntimeConfig,
        RuntimeConfigReport, runtime_config_report_from_args,
    },
    enrich::{
        http_client::HttpExternalClients, jina_reader::JinaReaderOptions,
        tencent_lbs::TencentLbsOptions,
    },
    error::BridgeError,
    health::HealthState,
    llm::gemini::{GeminiClient, GeminiConfig},
    media::{WechatApiConfig, WechatMediaClient},
    receiver::{ReceiverConfig, ReceiverState},
    source::SourceWriter,
    status::{StatusContext, build_service_status},
    store::{StatusSnapshot, Store},
    worker::{
        ExternalClients, MediaJobProcessor, NoopExternalClients, NoopMediaJobProcessor,
        RetryPolicy, WorkOutcome, Worker, media_processor::GeminiMediaJobProcessor,
    },
};

pub async fn run() -> Result<(), error::BridgeError> {
    run_from_args(env::args()).await
}

pub async fn run_from_args<I, S>(args: I) -> Result<(), error::BridgeError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let cli = CliConfig::parse(args.clone())?;
    let Some(report) = runtime_config_report_from_args(args)? else {
        return Ok(());
    };

    if cli.verbose_version {
        print_verbose_config_report(&report);
        return Ok(());
    }
    if cli.command == Some(CliCommand::Status) {
        print_status_report(&report).await?;
        return Ok(());
    }
    if cli.command == Some(CliCommand::Doctor) {
        run_doctor(&report, cli.env_file.as_deref())?;
        return Ok(());
    }
    if cli.command == Some(CliCommand::Health) {
        check_local_endpoint(&report.runtime.app, &report.runtime.app.healthz_path).await?;
        return Ok(());
    }
    if cli.command == Some(CliCommand::Ready) {
        check_local_endpoint(&report.runtime.app, &report.runtime.app.readyz_path).await?;
        return Ok(());
    }

    run_with_report(report).await
}

fn print_verbose_config_report(report: &RuntimeConfigReport) {
    println!("sage-wiki-bridge {}", env!("CARGO_PKG_VERSION"));
    println!("build:");
    println!("  package_version: {}", env!("CARGO_PKG_VERSION"));
    println!("  target: {}", std::env::consts::ARCH);
    println!("  os: {}", std::env::consts::OS);
    println!("config:");
    print_config_entries(&report.entries);
}

async fn print_status_report(report: &RuntimeConfigReport) -> Result<(), BridgeError> {
    if let Some(running_status) = fetch_running_status(report).await? {
        println!("{running_status}");
        return Ok(());
    }

    let config = &report.runtime.app;
    let store = Store::connect_with_pool_options(
        &config.database_url,
        config.database_max_connections,
        config.database_min_connections,
    )
    .await?;
    let context = StatusContext::from_report(report);
    let status = build_service_status(&store, &context).await?;

    println!("sage-wiki-bridge status");
    println!("instance:");
    println!("  scope: configured_runtime_snapshot");
    println!("  package_version: {}", status.service.package_version);
    println!("  status_command_pid: {}", status.process.pid);
    println!("  memory_rss_bytes: {:?}", status.process.memory_rss_bytes);
    println!("  database_url: {}", status.service.database_url);
    println!("  bind_addr: {}", status.service.bind_addr);
    println!("  worker_enabled: {}", status.service.worker_enabled);
    println!("  callback_path: {}", status.endpoints.callback_path);
    println!("  admin_status_path: {}", status.endpoints.status_path);
    println!("config:");
    print_config_entries(&report.entries);
    println!("runtime_checks:");
    println!("  database_ready: {}", status.runtime_checks.database_ready);
    println!(
        "  source_dir_writable: {}",
        status.runtime_checks.source_dir_writable
    );
    println!(
        "  raw_archive_dir_writable: {}",
        status.runtime_checks.raw_archive_dir_writable
    );
    println!(
        "  processed_artifact_dir_writable: {}",
        status.runtime_checks.processed_artifact_dir_writable
    );
    println!("metrics:");
    print_status_snapshot(&status.metrics);

    Ok(())
}

async fn fetch_running_status(report: &RuntimeConfigReport) -> Result<Option<String>, BridgeError> {
    let Some(admin_view_key) = report
        .runtime
        .secrets
        .admin_view_key
        .as_deref()
        .filter(|key| !key.is_empty())
    else {
        return Ok(None);
    };
    let config = &report.runtime.app;
    let url = local_url(config, &format!("{}/status", config.admin_base_path));
    let client = reqwest::Client::builder()
        .timeout(config.http_timeout.min(Duration::from_secs(3)))
        .build()
        .map_err(|err| BridgeError::ExternalRequest(err.to_string()))?;
    let Ok(response) = client.get(&url).bearer_auth(admin_view_key).send().await else {
        return Ok(None);
    };
    if !response.status().is_success() {
        return Ok(None);
    }
    let text = response
        .text()
        .await
        .map_err(|err| BridgeError::ExternalRequest(err.to_string()))?;
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(value) => Ok(Some(
            serde_json::to_string_pretty(&value)
                .map_err(|err| BridgeError::ExternalPayloadInvalid(err.to_string()))?,
        )),
        Err(_) => Ok(Some(text)),
    }
}

fn run_doctor(report: &RuntimeConfigReport, env_file: Option<&Path>) -> Result<(), BridgeError> {
    let mut failed = false;
    let config = &report.runtime.app;
    let secrets = &report.runtime.secrets;

    println!("sage-wiki-bridge doctor");
    println!("package_version: {}", env!("CARGO_PKG_VERSION"));
    println!("env_file: {}", display_optional_path(env_file));
    println!("bind_addr: {}", config.bind_addr);
    println!("callback_path: {}", config.callback_path);
    println!("database_url: {}", config.database_url);
    println!("health_url: {}", local_url(config, &config.healthz_path));
    println!("ready_url: {}", local_url(config, &config.readyz_path));

    check_required("WECHAT_TOKEN", secrets.wechat_token.as_deref(), &mut failed);
    check_required(
        "WECHAT_ADMIN_OPENIDS",
        (!secrets.admin_openids.is_empty()).then_some("<configured>"),
        &mut failed,
    );
    check_required(
        "ADMIN_VIEW_KEY",
        secrets.admin_view_key.as_deref(),
        &mut failed,
    );
    if config.encrypted_callback_enabled {
        check_required(
            "WECHAT_ENCODING_AES_KEY",
            secrets.wechat_encoding_aes_key.as_deref(),
            &mut failed,
        );
    }

    check_dir(
        "sage-wiki source dir",
        &config.source_dir,
        false,
        &mut failed,
    );
    check_dir(
        "raw archive dir",
        &config.raw_archive_dir,
        true,
        &mut failed,
    );
    check_dir(
        "processed artifact dir",
        &config.processed_artifact_dir,
        true,
        &mut failed,
    );
    if let Some(parent) = sqlite_database_parent(&config.database_url) {
        check_dir("database dir", &parent, true, &mut failed);
    }

    if failed {
        return Err(BridgeError::Config(
            "doctor found one or more blocking problems".to_string(),
        ));
    }
    Ok(())
}

async fn check_local_endpoint(config: &AppConfig, path: &str) -> Result<(), BridgeError> {
    let url = local_url(config, path);
    let client = reqwest::Client::builder()
        .timeout(config.http_timeout)
        .build()
        .map_err(|err| BridgeError::ExternalRequest(err.to_string()))?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|err| BridgeError::ExternalRequest(format!("{url}: {err}")))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| BridgeError::ExternalRequest(err.to_string()))?;
    println!("{status} {url}");
    println!("{body}");
    if !status.is_success() {
        return Err(BridgeError::ExternalRequest(format!(
            "{url} returned {status}"
        )));
    }
    Ok(())
}

fn local_url(config: &AppConfig, path: &str) -> String {
    let bind_addr = config
        .bind_addr
        .strip_prefix("0.0.0.0:")
        .map(|port| format!("127.0.0.1:{port}"))
        .unwrap_or_else(|| config.bind_addr.clone());
    format!("http://{bind_addr}{path}")
}

fn display_optional_path(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "<not loaded>".to_string())
}

fn check_required(name: &str, value: Option<&str>, failed: &mut bool) {
    if value.is_some_and(|value| !value.trim().is_empty()) {
        println!("ok: {name} is set");
    } else {
        println!("error: {name} is required");
        *failed = true;
    }
}

fn check_dir(label: &str, path: &Path, create: bool, failed: &mut bool) {
    if create {
        let _ = fs::create_dir_all(path);
    }
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.permissions().readonly() => {
            println!("ok: {label} exists and is writable: {}", path.display());
        }
        Ok(metadata) if metadata.is_dir() => {
            println!(
                "error: {label} exists but is not writable: {}",
                path.display()
            );
            *failed = true;
        }
        _ => {
            println!("error: {label} does not exist: {}", path.display());
            *failed = true;
        }
    }
}

fn sqlite_database_parent(database_url: &str) -> Option<std::path::PathBuf> {
    let path = database_url.strip_prefix("sqlite://")?;
    Path::new(path).parent().map(Path::to_path_buf)
}

fn print_config_entries(entries: &[ConfigReportEntry]) {
    for entry in entries {
        let redacted = if entry.secret { ", redacted" } else { "" };
        println!(
            "  {} = {}  # flag: {}, source: {}{}",
            entry.key, entry.value, entry.flag, entry.source, redacted
        );
    }
}

fn print_status_snapshot(snapshot: &StatusSnapshot) {
    println!("  total_messages: {}", snapshot.total_messages);
    println!("  authorized_messages: {}", snapshot.authorized_messages);
    println!("  failed_messages: {}", snapshot.failed_messages);
    println!(
        "  source_written_messages: {}",
        snapshot.source_written_messages
    );
    println!("  total_jobs: {}", snapshot.total_jobs);
    println!("  total_job_attempts: {}", snapshot.total_job_attempts);
    println!("  retry_attempts: {}", snapshot.retry_attempts);
    println!("  processed_text_bytes: {}", snapshot.processed_text_bytes);
    println!("  source_bytes_written: {}", snapshot.source_bytes_written);
    println!("  token_usage: not_tracked");
    println!("  messages_by_status:");
    print_grouped_counts(&snapshot.messages_by_status);
    println!("  messages_by_type:");
    print_grouped_counts(&snapshot.messages_by_type);
    println!("  jobs_by_status:");
    print_grouped_counts(&snapshot.jobs_by_status);
}

fn print_grouped_counts(counts: &[(String, i64)]) {
    if counts.is_empty() {
        println!("    <empty>: 0");
        return;
    }
    for (key, count) in counts {
        println!("    {key}: {count}");
    }
}

pub async fn run_with_config(runtime_config: RuntimeConfig) -> Result<(), error::BridgeError> {
    run_with_report(RuntimeConfigReport {
        entries: Vec::new(),
        runtime: runtime_config,
    })
    .await
}

pub async fn run_with_report(
    runtime_config: RuntimeConfigReport,
) -> Result<(), error::BridgeError> {
    let secrets = runtime_config.runtime.secrets;
    let config = runtime_config.runtime.app;
    let status_context = StatusContext::from_config_and_entries(&config, runtime_config.entries);
    telemetry::init(&config.log_filter);
    let wechat_token = secrets.require_wechat_token()?.to_string();

    ensure_sqlite_parent(&config.database_url)?;
    let store = Store::connect_with_pool_options(
        &config.database_url,
        config.database_max_connections,
        config.database_min_connections,
    )
    .await?;
    store.migrate().await?;
    seed_configured_admin_openids(&store, &secrets).await?;

    let external_clients = build_external_clients(&config, &secrets)?;
    let media_processor = build_media_processor(&config, &secrets)?;
    let worker = Worker::with_processors(
        store.clone(),
        SourceWriter::new(&config.source_dir),
        external_clients,
        media_processor,
        config.worker_id.clone(),
        config.bridge_version.clone(),
    )
    .with_processed_artifact_store(ProcessedArtifactStore::new(&config.processed_artifact_dir))
    .with_retry_policy(RetryPolicy {
        base_delay: config.worker_retry_base,
        max_delay: config.worker_retry_max,
    });

    if config.worker_enabled {
        let interval = config.worker_interval;
        let processing_timeout = config.worker_processing_timeout;
        tokio::spawn(async move {
            run_worker_loop(worker, interval, processing_timeout).await;
        });
    }

    let app = receiver::router(ReceiverState {
        config: ReceiverConfig {
            wechat_token,
            callback_path: config.callback_path.clone(),
            encrypted_callback_enabled: config.encrypted_callback_enabled,
            wechat_app_id: secrets.wechat_app_id.clone(),
            wechat_encoding_aes_key: secrets.wechat_encoding_aes_key.clone(),
            honeypot_reply_enabled: config.honeypot_reply_enabled,
            honeypot_reply_text: config.honeypot_reply_text.clone(),
            whitelist_join_command: config.whitelist_join_command.clone(),
            request_body_limit_bytes: config.request_body_limit_bytes,
        },
        store: store.clone(),
        raw_archive: RawArchive::new(&config.raw_archive_dir, config.raw_archive_full),
    })
    .merge(admin::router(AdminState {
        store: store.clone(),
        base_path: config.admin_base_path.clone(),
        view_key: secrets.admin_view_key.clone(),
        default_per_page: config.admin_default_per_page,
        max_per_page: config.admin_max_per_page,
        status_context,
    }))
    .merge(health::router(HealthState {
        store,
        healthz_path: config.healthz_path.clone(),
        readyz_path: config.readyz_path.clone(),
    }))
    .layer(middleware::from_fn(access_log));
    let listener = tokio::net::TcpListener::bind(&config.bind_addr)
        .await
        .map_err(|err| {
            BridgeError::Config(format!("failed to bind {}: {err}", config.bind_addr))
        })?;
    tracing::info!(
        component = "main",
        bind_addr = %config.bind_addr,
        callback_path = %config.callback_path,
        worker_enabled = config.worker_enabled,
        "sage-wiki-bridge listening"
    );

    axum::serve(listener, app)
        .await
        .map_err(|err| BridgeError::Config(format!("server failed: {err}")))
}

async fn access_log(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path().to_string();
    let query = uri.query().map(str::to_string);
    let user_agent = request
        .headers()
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let forwarded_for = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let started = std::time::Instant::now();

    tracing::info!(
        component = "http",
        method = %method,
        path = %path,
        query = query.as_deref().unwrap_or(""),
        user_agent = user_agent.as_deref().unwrap_or(""),
        x_forwarded_for = forwarded_for.as_deref().unwrap_or(""),
        "http request started"
    );

    let response = next.run(request).await;
    let status = response.status();
    tracing::info!(
        component = "http",
        method = %method,
        path = %path,
        status = status.as_u16(),
        duration_ms = started.elapsed().as_millis() as u64,
        "http request completed"
    );
    response
}

async fn seed_configured_admin_openids(
    store: &Store,
    secrets: &EnvSecrets,
) -> Result<(), BridgeError> {
    for openid in &secrets.admin_openids {
        let openid = wechat::OpenId::new(openid);
        let openid_hash = wechat::OpenIdHash::sha256_for_display(&openid).to_string();
        store
            .upsert_whitelist(openid.as_str(), &openid_hash, "env-admin-openids")
            .await?;
        tracing::info!(
            component = "main",
            openid_hash = %openid_hash,
            "configured admin openid whitelisted"
        );
    }
    Ok(())
}

fn build_external_clients(
    config: &AppConfig,
    secrets: &EnvSecrets,
) -> Result<Arc<dyn ExternalClients>, BridgeError> {
    let Some(tencent_lbs_key) = secrets
        .tencent_lbs_key
        .as_deref()
        .filter(|key| !key.is_empty())
    else {
        tracing::warn!(
            component = "main",
            "TENCENT_LBS_KEY not configured; location and link jobs will use noop external clients"
        );
        return Ok(Arc::new(NoopExternalClients));
    };

    Ok(Arc::new(HttpExternalClients::new(
        TencentLbsOptions {
            endpoint: config.tencent_lbs_endpoint.clone(),
            key: tencent_lbs_key.to_string(),
            get_poi: config.tencent_lbs_get_poi,
            radius_meters: config.tencent_lbs_radius_meters,
        },
        JinaReaderOptions {
            endpoint: config.jina_reader_endpoint.clone(),
        },
        secrets.jina_api_key.clone(),
        config.http_timeout,
    )?))
}

fn build_media_processor(
    config: &AppConfig,
    secrets: &EnvSecrets,
) -> Result<Arc<dyn MediaJobProcessor>, BridgeError> {
    let Some(app_id) = secrets
        .wechat_app_id
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        tracing::warn!(
            component = "main",
            "WECHAT_APP_ID not configured; media jobs will use noop media processor"
        );
        return Ok(Arc::new(NoopMediaJobProcessor));
    };
    let Some(app_secret) = secrets
        .wechat_app_secret
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        tracing::warn!(
            component = "main",
            "WECHAT_APP_SECRET not configured; media jobs will use noop media processor"
        );
        return Ok(Arc::new(NoopMediaJobProcessor));
    };
    let Some(gemini_api_key) = secrets
        .gemini_api_key
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        tracing::warn!(
            component = "main",
            "GEMINI_API_KEY not configured; media jobs will use noop media processor"
        );
        return Ok(Arc::new(NoopMediaJobProcessor));
    };

    let media_client = WechatMediaClient::new(
        WechatApiConfig {
            api_base: config.wechat_api_base.clone(),
            app_id: app_id.to_string(),
            app_secret: app_secret.to_string(),
        },
        config.http_timeout,
        config.max_media_bytes,
    )?;
    let gemini_client = GeminiClient::new(
        GeminiConfig {
            endpoint_base: config.gemini_endpoint_base.clone(),
            api_key: gemini_api_key.to_string(),
            model: config.gemini_model.clone(),
            max_inline_bytes: config.gemini_max_inline_bytes,
        },
        config.http_timeout,
    )?;

    Ok(Arc::new(
        GeminiMediaJobProcessor::new(
            media_client,
            Arc::new(gemini_client),
            &config.raw_archive_dir,
            config.wechat_token_refresh_skew,
        )
        .with_prompts(
            config.llm_image_system_prompt.clone(),
            config.llm_voice_system_prompt.clone(),
            config.llm_video_system_prompt.clone(),
        ),
    ))
}

async fn run_worker_loop(
    worker: Worker,
    interval: std::time::Duration,
    processing_timeout: std::time::Duration,
) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        let now = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
        match worker
            .requeue_stale_processing_jobs(&now, processing_timeout)
            .await
        {
            Ok(count) if count > 0 => tracing::warn!(
                component = "worker",
                requeued_jobs = count,
                "stale processing jobs requeued"
            ),
            Ok(_) => {}
            Err(err) => tracing::warn!(
                component = "worker",
                error = %err,
                "failed to requeue stale processing jobs"
            ),
        }
        match worker.process_next(&now).await {
            Ok(WorkOutcome::Done {
                job_id,
                source_path,
            }) => tracing::info!(
                component = "worker",
                job_id,
                source_path = %source_path.display(),
                "worker job done"
            ),
            Ok(WorkOutcome::NoJob) => {}
            Err(err) => tracing::warn!(component = "worker", error = %err, "worker job failed"),
        }
    }
}

fn ensure_sqlite_parent(database_url: &str) -> Result<(), BridgeError> {
    let Some(path) = database_url.strip_prefix("sqlite://") else {
        return Ok(());
    };
    if path == ":memory:" {
        return Ok(());
    }
    if let Some(parent) = Path::new(path)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
