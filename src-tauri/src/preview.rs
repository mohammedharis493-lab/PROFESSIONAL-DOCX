use crate::{launcher, persistence::ResolvedFileSource};
use serde::Serialize;
use std::{
    fs,
    io::{Read, Take},
    path::Path,
};

pub const MAX_TEXT_PREVIEW_BYTES: u64 = 256 * 1024;

const SUPPORTED_TEXT_EXTENSIONS: &[&str] = &["txt", "csv", "xml"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPreview {
    pub content: String,
    pub truncated: bool,
    pub total_size_bytes: u64,
    pub previewed_bytes: u64,
    pub extension: String,
}

pub fn preview_text_source(source: &ResolvedFileSource) -> Result<TextPreview, String> {
    let path = launcher::validated_existing_path(source).map_err(|error| error.to_string())?;
    preview_validated_path(&path)
}

fn preview_validated_path(path: &Path) -> Result<TextPreview, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if !SUPPORTED_TEXT_EXTENSIONS.contains(&extension.as_str()) {
        return Err(format!(
            "In-app text preview is not supported for .{} files.",
            if extension.is_empty() {
                "<none>"
            } else {
                &extension
            }
        ));
    }

    let metadata =
        fs::metadata(path).map_err(|error| format!("Unable to inspect preview source: {error}"))?;
    let total_size_bytes = metadata.len();

    let file =
        fs::File::open(path).map_err(|error| format!("Unable to open preview source: {error}"))?;
    let mut limited: Take<fs::File> = file.take(MAX_TEXT_PREVIEW_BYTES + 1);
    let mut bytes = Vec::with_capacity(
        usize::try_from(total_size_bytes.min(MAX_TEXT_PREVIEW_BYTES + 1)).unwrap_or_default(),
    );
    limited
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Unable to read preview source: {error}"))?;

    let truncated = bytes.len() as u64 > MAX_TEXT_PREVIEW_BYTES;
    if truncated {
        bytes.truncate(MAX_TEXT_PREVIEW_BYTES as usize);
    }

    let previewed_bytes = bytes.len() as u64;
    let content = String::from_utf8_lossy(&bytes)
        .trim_start_matches('﻿')
        .to_string();

    Ok(TextPreview {
        content,
        truncated,
        total_size_bytes,
        previewed_bytes,
        extension,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use uuid::Uuid;

    struct PreviewFixture {
        root: PathBuf,
    }

    impl PreviewFixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("professional-docx-preview-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).expect("preview test root should be created");
            Self { root }
        }

        fn source(&self, relative_path: &str) -> ResolvedFileSource {
            ResolvedFileSource {
                storage_root_path: self.root.clone(),
                relative_path: PathBuf::from(relative_path),
            }
        }
    }

    impl Drop for PreviewFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn previews_supported_text_without_exposing_unbounded_content() {
        let fixture = PreviewFixture::new();
        fs::write(fixture.root.join("sample.txt"), b"alpha\nbeta\n")
            .expect("text fixture should be written");

        let preview = preview_text_source(&fixture.source("sample.txt"))
            .expect("supported text preview should succeed");

        assert_eq!(preview.content, "alpha\nbeta\n");
        assert!(!preview.truncated);
        assert_eq!(preview.total_size_bytes, 11);
        assert_eq!(preview.previewed_bytes, 11);
        assert_eq!(preview.extension, "txt");
    }

    #[test]
    fn large_text_preview_is_bounded_and_marked_truncated() {
        let fixture = PreviewFixture::new();
        let content = vec![b'x'; (MAX_TEXT_PREVIEW_BYTES + 32) as usize];
        fs::write(fixture.root.join("large.csv"), content)
            .expect("large fixture should be written");

        let preview = preview_text_source(&fixture.source("large.csv"))
            .expect("large preview should succeed");

        assert!(preview.truncated);
        assert_eq!(preview.previewed_bytes, MAX_TEXT_PREVIEW_BYTES);
        assert_eq!(preview.total_size_bytes, MAX_TEXT_PREVIEW_BYTES + 32);
        assert_eq!(preview.content.len(), MAX_TEXT_PREVIEW_BYTES as usize);
    }

    #[test]
    fn unsupported_extension_is_rejected() {
        let fixture = PreviewFixture::new();
        fs::write(fixture.root.join("document.pdf"), b"%PDF-test")
            .expect("unsupported fixture should be written");

        let error = preview_text_source(&fixture.source("document.pdf"))
            .expect_err("unsupported preview should fail");

        assert!(error.contains("not supported"));
    }

    #[cfg(unix)]
    #[test]
    fn preview_rejects_symlink_escape_from_approved_root() {
        use std::os::unix::fs::symlink;

        let fixture = PreviewFixture::new();
        let outside = std::env::temp_dir().join(format!(
            "professional-docx-preview-outside-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&outside).expect("outside directory should be created");
        fs::write(outside.join("secret.txt"), b"secret").expect("outside file should be written");
        symlink(outside.join("secret.txt"), fixture.root.join("linked.txt"))
            .expect("symlink should be created");

        let error = preview_text_source(&fixture.source("linked.txt"))
            .expect_err("symlink escape should be rejected");

        assert!(error.contains("approved storage root"));

        let _ = fs::remove_dir_all(outside);
    }
}
