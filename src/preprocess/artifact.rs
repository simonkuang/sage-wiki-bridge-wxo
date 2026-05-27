use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessedArtifactKind {
    Text,
    Image,
    Voice,
    Video,
    ShortVideo,
    Location,
    Link,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedArtifact {
    pub message_key: String,
    pub kind: ProcessedArtifactKind,
    pub markdown_body: String,
    pub summary: Option<String>,
    pub raw_payload_paths: Vec<PathBuf>,
    pub processed_payload_paths: Vec<PathBuf>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub external_service: Option<String>,
}

impl ProcessedArtifact {
    pub fn new(
        message_key: impl Into<String>,
        kind: ProcessedArtifactKind,
        markdown_body: impl Into<String>,
    ) -> Self {
        Self {
            message_key: message_key.into(),
            kind,
            markdown_body: markdown_body.into(),
            summary: None,
            raw_payload_paths: Vec::new(),
            processed_payload_paths: Vec::new(),
            provider: None,
            model: None,
            external_service: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_minimal_artifact() {
        let artifact = ProcessedArtifact::new("msg_1", ProcessedArtifactKind::Text, "hello");

        assert_eq!(artifact.message_key, "msg_1");
        assert_eq!(artifact.kind, ProcessedArtifactKind::Text);
        assert_eq!(artifact.markdown_body, "hello");
        assert!(artifact.provider.is_none());
    }
}
