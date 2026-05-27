use std::path::PathBuf;

use crate::{
    preprocess::{artifact::ProcessedArtifact, artifact::ProcessedArtifactKind, message_key},
    wechat::message::LinkMessage,
};

pub fn process_link(
    message: &LinkMessage,
    reader_markdown: &str,
    reader_payload_path: Option<PathBuf>,
) -> ProcessedArtifact {
    let mut body = String::new();
    body.push_str("## Original Link\n\n");
    if let Some(title) = &message.title {
        body.push_str(&format!("- Title: {title}\n"));
    }
    if let Some(description) = &message.description {
        body.push_str(&format!("- Description: {description}\n"));
    }
    body.push_str(&format!("- URL: {}\n\n", message.url.as_str()));
    body.push_str("## Reader Content\n\n");
    body.push_str(reader_markdown.trim());
    body.push('\n');

    let mut artifact = ProcessedArtifact::new(
        message_key(&message.common, "link"),
        ProcessedArtifactKind::Link,
        body,
    );
    artifact.external_service = Some("jina_reader".to_string());
    artifact.summary = message.title.clone();
    if let Some(path) = reader_payload_path {
        artifact.processed_payload_paths.push(path);
    }
    artifact
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::wechat::{IncomingMessage, parse_plain_message};

    use super::*;

    #[test]
    fn link_preprocessor_replaces_url_with_reader_content() {
        let xml = include_str!("../../tests/fixtures/wechat/link.xml");
        let message = parse_plain_message(xml).unwrap();
        let IncomingMessage::Link(link) = message else {
            panic!("expected link");
        };

        let artifact = process_link(
            &link,
            include_str!("../../tests/fixtures/external/jina_reader_success.md"),
            Some(PathBuf::from("data/processed/msg/link.jina-reader.md")),
        );

        assert_eq!(artifact.kind, ProcessedArtifactKind::Link);
        assert_eq!(artifact.external_service.as_deref(), Some("jina_reader"));
        assert!(artifact.markdown_body.contains("## Reader Content"));
        assert!(
            artifact
                .markdown_body
                .contains("这是 Jina Reader 返回的 Markdown 内容")
        );
        assert_eq!(artifact.processed_payload_paths.len(), 1);
    }
}
