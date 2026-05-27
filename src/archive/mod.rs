use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::error::BridgeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveRecord {
    pub path: Option<PathBuf>,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct RawArchive {
    root: PathBuf,
    save_full_payload: bool,
}

impl RawArchive {
    pub fn new(root: impl Into<PathBuf>, save_full_payload: bool) -> Self {
        Self {
            root: root.into(),
            save_full_payload,
        }
    }

    pub fn archive_bytes(
        &self,
        message_key: &str,
        filename: &str,
        bytes: &[u8],
    ) -> Result<ArchiveRecord, BridgeError> {
        let sha256 = sha256_hex(bytes);
        let size_bytes = bytes.len() as u64;

        if !self.save_full_payload {
            return Ok(ArchiveRecord {
                path: None,
                sha256,
                size_bytes,
            });
        }

        let relative_path = safe_relative_path(message_key, filename)?;
        let path = self.root.join(relative_path);
        ensure_under_root(&self.root, &path)?;
        write_file_atomically(&path, bytes)?;

        Ok(ArchiveRecord {
            path: Some(path),
            sha256,
            size_bytes,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ProcessedArtifactStore {
    root: PathBuf,
}

impl ProcessedArtifactStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn save_artifact(
        &self,
        message_key: &str,
        filename: &str,
        bytes: &[u8],
    ) -> Result<ArchiveRecord, BridgeError> {
        let relative_path = safe_relative_path(message_key, filename)?;
        let path = self.root.join(relative_path);
        ensure_under_root(&self.root, &path)?;
        write_file_atomically(&path, bytes)?;

        Ok(ArchiveRecord {
            path: Some(path),
            sha256: sha256_hex(bytes),
            size_bytes: bytes.len() as u64,
        })
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn safe_relative_path(message_key: &str, filename: &str) -> Result<PathBuf, BridgeError> {
    let key = sanitize_segment(message_key)?;
    let name = sanitize_segment(filename)?;
    Ok(PathBuf::from(key).join(name))
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

    let sanitized: String = segment
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect();

    Ok(sanitized)
}

fn ensure_under_root(root: &Path, path: &Path) -> Result<(), BridgeError> {
    let root = root.components().collect::<Vec<_>>();
    let path_components = path.components().collect::<Vec<_>>();

    if path_components.starts_with(&root) {
        Ok(())
    } else {
        Err(BridgeError::PathOutsideRoot(path.display().to_string()))
    }
}

fn write_file_atomically(path: &Path, bytes: &[u8]) -> Result<(), BridgeError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("file")
    ));

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
    use super::*;

    #[test]
    fn raw_archive_metadata_only_hashes_without_writing() {
        let temp = tempfile::tempdir().unwrap();
        let archive = RawArchive::new(temp.path(), false);

        let record = archive
            .archive_bytes("msg_1", "callback.xml", b"<xml />")
            .unwrap();

        assert!(record.path.is_none());
        assert_eq!(record.size_bytes, 7);
        assert!(temp.path().read_dir().unwrap().next().is_none());
    }

    #[test]
    fn raw_archive_full_mode_writes_payload() {
        let temp = tempfile::tempdir().unwrap();
        let archive = RawArchive::new(temp.path(), true);

        let record = archive
            .archive_bytes("msg_1", "callback.xml", b"<xml />")
            .unwrap();

        let path = record.path.unwrap();
        assert_eq!(fs::read(path).unwrap(), b"<xml />");
    }

    #[test]
    fn processed_artifact_store_always_writes() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProcessedArtifactStore::new(temp.path());

        let record = store
            .save_artifact("msg_1", "source-draft.md", b"# hello")
            .unwrap();

        let path = record.path.unwrap();
        assert_eq!(fs::read(path).unwrap(), b"# hello");
    }

    #[test]
    fn rejects_path_escape_segments() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProcessedArtifactStore::new(temp.path());

        let err = store
            .save_artifact("../secret", "source.md", b"bad")
            .unwrap_err();

        assert!(matches!(err, BridgeError::PathOutsideRoot(_)));
    }
}
