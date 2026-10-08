use crate::{
    evidence::{self, EvidenceError, EvidenceState},
    launcher,
    persistence::{self, ResolvedFileSource},
};
use serde::Serialize;
use std::{
    fs,
    io::{Read, Take},
    path::Path,
};
use tauri::http::{
    header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE},
    Request, Response, StatusCode,
};
use uuid::Uuid;

pub const MAX_TEXT_PREVIEW_BYTES: u64 = 256 * 1024;
pub const MAX_PDF_PREVIEW_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_IMAGE_PREVIEW_BYTES: u64 = 32 * 1024 * 1024;

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

pub fn image_preview_response(
    database_path: &Path,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let Some(file_instance_id) = parse_image_preview_file_instance_id(request.uri().path()) else {
        return image_error_response(StatusCode::BAD_REQUEST, "Invalid image preview request.");
    };

    let source = match persistence::resolve_file_instance_source(database_path, &file_instance_id) {
        Ok(Some(source)) => source,
        Ok(None) => {
            return image_error_response(StatusCode::NOT_FOUND, "Indexed image no longer exists.");
        }
        Err(_) => {
            return image_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unable to resolve image preview.",
            );
        }
    };

    let (bytes, content_type) = match read_image_source(&source) {
        Ok(result) => result,
        Err(ImagePreviewError::TooLarge) => {
            return image_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Image is too large for in-app preview. Open the original instead.",
            );
        }
        Err(ImagePreviewError::Unsupported) => {
            return image_error_response(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "The indexed source is not a supported raster image preview target.",
            );
        }
        Err(ImagePreviewError::Unavailable) => {
            return image_error_response(
                StatusCode::NOT_FOUND,
                "The original image is currently unavailable.",
            );
        }
    };

    if persistence::record_document_open(database_path, &file_instance_id).is_err() {
        return image_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Image preview opened but recent-document history could not be updated.",
        );
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .header(CONTENT_DISPOSITION, "inline")
        .header(CACHE_CONTROL, "no-store, private")
        .header(CONTENT_LENGTH, bytes.len().to_string())
        .header("Access-Control-Allow-Origin", "*")
        .header("X-Content-Type-Options", "nosniff")
        .body(bytes)
        .expect("static image preview response headers are valid")
}

pub fn pdf_preview_response(database_path: &Path, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let Some(file_instance_id) = parse_pdf_preview_file_instance_id(request.uri().path()) else {
        return pdf_error_response(StatusCode::BAD_REQUEST, "Invalid PDF preview request.");
    };

    let source = match persistence::resolve_file_instance_source(database_path, &file_instance_id) {
        Ok(Some(source)) => source,
        Ok(None) => {
            return pdf_error_response(StatusCode::NOT_FOUND, "Indexed PDF no longer exists.");
        }
        Err(_) => {
            return pdf_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unable to resolve PDF preview.",
            );
        }
    };

    let bytes = match read_pdf_source(&source) {
        Ok(bytes) => bytes,
        Err(PdfPreviewError::TooLarge) => {
            return pdf_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "PDF is too large for in-app preview. Open the original instead.",
            );
        }
        Err(PdfPreviewError::Unsupported) => {
            return pdf_error_response(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "The indexed source is not a valid PDF preview target.",
            );
        }
        Err(PdfPreviewError::Unavailable) => {
            return pdf_error_response(
                StatusCode::NOT_FOUND,
                "The original PDF is currently unavailable.",
            );
        }
    };

    if persistence::record_document_open(database_path, &file_instance_id).is_err() {
        return pdf_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "PDF preview opened but recent-document history could not be updated.",
        );
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/pdf")
        .header(CONTENT_DISPOSITION, "inline")
        .header(CACHE_CONTROL, "no-store, private")
        .header(CONTENT_LENGTH, bytes.len().to_string())
        .header("Access-Control-Allow-Origin", "*")
        .header("X-Content-Type-Options", "nosniff")
        .body(bytes)
        .expect("static PDF preview response headers are valid")
}

