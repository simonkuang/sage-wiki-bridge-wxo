use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{error::BridgeError, preprocess::artifact::ProcessedArtifact};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMetadata {
    pub wechat_msg_id: Option<String>,
    pub thread_id: Option<String>,
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
    format: SourceFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceFormat {
    Ai,
    DailyLog,
}

impl SourceWriter {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::ai(root)
    }

    pub fn ai(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            format: SourceFormat::Ai,
        }
    }

    pub fn daily_log(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            format: SourceFormat::DailyLog,
        }
    }

    pub fn write_source(
        &self,
        artifact: &ProcessedArtifact,
        metadata: &SourceMetadata,
    ) -> Result<SourceWriteResult, BridgeError> {
        let message_key = sanitize_segment(&artifact.message_key)?;
        let capture_date = capture_date_from_received_at(&metadata.received_at)?;
        let filename = format!("{capture_date}.md");
        let path = self.root.join(filename);
        ensure_under_root(&self.root, &path)?;

        let current = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => match self.format {
                SourceFormat::Ai => render_ai_document_header(&capture_date),
                SourceFormat::DailyLog => render_daily_log_document_header(&capture_date, metadata),
            },
            Err(err) => return Err(err.into()),
        };
        let markdown = match self.format {
            SourceFormat::Ai => {
                let thread_id =
                    sanitize_segment(metadata.thread_id.as_deref().unwrap_or(&message_key))?;
                let item = render_ai_item(&message_key, artifact, metadata);
                upsert_ai_thread_item(&current, &thread_id, &message_key, &item)
            }
            SourceFormat::DailyLog => {
                let entry = render_daily_log_entry(&message_key, artifact, metadata);
                upsert_daily_entry(&current, &message_key, &entry)
            }
        };
        write_file_atomically(&path, markdown.as_bytes())?;

        Ok(SourceWriteResult {
            path,
            bytes_written: markdown.len() as u64,
        })
    }
}

fn render_ai_document_header(capture_date: &str) -> String {
    let mut frontmatter = String::new();
    frontmatter.push_str("---\n");
    push_yaml_str(&mut frontmatter, "source", "wechat-official-account");
    push_yaml_str(&mut frontmatter, "source_type", "wechat_ai_messages");
    push_yaml_str(&mut frontmatter, "capture_date", capture_date);
    frontmatter.push_str("---\n\n");

    format!("{frontmatter}# WeChat Knowledge {capture_date}\n")
}

fn render_daily_log_document_header(capture_date: &str, metadata: &SourceMetadata) -> String {
    let mut frontmatter = String::new();
    frontmatter.push_str("---\n");
    push_yaml_str(&mut frontmatter, "source", "wechat-official-account");
    push_yaml_str(&mut frontmatter, "source_type", "wechat_daily_messages");
    push_yaml_str(&mut frontmatter, "capture_date", capture_date);
    push_yaml_str(&mut frontmatter, "bridge_version", &metadata.bridge_version);
    frontmatter.push_str("---\n\n");

    format!("{frontmatter}# WeChat captures {capture_date}\n")
}

fn render_ai_item(
    sanitized_message_key: &str,
    artifact: &ProcessedArtifact,
    metadata: &SourceMetadata,
) -> String {
    let mut entry = String::new();
    entry.push_str(&format!(
        "<!-- swb:item:start:{sanitized_message_key} -->\n"
    ));
    entry.push_str(&format!(
        "[{} {}]\n\n",
        metadata.received_at, metadata.message_type
    ));
    entry.push_str(artifact.markdown_body.trim());
    entry.push_str("\n\n");
    if let Some(service) = &metadata.external_service {
        entry.push_str(&format!("_via {service}_\n\n"));
    }
    entry.push_str(&format!("<!-- swb:item:end:{sanitized_message_key} -->\n"));
    entry
}

