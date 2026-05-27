pub mod archive;
pub mod config;
pub mod enrich;
pub mod error;
pub mod preprocess;
pub mod source;
pub mod telemetry;
pub mod wechat;

pub fn run() -> Result<(), error::BridgeError> {
    dotenvy::dotenv().ok();
    telemetry::init();
    tracing::info!(component = "main", "sage-wiki-bridge bootstrap complete");
    Ok(())
}