pub fn controlled_pdf_preview_response(
    database_path: &Path,
    evidence_state: &EvidenceState,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let Some(controlled_evidence_version_id) =
        parse_controlled_pdf_preview_evidence_id(request.uri().path())
    else {
        return pdf_error_response(
            StatusCode::BAD_REQUEST,
            "Invalid controlled PDF preview request.",
        );
    };

    let controlled = match evidence::read_controlled_evidence_bytes(
        database_path,
        evidence_state,
        &controlled_evidence_version_id,
        MAX_PDF_PREVIEW_BYTES as usize,
    ) {
        Ok(controlled) => controlled,
        Err(EvidenceError::Configuration(message))
            if message.contains("exceeds the") && message.contains("read limit") =>
        {
            return pdf_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Controlled PDF is too large for in-app preview.",
            );
        }
        Err(EvidenceError::Configuration(_)) => {
            return pdf_error_response(
                StatusCode::NOT_FOUND,
                "Controlled PDF evidence is unavailable.",
            );
        }
        Err(EvidenceError::Integrity(_)) => {
            return pdf_error_response(
                StatusCode::CONFLICT,
                "Controlled PDF evidence failed integrity verification.",
            );
        }
        Err(_) => {
            return pdf_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unable to read controlled PDF evidence.",
            );
        }
    };

    if !has_pdf_header(&controlled.bytes) {
        return pdf_error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Controlled evidence is not a valid PDF preview target.",
        );
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/pdf")
        .header(CONTENT_DISPOSITION, "inline")
        .header(CACHE_CONTROL, "no-store, private")
        .header(CONTENT_LENGTH, controlled.bytes.len().to_string())
        .header("Access-Control-Allow-Origin", "*")
        .header("X-Content-Type-Options", "nosniff")
        .body(controlled.bytes)
        .expect("static controlled PDF preview response headers are valid")
}

fn parse_image_preview_file_instance_id(path: &str) -> Option<String> {
    let raw = path.strip_prefix("/image/")?;
    if raw.is_empty() || raw.contains('/') {
        return None;
    }

    Uuid::parse_str(raw).ok().map(|value| value.to_string())
}

fn parse_pdf_preview_file_instance_id(path: &str) -> Option<String> {
    let raw = path.strip_prefix("/pdf/")?;
    if raw.is_empty() || raw.contains('/') {
        return None;
    }

    Uuid::parse_str(raw).ok().map(|value| value.to_string())
}

fn parse_controlled_pdf_preview_evidence_id(path: &str) -> Option<String> {
    let raw = path.strip_prefix("/controlled-pdf/")?;
    if raw.is_empty() || raw.contains('/') {
        return None;
    }

    Uuid::parse_str(raw).ok().map(|value| value.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImagePreviewError {
    TooLarge,
    Unsupported,
    Unavailable,
}

fn read_image_source(
    source: &ResolvedFileSource,
) -> Result<(Vec<u8>, &'static str), ImagePreviewError> {
    let path =
        launcher::validated_existing_path(source).map_err(|_| ImagePreviewError::Unavailable)?;

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let expected_content_type =
        image_content_type_for_extension(&extension).ok_or(ImagePreviewError::Unsupported)?;

    let metadata = fs::metadata(&path).map_err(|_| ImagePreviewError::Unavailable)?;
    if metadata.len() > MAX_IMAGE_PREVIEW_BYTES {
        return Err(ImagePreviewError::TooLarge);
    }

    let bytes = fs::read(path).map_err(|_| ImagePreviewError::Unavailable)?;
    let detected_content_type =
        detect_image_content_type(&bytes).ok_or(ImagePreviewError::Unsupported)?;

    if detected_content_type != expected_content_type {
        return Err(ImagePreviewError::Unsupported);
    }

    Ok((bytes, detected_content_type))
}

fn image_content_type_for_extension(extension: &str) -> Option<&'static str> {
    match extension {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        _ => None,
    }
}

fn detect_image_content_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }

    if bytes.len() >= 3 && bytes[0..3] == [0xff, 0xd8, 0xff] {
        return Some("image/jpeg");
    }

    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }

    if bytes.len() >= 12
        && bytes.starts_with(b"RIFF")
        && bytes.get(8..12) == Some(b"WEBP".as_slice())
    {
        return Some("image/webp");
    }

    if bytes.starts_with(b"BM") {
        return Some("image/bmp");
    }

    None
}