fn render_daily_log_entry(
    sanitized_message_key: &str,
    artifact: &ProcessedArtifact,
    metadata: &SourceMetadata,
) -> String {
    let mut entry = String::new();
    entry.push_str(&format!(
        "<!-- sage-wiki-bridge-message-start:{sanitized_message_key} -->\n\n"
    ));
    entry.push_str(&format!(
        "## {} {}\n\n",
        metadata.received_at, metadata.message_type
    ));
    entry.push_str("### Metadata\n\n");
    if let Some(msg_id) = &metadata.wechat_msg_id {
        push_bullet_str(&mut entry, "wechat_msg_id", msg_id);
    }
    push_bullet_str(&mut entry, "message_key", sanitized_message_key);
    push_bullet_str(&mut entry, "message_type", &metadata.message_type);
    push_bullet_str(&mut entry, "received_at", &metadata.received_at);
    if let Some(create_time) = metadata.wechat_create_time {
        entry.push_str(&format!("- wechat_create_time: `{create_time}`\n"));
    }
    push_bullet_str(&mut entry, "openid_hash", &metadata.openid_hash);
    if let Some(provider) = &metadata.provider {
        push_bullet_str(&mut entry, "provider", provider);
    }
    if let Some(model) = &metadata.model {
        push_bullet_str(&mut entry, "model", model);
    }
    if let Some(service) = &metadata.external_service {
        push_bullet_str(&mut entry, "external_service", service);
    }
    if let Some(raw_dir) = &metadata.raw_dir {
        push_bullet_str(&mut entry, "raw_dir", raw_dir);
    }
    push_bullet_str(&mut entry, "bridge_version", &metadata.bridge_version);
    entry.push_str("\n### Processed Content\n\n");
    entry.push_str(artifact.markdown_body.trim());
    entry.push_str("\n\n");
    entry.push_str(&format!(
        "<!-- sage-wiki-bridge-message-end:{sanitized_message_key} -->\n"
    ));

    entry
}

fn upsert_daily_entry(current: &str, message_key: &str, entry: &str) -> String {
    let start_marker = format!("<!-- sage-wiki-bridge-message-start:{message_key} -->");
    let end_marker = format!("<!-- sage-wiki-bridge-message-end:{message_key} -->");
    let Some(start) = current.find(&start_marker) else {
        let mut next = current.trim_end().to_string();
        next.push_str("\n\n");
        next.push_str(entry.trim_end());
        next.push('\n');
        return next;
    };
    let Some(relative_end) = current[start..].find(&end_marker) else {
        let mut next = current.trim_end().to_string();
        next.push_str("\n\n");
        next.push_str(entry.trim_end());
        next.push('\n');
        return next;
    };
    let end = start + relative_end + end_marker.len();
    let mut next = String::new();
    next.push_str(current[..start].trim_end());
    next.push_str("\n\n");
    next.push_str(entry.trim_end());
    next.push('\n');
    next.push_str(current[end..].trim_start_matches('\n'));
    next
}

fn upsert_ai_thread_item(current: &str, thread_id: &str, message_key: &str, item: &str) -> String {
    let thread_start_marker = format!("<!-- swb:thread v=1 id={thread_id} -->");
    let thread_end_marker = "<!-- /swb:thread -->";
    let item_start_marker = format!("<!-- swb:item:start:{message_key} -->");
    let item_end_marker = format!("<!-- swb:item:end:{message_key} -->");

    let Some(thread_start) = current.find(&thread_start_marker) else {
        let mut next = current.trim_end().to_string();
        next.push_str("\n\n");
        next.push_str(&thread_start_marker);
        next.push_str("\n<<< wechat-thread >>>\n\n");
        next.push_str(item.trim_end());
        next.push_str("\n\n<<< /wechat-thread >>>\n");
        next.push_str(thread_end_marker);
        next.push('\n');
        return next;
    };

    let Some(relative_thread_end) = current[thread_start..].find(thread_end_marker) else {
        let mut next = current.trim_end().to_string();
        next.push_str("\n\n");
        next.push_str(&thread_start_marker);
        next.push_str("\n<<< wechat-thread >>>\n\n");
        next.push_str(item.trim_end());
        next.push_str("\n\n<<< /wechat-thread >>>\n");
        next.push_str(thread_end_marker);
        next.push('\n');
        return next;
    };
    let thread_end = thread_start + relative_thread_end;
    let thread_body = &current[thread_start..thread_end];

    if let Some(relative_item_start) = thread_body.find(&item_start_marker) {
        if let Some(relative_item_end) = thread_body[relative_item_start..].find(&item_end_marker) {
            let item_start = thread_start + relative_item_start;
            let item_end =
                thread_start + relative_item_start + relative_item_end + item_end_marker.len();
            let mut next = String::new();
            next.push_str(current[..item_start].trim_end());
            next.push_str("\n");
            next.push_str(item.trim_end());
            next.push('\n');
            next.push_str(current[item_end..].trim_start_matches('\n'));
            return next;
        }
    }

    let insert_at = thread_body
        .find("<<< /wechat-thread >>>")
        .map(|relative| thread_start + relative)
        .unwrap_or(thread_end);
    let mut next = String::new();
    next.push_str(current[..insert_at].trim_end());
    next.push_str("\n\n");
    next.push_str(item.trim_end());
    next.push_str("\n\n");
    next.push_str(current[insert_at..].trim_start_matches('\n'));
    next
}

