use std::{env, fs};

use sage_wiki_bridge::wechat::crypto::{decrypt_callback_message, parse_encrypted_envelope};

fn main() {
    if let Err(err) = run() {
        eprintln!("wx_crypto_check failed: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().collect::<Vec<_>>();
    if args.len() != 6 {
        eprintln!(
            "usage: wx_crypto_check <encrypted_xml_file> <timestamp> <nonce> <msg_signature> <expected_app_id>"
        );
        std::process::exit(2);
    }

    dotenvy::dotenv().ok();
    let token = env::var("WECHAT_TOKEN")?;
    let encoding_aes_key = env::var("WECHAT_ENCODING_AES_KEY")?;
    let encrypted_xml = fs::read_to_string(&args[1])?;
    let envelope = parse_encrypted_envelope(&encrypted_xml)?;
    let decrypted = decrypt_callback_message(
        &token,
        &encoding_aes_key,
        &args[5],
        &args[2],
        &args[3],
        &args[4],
        &envelope.encrypted_payload,
    )?;

    println!("{}", decrypted.xml);
    Ok(())
}
