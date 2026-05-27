fn main() {
    if let Err(err) = sage_wiki_bridge::run() {
        eprintln!("sage-wiki-bridge failed: {err}");
        std::process::exit(1);
    }
}