#[cfg(test)]
fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

fn push_yaml_str(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push_str(": ");
    out.push('"');
    out.push_str(&value.replace('\\', "\\\\").replace('"', "\\\""));
    out.push_str("\"\n");
}

fn push_bullet_str(out: &mut String, key: &str, value: &str) {
    out.push_str("- ");
    out.push_str(key);
    out.push_str(": \"");
    out.push_str(&value.replace('\\', "\\\\").replace('"', "\\\""));
    out.push_str("\"\n");
}

fn capture_date_from_received_at(received_at: &str) -> Result<String, BridgeError> {
    let Some(date) = received_at.get(0..10) else {
        return Err(BridgeError::Config(format!(
            "received_at is too short for daily source path: {received_at}"
        )));
    };
    let valid = date.chars().enumerate().all(|(index, ch)| {
        if matches!(index, 4 | 7) {
            ch == '-'
        } else {
            ch.is_ascii_digit()
        }
    });
    if !valid {
        return Err(BridgeError::Config(format!(
            "received_at does not start with YYYY-MM-DD: {received_at}"
        )));
    }
    sanitize_segment(date)
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
            thread_id: None,
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
        let writer = SourceWriter::daily_log(temp.path());
        let artifact = ProcessedArtifact::new(
            "20260527T133015Z_1000000000000000001",
            ProcessedArtifactKind::Text,
            "hello sage-wiki",
        );

        let result = writer.write_source(&artifact, &metadata()).unwrap();
        let content = fs::read_to_string(&result.path).unwrap();

        assert_eq!(result.path.file_name().unwrap(), "2026-05-27.md");
        assert!(content.contains("source: \"wechat-official-account\""));
        assert!(content.contains("source_type: \"wechat_daily_messages\""));
        assert!(content.contains("capture_date: \"2026-05-27\""));
        assert!(content.contains("message_type: \"text\""));
        assert!(content.contains("### Processed Content"));
        assert!(content.contains("hello sage-wiki"));
        assert!(content.contains(
            "<!-- sage-wiki-bridge-message-start:20260527T133015Z_1000000000000000001 -->"
        ));
    }

    #[test]
    fn upserts_existing_daily_message_entry() {
        let temp = tempfile::tempdir().unwrap();
        let writer = SourceWriter::daily_log(temp.path());
        let first = ProcessedArtifact::new("msg_1", ProcessedArtifactKind::Text, "old content");
        let second = ProcessedArtifact::new("msg_1", ProcessedArtifactKind::Text, "new content");

        let result = writer.write_source(&first, &metadata()).unwrap();
        writer.write_source(&second, &metadata()).unwrap();
        let content = fs::read_to_string(result.path).unwrap();

        assert!(!content.contains("old content"));
        assert!(content.contains("new content"));
        assert_eq!(
            content
                .matches("<!-- sage-wiki-bridge-message-start:msg_1 -->")
                .count(),
            1
        );
    }

    #[test]
    fn rejects_unsafe_message_key() {
        let temp = tempfile::tempdir().unwrap();
        let writer = SourceWriter::daily_log(temp.path());
        let artifact = ProcessedArtifact::new("../escape", ProcessedArtifactKind::Text, "bad path");

        let err = writer.write_source(&artifact, &metadata()).unwrap_err();

        assert!(matches!(err, BridgeError::PathOutsideRoot(_)));
    }

    #[test]
    fn escapes_yaml_string_values() {
        let artifact = ProcessedArtifact::new("msg_1", ProcessedArtifactKind::Text, "body");
        let mut metadata = metadata();
        metadata.message_type = "text\"quoted".to_string();

        let markdown = render_daily_log_entry("msg_1", &artifact, &metadata);

        assert!(markdown.contains("- message_type: \"text\\\"quoted\""));
    }

    #[test]
    fn writes_ai_friendly_daily_source_by_default() {
        let temp = tempfile::tempdir().unwrap();
        let writer = SourceWriter::new(temp.path());
        let artifact = ProcessedArtifact::new(
            "20260527T133015Z_1000000000000000001",
            ProcessedArtifactKind::Text,
            "hello sage-wiki",
        );

        let result = writer.write_source(&artifact, &metadata()).unwrap();
        let content = fs::read_to_string(&result.path).unwrap();

        assert!(content.contains("source_type: \"wechat_ai_messages\""));
        assert!(content.contains("# WeChat Knowledge 2026-05-27"));
        assert!(
            content.contains("<!-- swb:thread v=1 id=20260527T133015Z_1000000000000000001 -->")
        );
        assert!(content.contains("<<< wechat-thread >>>"));
        assert!(content.contains("[2026-05-27T21:30:15+08:00 text]"));
        assert!(content.contains("hello sage-wiki"));
        assert!(content.contains("<!-- swb:item:start:20260527T133015Z_1000000000000000001 -->"));
        assert!(content.contains("<<< /wechat-thread >>>"));
        assert!(!content.contains("### Metadata"));
        assert!(!content.contains("openid_hash"));
        assert!(!content.contains("raw_dir"));
    }

    #[test]
    fn appends_multiple_items_to_same_ai_thread() {
        let temp = tempfile::tempdir().unwrap();
        let writer = SourceWriter::new(temp.path());
        let first = ProcessedArtifact::new("msg_1", ProcessedArtifactKind::Text, "first");
        let second = ProcessedArtifact::new("msg_2", ProcessedArtifactKind::Text, "second");
        let mut first_metadata = metadata();
        first_metadata.thread_id = Some("thread_1".to_string());
        let mut second_metadata = metadata();
        second_metadata.thread_id = Some("thread_1".to_string());
        second_metadata.received_at = "2026-05-27T21:35:15+08:00".to_string();

        let result = writer.write_source(&first, &first_metadata).unwrap();
        writer.write_source(&second, &second_metadata).unwrap();
        let content = fs::read_to_string(result.path).unwrap();

        assert_eq!(
            count_occurrences(&content, "<!-- swb:thread v=1 id=thread_1 -->"),
            1
        );
        assert!(content.contains("<!-- swb:item:start:msg_1 -->"));
        assert!(content.contains("<!-- swb:item:start:msg_2 -->"));
        assert!(content.contains("[2026-05-27T21:35:15+08:00 text]"));
    }

    #[test]
    fn upserts_existing_ai_thread_item() {
        let temp = tempfile::tempdir().unwrap();
        let writer = SourceWriter::new(temp.path());
        let first = ProcessedArtifact::new("msg_1", ProcessedArtifactKind::Text, "old");
        let second = ProcessedArtifact::new("msg_1", ProcessedArtifactKind::Text, "new");
        let mut metadata = metadata();
        metadata.thread_id = Some("thread_1".to_string());

        let result = writer.write_source(&first, &metadata).unwrap();
        writer.write_source(&second, &metadata).unwrap();
        let content = fs::read_to_string(result.path).unwrap();

        assert!(!content.contains("old"));
        assert!(content.contains("new"));
        assert_eq!(
            count_occurrences(&content, "<!-- swb:item:start:msg_1 -->"),
            1
        );
    }
}
