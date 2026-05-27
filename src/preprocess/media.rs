use std::path::PathBuf;

use crate::preprocess::artifact::{ProcessedArtifact, ProcessedArtifactKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPreprocessOutput {
    pub message_key: String,
    pub kind: ProcessedArtifactKind,
    pub markdown_body: String,
    pub provider: String,
    pub model: String,
    pub raw_payload_paths: Vec<PathBuf>,
    pub processed_payload_paths: Vec<PathBuf>,
}

pub fn process_model_output(output: ModelPreprocessOutput) -> ProcessedArtifact {
    let mut artifact =
        ProcessedArtifact::new(output.message_key, output.kind, output.markdown_body);
    artifact.provider = Some(output.provider);
    artifact.model = Some(output.model);
    artifact.raw_payload_paths = output.raw_payload_paths;
    artifact.processed_payload_paths = output.processed_payload_paths;
    artifact
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_preprocessor_wraps_model_output() {
        let artifact = process_model_output(ModelPreprocessOutput {
            message_key: "msg_image".to_string(),
            kind: ProcessedArtifactKind::Image,
            markdown_body: "image description".to_string(),
            provider: "gemini".to_string(),
            model: "gemini-aistudio-configured".to_string(),
            raw_payload_paths: vec![PathBuf::from("data/raw/msg/media.jpg")],
            processed_payload_paths: vec![PathBuf::from("data/processed/msg/llm.response.json")],
        });

        assert_eq!(artifact.kind, ProcessedArtifactKind::Image);
        assert_eq!(artifact.provider.as_deref(), Some("gemini"));
        assert_eq!(
            artifact.model.as_deref(),
            Some("gemini-aistudio-configured")
        );
        assert_eq!(artifact.raw_payload_paths.len(), 1);
        assert_eq!(artifact.processed_payload_paths.len(), 1);
    }
}
