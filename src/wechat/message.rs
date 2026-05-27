use quick_xml::de::from_str;
use serde::Deserialize;

use crate::error::BridgeError;

use super::types::{MediaId, OpenId, UrlString, WechatMsgId};

#[derive(Debug, Clone, PartialEq)]
pub enum IncomingMessage {
    Text(TextMessage),
    Image(ImageMessage),
    Voice(VoiceMessage),
    Video(VideoMessage),
    ShortVideo(VideoMessage),
    Location(LocationMessage),
    Link(LinkMessage),
    Unsupported(UnsupportedMessage),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFields {
    pub to_user_name: String,
    pub from_user_name: OpenId,
    pub create_time: i64,
    pub msg_id: Option<WechatMsgId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMessage {
    pub common: CommonFields,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageMessage {
    pub common: CommonFields,
    pub pic_url: Option<String>,
    pub media_id: MediaId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceMessage {
    pub common: CommonFields,
    pub media_id: MediaId,
    pub format: Option<String>,
    pub recognition: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoMessage {
    pub common: CommonFields,
    pub media_id: MediaId,
    pub thumb_media_id: Option<MediaId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocationMessage {
    pub common: CommonFields,
    pub latitude: f64,
    pub longitude: f64,
    pub scale: Option<i32>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkMessage {
    pub common: CommonFields,
    pub title: Option<String>,
    pub description: Option<String>,
    pub url: UrlString,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedMessage {
    pub common: CommonFields,
    pub msg_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename = "xml")]
struct RawWechatMessage {
    #[serde(rename = "ToUserName")]
    to_user_name: String,
    #[serde(rename = "FromUserName")]
    from_user_name: String,
    #[serde(rename = "CreateTime")]
    create_time: i64,
    #[serde(rename = "MsgType")]
    msg_type: String,
    #[serde(rename = "MsgId")]
    msg_id: Option<String>,
    #[serde(rename = "Content")]
    content: Option<String>,
    #[serde(rename = "PicUrl")]
    pic_url: Option<String>,
    #[serde(rename = "MediaId")]
    media_id: Option<String>,
    #[serde(rename = "Format")]
    format: Option<String>,
    #[serde(rename = "Recognition")]
    recognition: Option<String>,
    #[serde(rename = "ThumbMediaId")]
    thumb_media_id: Option<String>,
    #[serde(rename = "Location_X")]
    location_x: Option<f64>,
    #[serde(rename = "Location_Y")]
    location_y: Option<f64>,
    #[serde(rename = "Scale")]
    scale: Option<i32>,
    #[serde(rename = "Label")]
    label: Option<String>,
    #[serde(rename = "Title")]
    title: Option<String>,
    #[serde(rename = "Description")]
    description: Option<String>,
    #[serde(rename = "Url")]
    url: Option<String>,
}

pub fn parse_plain_message(xml: &str) -> Result<IncomingMessage, BridgeError> {
    let raw: RawWechatMessage =
        from_str(xml).map_err(|err| BridgeError::WechatXmlInvalid(err.to_string()))?;
    raw.try_into()
}

impl TryFrom<RawWechatMessage> for IncomingMessage {
    type Error = BridgeError;

    fn try_from(raw: RawWechatMessage) -> Result<Self, Self::Error> {
        let common = CommonFields {
            to_user_name: raw.to_user_name,
            from_user_name: OpenId::new(raw.from_user_name),
            create_time: raw.create_time,
            msg_id: raw.msg_id.map(WechatMsgId::new),
        };

        match raw.msg_type.as_str() {
            "text" => Ok(Self::Text(TextMessage {
                common,
                content: required(raw.content, "Content")?,
            })),
            "image" => Ok(Self::Image(ImageMessage {
                common,
                pic_url: raw.pic_url,
                media_id: MediaId::new(required(raw.media_id, "MediaId")?),
            })),
            "voice" => Ok(Self::Voice(VoiceMessage {
                common,
                media_id: MediaId::new(required(raw.media_id, "MediaId")?),
                format: raw.format,
                recognition: raw.recognition,
            })),
            "video" => Ok(Self::Video(VideoMessage {
                common,
                media_id: MediaId::new(required(raw.media_id, "MediaId")?),
                thumb_media_id: raw.thumb_media_id.map(MediaId::new),
            })),
            "shortvideo" => Ok(Self::ShortVideo(VideoMessage {
                common,
                media_id: MediaId::new(required(raw.media_id, "MediaId")?),
                thumb_media_id: raw.thumb_media_id.map(MediaId::new),
            })),
            "location" => Ok(Self::Location(LocationMessage {
                common,
                latitude: required(raw.location_x, "Location_X")?,
                longitude: required(raw.location_y, "Location_Y")?,
                scale: raw.scale,
                label: raw.label,
            })),
            "link" => Ok(Self::Link(LinkMessage {
                common,
                title: raw.title,
                description: raw.description,
                url: UrlString::new(required(raw.url, "Url")?),
            })),
            other => Ok(Self::Unsupported(UnsupportedMessage {
                common,
                msg_type: other.to_string(),
            })),
        }
    }
}

fn required<T>(value: Option<T>, field: &str) -> Result<T, BridgeError> {
    value.ok_or_else(|| BridgeError::WechatXmlInvalid(format!("missing required field {field}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> &'static str {
        match name {
            "text" => include_str!("../../tests/fixtures/wechat/text.xml"),
            "image" => include_str!("../../tests/fixtures/wechat/image.xml"),
            "voice" => include_str!("../../tests/fixtures/wechat/voice.xml"),
            "video" => include_str!("../../tests/fixtures/wechat/video.xml"),
            "shortvideo" => include_str!("../../tests/fixtures/wechat/shortvideo.xml"),
            "location" => include_str!("../../tests/fixtures/wechat/location.xml"),
            "link" => include_str!("../../tests/fixtures/wechat/link.xml"),
            "unsupported" => include_str!("../../tests/fixtures/wechat/unsupported.xml"),
            "malformed" => include_str!("../../tests/fixtures/wechat/malformed.xml"),
            _ => panic!("unknown fixture {name}"),
        }
    }

    #[test]
    fn parses_text_message() {
        let msg = parse_plain_message(fixture("text")).unwrap();
        match msg {
            IncomingMessage::Text(text) => {
                assert_eq!(text.common.from_user_name.as_str(), "openid-user-1");
                assert_eq!(text.content, "把这条知识存进 sage-wiki");
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn parses_image_message() {
        let msg = parse_plain_message(fixture("image")).unwrap();
        match msg {
            IncomingMessage::Image(image) => {
                assert_eq!(image.media_id.as_str(), "media-image-1");
                assert_eq!(
                    image.pic_url.as_deref(),
                    Some("https://mmbiz.qpic.cn/example.jpg")
                );
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn parses_voice_message() {
        let msg = parse_plain_message(fixture("voice")).unwrap();
        match msg {
            IncomingMessage::Voice(voice) => {
                assert_eq!(voice.media_id.as_str(), "media-voice-1");
                assert_eq!(voice.format.as_deref(), Some("amr"));
                assert_eq!(voice.recognition.as_deref(), Some("这是一段语音识别结果"));
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn parses_video_message() {
        let msg = parse_plain_message(fixture("video")).unwrap();
        match msg {
            IncomingMessage::Video(video) => {
                assert_eq!(video.media_id.as_str(), "media-video-1");
                assert_eq!(video.thumb_media_id.unwrap().as_str(), "media-thumb-1");
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn parses_shortvideo_message() {
        let msg = parse_plain_message(fixture("shortvideo")).unwrap();
        match msg {
            IncomingMessage::ShortVideo(video) => {
                assert_eq!(video.media_id.as_str(), "media-shortvideo-1");
                assert_eq!(
                    video.thumb_media_id.unwrap().as_str(),
                    "media-short-thumb-1"
                );
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn parses_location_message_with_latitude_then_longitude() {
        let msg = parse_plain_message(fixture("location")).unwrap();
        match msg {
            IncomingMessage::Location(location) => {
                assert_eq!(location.latitude, 23.134521);
                assert_eq!(location.longitude, 113.358803);
                assert_eq!(location.scale, Some(16));
                assert_eq!(location.label.as_deref(), Some("广东省广州市天河区示例路"));
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn parses_link_message() {
        let msg = parse_plain_message(fixture("link")).unwrap();
        match msg {
            IncomingMessage::Link(link) => {
                assert_eq!(link.title.as_deref(), Some("示例文章"));
                assert_eq!(link.url.as_str(), "https://example.com/article");
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn preserves_unsupported_message() {
        let msg = parse_plain_message(fixture("unsupported")).unwrap();
        match msg {
            IncomingMessage::Unsupported(unsupported) => {
                assert_eq!(unsupported.msg_type, "event");
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn rejects_malformed_xml() {
        let err = parse_plain_message(fixture("malformed")).unwrap_err();
        assert!(matches!(err, BridgeError::WechatXmlInvalid(_)));
    }
}
