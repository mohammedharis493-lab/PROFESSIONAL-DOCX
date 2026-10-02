mod persistence;

use serde::Serialize;
use std::{collections::HashMap, path::PathBuf, sync::Mutex, time::UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;
use walkdir::WalkDir;

const PREVIEW_FILE_LIMIT: usize = 200;

#[derive(Debug, Clone)]
struct ApprovedStorageRoot {
    storage_root_id: Uuid,
    canonical_path: PathBuf,
    display_path: String,
}

#[derive(Default)]
struct StorageRootRegistry {
    roots: Mutex<HashMap<Uuid, ApprovedStorageRoot>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApprovedStorageRootDto {
    storage_root_id: String,
    display_path: String,
}

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
    storage_root_id: String,
    root_display_path: String,
    total_files: u64,
    total_bytes: u64,
    skipped_entries: u64,
    preview_files: Vec<FileEntry>,
}

#[tauri::command]
async fn choose_and_register_storage_root(
    app: AppHandle,
    roots: State<'_, StorageRootRegistry>,
) -> Result<Option<ApprovedStorageRootDto>, String> {
    let selected = app.dialog().file().blocking_pick_folder();

    let Some(selected) = selected else {
        return Ok(None);
    };

    let selected_path = selected
        .into_path()
        .map_err(|_| "Selected folder could not be resolved to a native path.".to_string())?;

    let canonical_path = std::fs::canonicalize(&selected_path)
        .map_err(|error| format!("Unable to access selected folder: {error}"))?;

    if !canonical_path.is_dir() {
        return Err("Selected path is not a folder.".to_string());
    }

    let storage_root_id = Uuid::new_v4();
    let display_path = canonical_path.to_string_lossy().into_owned();

    let approved = ApprovedStorageRoot {
        storage_root_id,
        canonical_path,
        display_path: display_path.clone(),
    };

    let mut registry = roots
        .roots
        .lock()
        .map_err(|_| "Approved storage-root registry is unavailable.".to_string())?;

    registry.insert(storage_root_id, approved);

    Ok(Some(ApprovedStorageRootDto {
        storage_root_id: storage_root_id.to_string(),
        display_path,
    }))
}

#[tauri::command]
fn scan_storage_root(
    storage_root_id: String,
    roots: State<'_, StorageRootRegistry>,
) -> Result<FolderScan, String> {
    let root_id = Uuid::parse_str(&storage_root_id)
        .map_err(|_| "Invalid storage-root identifier.".to_string())?;

    let approved = {
        let registry = roots
            .roots
            .lock()
            .map_err(|_| "Approved storage-root registry is unavailable.".to_string())?;

        registry.get(&root_id).cloned().ok_or_else(|| {
            "Storage root is not approved for this application session.".to_string()
        })?
    };

    let mut total_files = 0_u64;
    let mut total_bytes = 0_u64;
    let mut skipped_entries = 0_u64;
    let mut preview_files = Vec::with_capacity(PREVIEW_FILE_LIMIT);

    for entry_result in WalkDir::new(&approved.canonical_path).follow_links(false) {
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
        storage_root_id: approved.storage_root_id.to_string(),
        root_display_path: approved.display_path,
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
        .manage(StorageRootRegistry::default())
        .setup(|app| {
            let database_path = app
                .path()
                .app_data_dir()?
                .join("data")
                .join("metadata.sqlite");

            persistence::initialize_database(&database_path)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            choose_and_register_storage_root,
            scan_storage_root
        ])
        .run(tauri::generate_context!())
        .expect("error while running Professional DocX");
}
