use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("wechat signature invalid")]
    WechatSignatureInvalid,

    #[error("wechat xml invalid: {0}")]
    WechatXmlInvalid(String),

    #[error("unsupported message type: {0}")]
    MessageUnsupported(String),

    #[error("url not allowed: {0}")]
    UrlNotAllowed(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("path escapes configured root: {0}")]
    PathOutsideRoot(String),
}
