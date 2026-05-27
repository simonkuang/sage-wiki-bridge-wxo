use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{error::BridgeError, preprocess::artifact::ProcessedArtifact};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMetadata {
    pub wechat_msg_id: Option<String>,
    pub message_type: String,
    pub received_at: String,
    pub wechat_create_time: Option<i64>,
    pub openid_hash: String,
    pub raw_dir: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub external_service: Option<String>,
    pub bridge_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceWriteResult {
    pub path: PathBuf,
    pub bytes_written: u64,
}

#[derive(Debug, Clone)]
pub struct SourceWriter {
    root: PathBuf,
}

impl SourceWriter {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn write_source(
        &self,
        artifact: &ProcessedArtifact,
        metadata: &SourceMetadata,
    ) -> Result<SourceWriteResult, BridgeError> {
        let filename = format!("{}.md", sanitize_segment(&artifact.message_key)?);
        let path = self.root.join(filename);
        ensure_under_root(&self.root, &path)?;

        let markdown = render_markdown(artifact, metadata);
        write_file_atomically(&path, markdown.as_bytes())?;

        Ok(SourceWriteResult {
            path,
            bytes_written: markdown.len() as u64,
        })
    }
}

fn render_markdown(artifact: &ProcessedArtifact, metadata: &SourceMetadata) -> String {
    let mut frontmatter = String::new();
    frontmatter.push_str("---\n");
    push_yaml_str(&mut frontmatter, "source", "wechat-official-account");
    push_yaml_str(&mut frontmatter, "source_type", "wechat_message");
    if let Some(msg_id) = &metadata.wechat_msg_id {
        push_yaml_str(&mut frontmatter, "wechat_msg_id", msg_id);
    }
    push_yaml_str(&mut frontmatter, "message_type", &metadata.message_type);
    push_yaml_str(&mut frontmatter, "received_at", &metadata.received_at);
    if let Some(create_time) = metadata.wechat_create_time {
        frontmatter.push_str(&format!("wechat_create_time: {create_time}\n"));
    }
    push_yaml_str(&mut frontmatter, "openid_hash", &metadata.openid_hash);
    if let Some(provider) = &metadata.provider {
        push_yaml_str(&mut frontmatter, "provider", provider);
    }
    if let Some(model) = &metadata.model {
        push_yaml_str(&mut frontmatter, "model", model);
    }
    if let Some(service) = &metadata.external_service {
        push_yaml_str(&mut frontmatter, "external_service", service);
    }
    if let Some(raw_dir) = &metadata.raw_dir {
        push_yaml_str(&mut frontmatter, "raw_dir", raw_dir);
    }
    push_yaml_str(&mut frontmatter, "bridge_version", &metadata.bridge_version);
    frontmatter.push_str("---\n\n");

    format!(
        "{frontmatter}# WeChat capture {}\n\n## Processed Content\n\n{}\n",
        metadata.received_at,
        artifact.markdown_body.trim()
    )
}

fn push_yaml_str(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push_str(": ");
    out.push('"');
    out.push_str(&value.replace('\\', "\\\\").replace('"', "\\\""));
    out.push_str("\"\n");
}

fn sanitize_segment(segment: &str) -> Result<String, BridgeError> {
    if segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.contains('/')
        || segment.contains('\\')
    {
        return Err(BridgeError::PathOutsideRoot(segment.to_string()));
    }

    Ok(segment
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect())
}

fn ensure_under_root(root: &Path, path: &Path) -> Result<(), BridgeError> {
    let root_components = root.components().collect::<Vec<_>>();
    let path_components = path.components().collect::<Vec<_>>();

    if path_components.starts_with(&root_components) {
        Ok(())
    } else {
        Err(BridgeError::PathOutsideRoot(path.display().to_string()))
    }
}

fn write_file_atomically(path: &Path, bytes: &[u8]) -> Result<(), BridgeError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = path.with_extension("md.tmp");
    {
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::preprocess::artifact::{ProcessedArtifact, ProcessedArtifactKind};

    use super::*;

    fn metadata() -> SourceMetadata {
        SourceMetadata {
            wechat_msg_id: Some("1000000000000000001".to_string()),
            message_type: "text".to_string(),
            received_at: "2026-05-27T21:30:15+08:00".to_string(),
            wechat_create_time: Some(1780000001),
            openid_hash: "sha256:abc".to_string(),
            raw_dir: Some("data/raw/msg_1".to_string()),
            provider: None,
            model: None,
            external_service: None,
            bridge_version: "0.1.0".to_string(),
        }
    }

    #[test]
    fn writes_markdown_source_atomically() {
        let temp = tempfile::tempdir().unwrap();
        let writer = SourceWriter::new(temp.path());
        let artifact = ProcessedArtifact::new(
            "20260527T133015Z_1000000000000000001",
            ProcessedArtifactKind::Text,
            "hello sage-wiki",
        );

        let result = writer.write_source(&artifact, &metadata()).unwrap();
        let content = fs::read_to_string(result.path).unwrap();

        assert!(content.contains("source: \"wechat-official-account\""));
        assert!(content.contains("message_type: \"text\""));
        assert!(content.contains("## Processed Content"));
        assert!(content.contains("hello sage-wiki"));
    }

    #[test]
    fn rejects_unsafe_message_key() {
        let temp = tempfile::tempdir().unwrap();
        let writer = SourceWriter::new(temp.path());
        let artifact = ProcessedArtifact::new("../escape", ProcessedArtifactKind::Text, "bad path");

        let err = writer.write_source(&artifact, &metadata()).unwrap_err();

        assert!(matches!(err, BridgeError::PathOutsideRoot(_)));
    }

    #[test]
    fn escapes_yaml_string_values() {
        let artifact = ProcessedArtifact::new("msg_1", ProcessedArtifactKind::Text, "body");
        let mut metadata = metadata();
        metadata.message_type = "text\"quoted".to_string();

        let markdown = render_markdown(&artifact, &metadata);

        assert!(markdown.contains("message_type: \"text\\\"quoted\""));
    }
}
