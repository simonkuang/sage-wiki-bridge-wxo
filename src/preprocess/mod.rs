pub mod artifact;
pub mod link;
pub mod location;
pub mod media;
pub mod text;

use crate::wechat::message::CommonFields;

fn message_key(common: &CommonFields, fallback_type: &str) -> String {
    common
        .msg_id
        .as_ref()
        .map(|msg_id| msg_id.as_str().to_string())
        .unwrap_or_else(|| format!("{}_{}", common.create_time, fallback_type))
}
