use std::{collections::HashMap, env, fs, path::Path};

use sage_wiki_bridge::wechat::crypto::{decrypt_callback_message, parse_encrypted_envelope};

fn main() {
    if let Err(err) = run() {
        eprintln!("wx_crypto_check failed: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        return Ok(());
    }

    let mut values = HashMap::new();
    let mut positionals = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--env-file" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err("--env-file requires a value".into());
                };
                values.extend(load_env_file(Path::new(path))?);
            }
            "--wechat-token" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("--wechat-token requires a value".into());
                };
                values.insert("WECHAT_TOKEN".to_string(), value.clone());
            }
            "--wechat-encoding-aes-key" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("--wechat-encoding-aes-key requires a value".into());
                };
                values.insert("WECHAT_ENCODING_AES_KEY".to_string(), value.clone());
            }
            arg if arg.starts_with("--") => return Err(format!("unknown option: {arg}").into()),
            _ => positionals.push(args[index].clone()),
        }
        index += 1;
    }

    if positionals.len() != 5 {
        print_usage();
        std::process::exit(2);
    }

    let token = values
        .get("WECHAT_TOKEN")
        .filter(|value| !value.trim().is_empty())
        .ok_or("WECHAT_TOKEN is required; pass --wechat-token or --env-file")?;
    let encoding_aes_key = values
        .get("WECHAT_ENCODING_AES_KEY")
        .filter(|value| !value.trim().is_empty())
        .ok_or(
            "WECHAT_ENCODING_AES_KEY is required; pass --wechat-encoding-aes-key or --env-file",
        )?;
    let encrypted_xml = fs::read_to_string(&positionals[0])?;
    let envelope = parse_encrypted_envelope(&encrypted_xml)?;
    let decrypted = decrypt_callback_message(
        token,
        encoding_aes_key,
        &positionals[4],
        &positionals[1],
        &positionals[2],
        &positionals[3],
        &envelope.encrypted_payload,
    )?;

    println!("{}", decrypted.xml);
    Ok(())
}

fn print_usage() {
    eprintln!(
        "usage: wx_crypto_check (--env-file PATH | --wechat-token TOKEN --wechat-encoding-aes-key KEY) <encrypted_xml_file> <timestamp> <nonce> <msg_signature> <expected_app_id>"
    );
}

fn load_env_file(path: &Path) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let mut values = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        values.insert(key.trim().to_string(), unquote(value.trim()).to_string());
    }
    Ok(values)
}

fn unquote(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}
