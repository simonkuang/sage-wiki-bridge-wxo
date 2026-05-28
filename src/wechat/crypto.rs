use aes::Aes256;
use base64::{
    Engine, alphabet,
    engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig, STANDARD},
};
use cbc::{
    Decryptor, Encryptor,
    cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7},
};
use quick_xml::de::from_str;
use serde::Deserialize;

use crate::{
    error::BridgeError,
    wechat::signature::{calculate_encrypted_signature, verify_encrypted_signature},
};

type Aes256CbcDecryptor = Decryptor<Aes256>;
type Aes256CbcEncryptor = Encryptor<Aes256>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedEnvelope {
    pub encrypted_payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecryptedMessage {
    pub xml: String,
    pub app_id: String,
}

#[derive(Debug, Deserialize)]
struct EncryptedEnvelopeXml {
    #[serde(rename = "Encrypt")]
    encrypt: String,
}

pub fn parse_encrypted_envelope(xml: &str) -> Result<EncryptedEnvelope, BridgeError> {
    let envelope: EncryptedEnvelopeXml =
        from_str(xml).map_err(|err| BridgeError::WechatXmlInvalid(err.to_string()))?;
    Ok(EncryptedEnvelope {
        encrypted_payload: envelope.encrypt,
    })
}

pub fn decrypt_callback_message(
    token: &str,
    encoding_aes_key: &str,
    expected_app_id: &str,
    timestamp: &str,
    nonce: &str,
    msg_signature: &str,
    encrypted_payload: &str,
) -> Result<DecryptedMessage, BridgeError> {
    if !verify_encrypted_signature(token, timestamp, nonce, encrypted_payload, msg_signature) {
        return Err(BridgeError::WechatSignatureInvalid);
    }

    let key = decode_encoding_aes_key(encoding_aes_key)?;
    let plaintext = decrypt_payload(&key, encrypted_payload)?;
    let (xml, app_id) = split_plaintext(&plaintext)?;
    if app_id != expected_app_id {
        return Err(BridgeError::Config(format!(
            "decrypted appid mismatch: expected {expected_app_id}, got {app_id}"
        )));
    }

    Ok(DecryptedMessage { xml, app_id })
}

pub fn encrypt_callback_message_for_test(
    token: &str,
    encoding_aes_key: &str,
    app_id: &str,
    timestamp: &str,
    nonce: &str,
    xml: &str,
) -> Result<(String, String), BridgeError> {
    let key = decode_encoding_aes_key(encoding_aes_key)?;
    let encrypted_payload = encrypt_payload(&key, xml, app_id)?;
    let signature = calculate_encrypted_signature(token, timestamp, nonce, &encrypted_payload);
    Ok((encrypted_payload, signature))
}

pub fn encrypt_reply_message(
    token: &str,
    encoding_aes_key: &str,
    app_id: &str,
    timestamp: &str,
    nonce: &str,
    xml: &str,
) -> Result<String, BridgeError> {
    let (encrypted_payload, signature) =
        encrypt_callback_message_for_test(token, encoding_aes_key, app_id, timestamp, nonce, xml)?;
    Ok(format!(
        "<xml><Encrypt><![CDATA[{encrypted_payload}]]></Encrypt><MsgSignature><![CDATA[{signature}]]></MsgSignature><TimeStamp>{timestamp}</TimeStamp><Nonce><![CDATA[{nonce}]]></Nonce></xml>"
    ))
}

fn decode_encoding_aes_key(encoding_aes_key: &str) -> Result<[u8; 32], BridgeError> {
    if encoding_aes_key.len() != 43 {
        return Err(BridgeError::Config(
            "WECHAT_ENCODING_AES_KEY must be 43 characters".to_string(),
        ));
    }
    let decoded = wechat_aes_key_engine()
        .decode(format!("{encoding_aes_key}="))
        .map_err(|err| BridgeError::Config(format!("invalid WECHAT_ENCODING_AES_KEY: {err}")))?;
    decoded
        .try_into()
        .map_err(|_| BridgeError::Config("WECHAT_ENCODING_AES_KEY must decode to 32 bytes".into()))
}

fn wechat_aes_key_engine() -> GeneralPurpose {
    GeneralPurpose::new(
        &alphabet::STANDARD,
        GeneralPurposeConfig::new().with_decode_allow_trailing_bits(true),
    )
}

fn decrypt_payload(key: &[u8; 32], encrypted_payload: &str) -> Result<Vec<u8>, BridgeError> {
    let encrypted = STANDARD
        .decode(encrypted_payload)
        .map_err(|err| BridgeError::ExternalPayloadInvalid(err.to_string()))?;
    Aes256CbcDecryptor::new(key.into(), (&key[..16]).into())
        .decrypt_padded_vec_mut::<Pkcs7>(&encrypted)
        .map_err(|err| BridgeError::ExternalPayloadInvalid(format!("decrypt failed: {err}")))
}

fn encrypt_payload(key: &[u8; 32], xml: &str, app_id: &str) -> Result<String, BridgeError> {
    let mut plaintext = Vec::with_capacity(20 + xml.len() + app_id.len());
    plaintext.extend_from_slice(b"0123456789abcdef");
    plaintext.extend_from_slice(&(xml.len() as u32).to_be_bytes());
    plaintext.extend_from_slice(xml.as_bytes());
    plaintext.extend_from_slice(app_id.as_bytes());
    let encrypted = Aes256CbcEncryptor::new(key.into(), (&key[..16]).into())
        .encrypt_padded_vec_mut::<Pkcs7>(&plaintext);
    Ok(STANDARD.encode(encrypted))
}

fn split_plaintext(plaintext: &[u8]) -> Result<(String, String), BridgeError> {
    if plaintext.len() < 20 {
        return Err(BridgeError::ExternalPayloadInvalid(
            "decrypted payload too short".to_string(),
        ));
    }
    let msg_len = u32::from_be_bytes(
        plaintext[16..20]
            .try_into()
            .map_err(|_| BridgeError::ExternalPayloadInvalid("message length missing".into()))?,
    ) as usize;
    let xml_start = 20;
    let xml_end = xml_start + msg_len;
    if plaintext.len() < xml_end {
        return Err(BridgeError::ExternalPayloadInvalid(
            "decrypted message length exceeds payload".to_string(),
        ));
    }
    let xml = String::from_utf8(plaintext[xml_start..xml_end].to_vec())
        .map_err(|err| BridgeError::ExternalPayloadInvalid(err.to_string()))?;
    let app_id = String::from_utf8(plaintext[xml_end..].to_vec())
        .map_err(|err| BridgeError::ExternalPayloadInvalid(err.to_string()))?;
    Ok((xml, app_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "bridge-token";
    const AES_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const APP_ID: &str = "wx1234567890abcdef";
    const TS: &str = "1780000000";
    const NONCE: &str = "nonce1";
    const XML: &str = "<xml><ToUserName><![CDATA[gh_bridge]]></ToUserName><FromUserName><![CDATA[openid-user-1]]></FromUserName><CreateTime>1780000001</CreateTime><MsgType><![CDATA[text]]></MsgType><Content><![CDATA[hello]]></Content><MsgId>1000000000000000001</MsgId></xml>";

    #[test]
    fn encrypted_callback_round_trip_decrypts_xml() {
        let (encrypted, signature) =
            encrypt_callback_message_for_test(TOKEN, AES_KEY, APP_ID, TS, NONCE, XML).unwrap();

        let decrypted =
            decrypt_callback_message(TOKEN, AES_KEY, APP_ID, TS, NONCE, &signature, &encrypted)
                .unwrap();

        assert_eq!(decrypted.xml, XML);
        assert_eq!(decrypted.app_id, APP_ID);
    }

    #[test]
    fn encrypted_callback_rejects_bad_signature() {
        let (encrypted, _signature) =
            encrypt_callback_message_for_test(TOKEN, AES_KEY, APP_ID, TS, NONCE, XML).unwrap();

        let err = decrypt_callback_message(TOKEN, AES_KEY, APP_ID, TS, NONCE, "bad", &encrypted)
            .unwrap_err();

        assert!(matches!(err, BridgeError::WechatSignatureInvalid));
    }

    #[test]
    fn decodes_wechat_aes_key_with_non_canonical_trailing_bits() {
        let key = format!("{}H", "A".repeat(42));

        let decoded = decode_encoding_aes_key(&key).unwrap();

        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn parses_encrypted_envelope() {
        let envelope =
            parse_encrypted_envelope("<xml><Encrypt><![CDATA[encrypted-body]]></Encrypt></xml>")
                .unwrap();

        assert_eq!(envelope.encrypted_payload, "encrypted-body");
    }

    #[test]
    fn encrypted_reply_round_trip_decrypts_xml() {
        let reply_xml = encrypt_reply_message(TOKEN, AES_KEY, APP_ID, TS, NONCE, XML).unwrap();
        let envelope = parse_encrypted_envelope(&reply_xml).unwrap();
        let signature = crate::wechat::signature::calculate_encrypted_signature(
            TOKEN,
            TS,
            NONCE,
            &envelope.encrypted_payload,
        );

        let decrypted = decrypt_callback_message(
            TOKEN,
            AES_KEY,
            APP_ID,
            TS,
            NONCE,
            &signature,
            &envelope.encrypted_payload,
        )
        .unwrap();

        assert_eq!(decrypted.xml, XML);
    }
}
