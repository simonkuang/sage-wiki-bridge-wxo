use crate::{
    preprocess::{artifact::ProcessedArtifact, artifact::ProcessedArtifactKind, message_key},
    wechat::message::TextMessage,
};

pub fn process_text(message: &TextMessage) -> ProcessedArtifact {
    ProcessedArtifact::new(
        message_key(&message.common, "text"),
        ProcessedArtifactKind::Text,
        message.content.clone(),
    )
}

#[cfg(test)]
mod tests {
    use crate::wechat::{IncomingMessage, parse_plain_message};

    use super::*;

    #[test]
    fn text_preprocessor_passes_content_through() {
        let xml = include_str!("../../tests/fixtures/wechat/text.xml");
        let message = parse_plain_message(xml).unwrap();
        let IncomingMessage::Text(text) = message else {
            panic!("expected text");
        };

        let artifact = process_text(&text);

        assert_eq!(artifact.message_key, "1000000000000000001");
        assert_eq!(artifact.kind, ProcessedArtifactKind::Text);
        assert_eq!(artifact.markdown_body, "把这条知识存进 sage-wiki");
    }
}
