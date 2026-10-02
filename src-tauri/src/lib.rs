use serde::Serialize;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

const PREVIEW_FILE_LIMIT: usize = 200;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileEntry {
    name: String,
    path: String,
    extension: String,
    size_bytes: u64,
    modified_unix_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FolderScan {
    root: String,
    total_files: u64,
    total_bytes: u64,
    skipped_entries: u64,
    preview_files: Vec<FileEntry>,
}

#[tauri::command]
fn scan_folder(root: String) -> Result<FolderScan, String> {
    let canonical_root = std::fs::canonicalize(&root)
        .map_err(|error| format!("Unable to access selected folder: {error}"))?;

    if !canonical_root.is_dir() {
        return Err("Selected path is not a folder.".to_string());
    }

    let mut total_files = 0_u64;
    let mut total_bytes = 0_u64;
    let mut skipped_entries = 0_u64;
    let mut preview_files = Vec::with_capacity(PREVIEW_FILE_LIMIT);

    for entry_result in WalkDir::new(&canonical_root).follow_links(false) {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(_) => {
                skipped_entries += 1;
                continue;
            }
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => {
                skipped_entries += 1;
                continue;
            }
        };

        total_files += 1;
        total_bytes = total_bytes.saturating_add(metadata.len());

        if preview_files.len() >= PREVIEW_FILE_LIMIT {
            continue;
        }

        let path = entry.path();
        let modified_unix_ms = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as u64);

        preview_files.push(FileEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: path.to_string_lossy().into_owned(),
            extension: path
                .extension()
                .map(|extension| extension.to_string_lossy().into_owned())
                .unwrap_or_default(),
            size_bytes: metadata.len(),
            modified_unix_ms,
        });
    }

    Ok(FolderScan {
        root: canonical_root.to_string_lossy().into_owned(),
        total_files,
        total_bytes,
        skipped_entries,
        preview_files,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![scan_folder])
        .run(tauri::generate_context!())
        .expect("error while running Professional DocX");
}
