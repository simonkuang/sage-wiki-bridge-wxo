use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use serde::Serialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    config::{AppConfig, ConfigReportEntry, RuntimeConfigReport},
    error::BridgeError,
    store::{StatusSnapshot, Store},
};

#[derive(Debug, Clone)]
pub struct StatusContext {
    started_at: String,
    started_instant: Instant,
    bind_addr: String,
    database_url: String,
    worker_enabled: bool,
    callback_path: String,
    healthz_path: String,
    readyz_path: String,
    admin_base_path: String,
    source_dir: PathBuf,
    source_log_dir: PathBuf,
    raw_archive_dir: PathBuf,
    processed_artifact_dir: PathBuf,
    entries: Vec<ConfigReportEntry>,
}

impl StatusContext {
    pub fn from_report(report: &RuntimeConfigReport) -> Self {
        Self::from_config_and_entries(&report.runtime.app, report.entries.clone())
    }

    pub fn from_config_and_entries(config: &AppConfig, entries: Vec<ConfigReportEntry>) -> Self {
        let started_at = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "unknown".to_string());
        Self {
            started_at,
            started_instant: Instant::now(),
            bind_addr: config.bind_addr.clone(),
            database_url: config.database_url.clone(),
            worker_enabled: config.worker_enabled,
            callback_path: config.callback_path.clone(),
            healthz_path: config.healthz_path.clone(),
            readyz_path: config.readyz_path.clone(),
            admin_base_path: config.admin_base_path.clone(),
            source_dir: config.source_dir.clone(),
            source_log_dir: config.source_log_dir.clone(),
            raw_archive_dir: config.raw_archive_dir.clone(),
            processed_artifact_dir: config.processed_artifact_dir.clone(),
            entries,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceStatusReport {
    pub service: ServiceInfo,
    pub process: ProcessInfo,
    pub endpoints: EndpointInfo,
    pub config: Vec<ConfigReportEntry>,
    pub metrics: StatusSnapshot,
    pub runtime_checks: RuntimeChecks,
    pub usage: UsageInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceInfo {
    pub package_version: &'static str,
    pub started_at: String,
    pub uptime_seconds: u64,
    pub bind_addr: String,
    pub database_url: String,
    pub worker_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub os: &'static str,
    pub arch: &'static str,
    pub current_working_dir: Option<String>,
    pub memory_rss_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EndpointInfo {
    pub callback_path: String,
    pub healthz_path: String,
    pub readyz_path: String,
    pub admin_base_path: String,
    pub status_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeChecks {
    pub database_ready: bool,
    pub source_dir_exists: bool,
    pub source_dir_writable: bool,
    pub source_log_dir_exists: bool,
    pub source_log_dir_writable: bool,
    pub raw_archive_dir_exists: bool,
    pub raw_archive_dir_writable: bool,
    pub processed_artifact_dir_exists: bool,
    pub processed_artifact_dir_writable: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageInfo {
    pub token_usage_status: &'static str,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
}

pub async fn build_service_status(
    store: &Store,
    context: &StatusContext,
) -> Result<ServiceStatusReport, BridgeError> {
    let metrics = store.status_snapshot().await?;
    Ok(ServiceStatusReport {
        service: ServiceInfo {
            package_version: env!("CARGO_PKG_VERSION"),
            started_at: context.started_at.clone(),
            uptime_seconds: context.started_instant.elapsed().as_secs(),
            bind_addr: context.bind_addr.clone(),
            database_url: context.database_url.clone(),
            worker_enabled: context.worker_enabled,
        },
        process: ProcessInfo {
            pid: std::process::id(),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            current_working_dir: std::env::current_dir()
                .ok()
                .map(|path| path.display().to_string()),
            memory_rss_bytes: memory_rss_bytes(),
        },
        endpoints: EndpointInfo {
            callback_path: context.callback_path.clone(),
            healthz_path: context.healthz_path.clone(),
            readyz_path: context.readyz_path.clone(),
            admin_base_path: context.admin_base_path.clone(),
            status_path: format!("{}/status", context.admin_base_path),
        },
        config: context.entries.clone(),
        metrics,
        runtime_checks: RuntimeChecks {
            database_ready: store.ping().await.is_ok(),
            source_dir_exists: context.source_dir.is_dir(),
            source_dir_writable: dir_writable(&context.source_dir),
            source_log_dir_exists: context.source_log_dir.is_dir(),
            source_log_dir_writable: dir_writable(&context.source_log_dir),
            raw_archive_dir_exists: context.raw_archive_dir.is_dir(),
            raw_archive_dir_writable: dir_writable(&context.raw_archive_dir),
            processed_artifact_dir_exists: context.processed_artifact_dir.is_dir(),
            processed_artifact_dir_writable: dir_writable(&context.processed_artifact_dir),
        },
        usage: UsageInfo {
            token_usage_status: "not_tracked",
            input_tokens: None,
            output_tokens: None,
        },
    })
}

fn dir_writable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_dir() && !metadata.permissions().readonly())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn memory_rss_bytes() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let value = line.strip_prefix("VmRSS:")?.trim();
        let kb = value.split_whitespace().next()?.parse::<u64>().ok()?;
        Some(kb * 1024)
    })
}

#[cfg(not(target_os = "linux"))]
fn memory_rss_bytes() -> Option<u64> {
    None
}
