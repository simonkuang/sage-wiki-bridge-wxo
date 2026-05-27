use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpenId(String);

impl OpenId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WechatMsgId(String);

impl WechatMsgId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MediaId(String);

impl MediaId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UrlString(String);

impl UrlString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpenIdHash(String);

impl OpenIdHash {
    pub fn sha256_for_display(openid: &OpenId) -> Self {
        use sha2::{Digest, Sha256};

        let digest = Sha256::digest(openid.as_str().as_bytes());
        Self(format!("sha256:{digest:x}"))
    }
}

impl fmt::Display for OpenIdHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
