#[tokio::main]
async fn main() {
    if let Err(err) = sage_wiki_bridge::run().await {
        eprintln!("sage-wiki-bridge failed: {err}");
        std::process::exit(1);
    }
}
