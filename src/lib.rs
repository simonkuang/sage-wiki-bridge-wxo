pub mod archive;
pub mod config;
pub mod enrich;
pub mod error;
pub mod media;
pub mod preprocess;
pub mod receiver;
pub mod source;
pub mod store;
pub mod telemetry;
pub mod wechat;
pub mod worker;

pub fn run() -> Result<(), error::BridgeError> {
    dotenvy::dotenv().ok();
    telemetry::init();
    tracing::info!(component = "main", "sage-wiki-bridge bootstrap complete");
    Ok(())
}
