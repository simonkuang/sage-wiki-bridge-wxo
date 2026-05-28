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
pub mod store;
pub mod telemetry;
pub mod wechat;
pub mod worker;

use std::{env, path::Path, sync::Arc};

use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    admin::AdminState,
    archive::{ProcessedArtifactStore, RawArchive},
    config::{AppConfig, EnvSecrets, RuntimeConfig, runtime_config_from_args},
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
    store::Store,
    wechat::oauth::{WechatOAuthClient, WechatOAuthConfig},
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
    let Some(runtime_config) = runtime_config_from_args(args)? else {
        return Ok(());
    };
    run_with_config(runtime_config).await
}

pub async fn run_with_config(runtime_config: RuntimeConfig) -> Result<(), error::BridgeError> {
    let secrets = runtime_config.secrets;
    let config = runtime_config.app;
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
            request_body_limit_bytes: config.request_body_limit_bytes,
        },
        store: store.clone(),
        raw_archive: RawArchive::new(&config.raw_archive_dir, config.raw_archive_full),
    })
    .merge(admin::router(AdminState {
        store: store.clone(),
        base_path: config.admin_base_path.clone(),
        view_key: secrets.admin_view_key.clone(),
        whitelist_join_key: secrets.whitelist_join_key.clone(),
        whitelist_join_redirect_url: config.whitelist_join_redirect_url.clone(),
        oauth_client: build_oauth_client(&config, &secrets)?,
        default_per_page: config.admin_default_per_page,
        max_per_page: config.admin_max_per_page,
    }))
    .merge(health::router(HealthState {
        store,
        healthz_path: config.healthz_path.clone(),
        readyz_path: config.readyz_path.clone(),
    }));
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

fn build_oauth_client(
    config: &AppConfig,
    secrets: &EnvSecrets,
) -> Result<Option<WechatOAuthClient>, BridgeError> {
    let Some(app_id) = secrets
        .wechat_app_id
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let Some(app_secret) = secrets
        .wechat_app_secret
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    Ok(Some(WechatOAuthClient::new(
        WechatOAuthConfig {
            app_id: app_id.to_string(),
            app_secret: app_secret.to_string(),
            api_base: config.wechat_api_base.clone(),
            authorize_base: config.wechat_oauth_authorize_base.clone(),
        },
        config.http_timeout,
    )?))
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
