pub mod config;
pub mod error;
pub mod telemetry;
pub mod wechat;

pub fn run() -> Result<(), error::BridgeError> {
    dotenvy::dotenv().ok();
    telemetry::init();
    tracing::info!(component = "main", "sage-wiki-bridge bootstrap complete");
    Ok(())
}
