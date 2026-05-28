use std::sync::OnceLock;

use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

static INIT: OnceLock<()> = OnceLock::new();

pub fn init(filter: &str) {
    INIT.get_or_init(|| {
        let filter = EnvFilter::try_new(filter)
            .unwrap_or_else(|_| EnvFilter::new("info,sage_wiki_bridge=debug"));

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().json().with_current_span(false))
            .init();
    });
}