fn image_error_response(status: StatusCode, message: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(CACHE_CONTROL, "no-store, private")
        .header("Access-Control-Allow-Origin", "*")
        .header("X-Content-Type-Options", "nosniff")
        .body(message.as_bytes().to_vec())
        .expect("static image preview error response headers are valid")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PdfPreviewError {
    TooLarge,
    Unsupported,
    Unavailable,
}

fn read_pdf_source(source: &ResolvedFileSource) -> Result<Vec<u8>, PdfPreviewError> {
    let path =
        launcher::validated_existing_path(source).map_err(|_| PdfPreviewError::Unavailable)?;

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    if !extension.eq_ignore_ascii_case("pdf") {
        return Err(PdfPreviewError::Unsupported);
    }

    let metadata = fs::metadata(&path).map_err(|_| PdfPreviewError::Unavailable)?;
    if metadata.len() > MAX_PDF_PREVIEW_BYTES {
        return Err(PdfPreviewError::TooLarge);
    }

    let bytes = fs::read(path).map_err(|_| PdfPreviewError::Unavailable)?;
    if !has_pdf_header(&bytes) {
        return Err(PdfPreviewError::Unsupported);
    }

    Ok(bytes)
}

fn has_pdf_header(bytes: &[u8]) -> bool {
    let header_window = &bytes[..bytes.len().min(1024)];
    header_window.windows(5).any(|window| window == b"%PDF-")
}

fn pdf_error_response(status: StatusCode, message: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(CACHE_CONTROL, "no-store, private")
        .header("Access-Control-Allow-Origin", "*")
        .header("X-Content-Type-Options", "nosniff")
        .body(message.as_bytes().to_vec())
        .expect("static PDF preview error response headers are valid")
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
    fn unsupported_text_extension_is_rejected() {
        let fixture = PreviewFixture::new();
        fs::write(fixture.root.join("document.pdf"), b"%PDF-test")
            .expect("unsupported fixture should be written");

        let error = preview_text_source(&fixture.source("document.pdf"))
            .expect_err("unsupported preview should fail");

        assert!(error.contains("not supported"));
    }

    #[test]
    fn reads_supported_raster_image_inside_approved_root() {
        let fixture = PreviewFixture::new();
        let png = b"\x89PNG\r\n\x1a\nminimal-png-fixture";
        fs::write(fixture.root.join("sample.png"), png).expect("PNG fixture should be written");

        let (bytes, content_type) = read_image_source(&fixture.source("sample.png"))
            .expect("valid raster image preview should succeed");

        assert_eq!(bytes, png);
        assert_eq!(content_type, "image/png");
    }

    #[test]
    fn rejects_image_content_that_does_not_match_extension() {
        let fixture = PreviewFixture::new();
        fs::write(
            fixture.root.join("fake.jpg"),
            b"\x89PNG\r\n\x1a\nactually-png",
        )
        .expect("mismatched image fixture should be written");

        let error = read_image_source(&fixture.source("fake.jpg"))
            .expect_err("mismatched raster image should fail");

        assert_eq!(error, ImagePreviewError::Unsupported);
    }

    #[test]
    fn rejects_svg_from_raster_image_preview() {
        let fixture = PreviewFixture::new();
        fs::write(
            fixture.root.join("active.svg"),
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script/></svg>",
        )
        .expect("SVG fixture should be written");

        let error = read_image_source(&fixture.source("active.svg"))
            .expect_err("SVG must not use raster image preview");

        assert_eq!(error, ImagePreviewError::Unsupported);
    }

    #[test]
    fn rejects_image_over_preview_size_limit() {
        let fixture = PreviewFixture::new();
        let path = fixture.root.join("huge.png");
        let file = fs::File::create(&path).expect("large image fixture should be created");
        file.set_len(MAX_IMAGE_PREVIEW_BYTES + 1)
            .expect("large image fixture should be sized");

        let error = read_image_source(&fixture.source("huge.png"))
            .expect_err("oversize image preview should fail");

        assert_eq!(error, ImagePreviewError::TooLarge);
    }

    #[test]
    fn parses_only_uuid_image_preview_routes() {
        let id = Uuid::new_v4();
        assert_eq!(
            parse_image_preview_file_instance_id(&format!("/image/{id}")),
            Some(id.to_string())
        );
        assert!(parse_image_preview_file_instance_id("/image/not-a-uuid").is_none());
        assert!(parse_image_preview_file_instance_id("/image/a/b").is_none());
        assert!(parse_image_preview_file_instance_id("/pdf/value").is_none());
    }

    #[test]
    fn reads_valid_pdf_inside_approved_root() {
        let fixture = PreviewFixture::new();
        let pdf = b"%PDF-1.7\n1 0 obj\n<<>>\nendobj\n%%EOF\n";
        fs::write(fixture.root.join("sample.pdf"), pdf).expect("PDF fixture should be written");

        let bytes = read_pdf_source(&fixture.source("sample.pdf"))
            .expect("valid PDF preview should succeed");

        assert_eq!(bytes, pdf);
    }

    #[test]
    fn rejects_non_pdf_content_with_pdf_extension() {
        let fixture = PreviewFixture::new();
        fs::write(fixture.root.join("fake.pdf"), b"not a pdf")
            .expect("fake PDF fixture should be written");

        let error = read_pdf_source(&fixture.source("fake.pdf"))
            .expect_err("invalid PDF signature should fail");

        assert_eq!(error, PdfPreviewError::Unsupported);
    }

    #[test]
    fn rejects_pdf_over_preview_size_limit() {
        let fixture = PreviewFixture::new();
        let path = fixture.root.join("huge.pdf");
        let file = fs::File::create(&path).expect("large PDF fixture should be created");
        file.set_len(MAX_PDF_PREVIEW_BYTES + 1)
            .expect("large PDF fixture should be sized");

        let error = read_pdf_source(&fixture.source("huge.pdf"))
            .expect_err("oversize PDF preview should fail");

        assert_eq!(error, PdfPreviewError::TooLarge);
    }

    #[test]
    fn parses_only_uuid_pdf_preview_routes() {
        let id = Uuid::new_v4();
        assert_eq!(
            parse_pdf_preview_file_instance_id(&format!("/pdf/{id}")),
            Some(id.to_string())
        );
        assert!(parse_pdf_preview_file_instance_id("/pdf/not-a-uuid").is_none());
        assert!(parse_pdf_preview_file_instance_id("/pdf/a/b").is_none());
        assert!(parse_pdf_preview_file_instance_id("/other/value").is_none());
    }

    #[test]
    fn parses_only_uuid_controlled_pdf_preview_routes() {
        let id = Uuid::new_v4();
        assert_eq!(
            parse_controlled_pdf_preview_evidence_id(&format!("/controlled-pdf/{id}")),
            Some(id.to_string())
        );
        assert!(parse_controlled_pdf_preview_evidence_id("/controlled-pdf/not-a-uuid").is_none());
        assert!(parse_controlled_pdf_preview_evidence_id("/controlled-pdf/a/b").is_none());
        assert!(parse_controlled_pdf_preview_evidence_id("/pdf/value").is_none());
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

    #[cfg(unix)]
    #[test]
    fn image_preview_rejects_symlink_escape_from_approved_root() {
        use std::os::unix::fs::symlink;

        let fixture = PreviewFixture::new();
        let outside = std::env::temp_dir().join(format!(
            "professional-docx-image-outside-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&outside).expect("outside directory should be created");
        fs::write(outside.join("secret.png"), b"\x89PNG\r\n\x1a\nsecret")
            .expect("outside image should be written");
        symlink(outside.join("secret.png"), fixture.root.join("linked.png"))
            .expect("symlink should be created");

        let error = read_image_source(&fixture.source("linked.png"))
            .expect_err("image symlink escape should be rejected");

        assert_eq!(error, ImagePreviewError::Unavailable);

        let _ = fs::remove_dir_all(outside);
    }

    #[cfg(unix)]
    #[test]
    fn pdf_preview_rejects_symlink_escape_from_approved_root() {
        use std::os::unix::fs::symlink;

        let fixture = PreviewFixture::new();
        let outside =
            std::env::temp_dir().join(format!("professional-docx-pdf-outside-{}", Uuid::new_v4()));
        fs::create_dir_all(&outside).expect("outside directory should be created");
        fs::write(outside.join("secret.pdf"), b"%PDF-1.7\n%%EOF\n")
            .expect("outside PDF should be written");
        symlink(outside.join("secret.pdf"), fixture.root.join("linked.pdf"))
            .expect("symlink should be created");

        let error = read_pdf_source(&fixture.source("linked.pdf"))
            .expect_err("PDF symlink escape should be rejected");

        assert_eq!(error, PdfPreviewError::Unavailable);

        let _ = fs::remove_dir_all(outside);
    }
}
