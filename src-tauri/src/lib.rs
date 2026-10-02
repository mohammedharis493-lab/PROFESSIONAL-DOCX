mod persistence;

use serde::Serialize;
use std::time::UNIX_EPOCH;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;
use walkdir::WalkDir;

const PREVIEW_FILE_LIMIT: usize = 200;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApprovedStorageRootDto {
    storage_root_id: String,
    display_path: String,
    availability_state: String,
}

impl From<persistence::StorageRootRecord> for ApprovedStorageRootDto {
    fn from(value: persistence::StorageRootRecord) -> Self {
        Self {
            storage_root_id: value.storage_root_id,
            display_path: value.display_path,
            availability_state: value.availability_state,
        }
    }
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
    database: State<'_, persistence::DatabaseState>,
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

    let registered = persistence::register_storage_root(
        database.path(),
        &Uuid::new_v4().to_string(),
        &selected_path,
        &canonical_path,
    )
    .map_err(|error| error.to_string())?;

    Ok(Some(registered.into()))
}

#[tauri::command]
fn list_storage_roots(
    database: State<'_, persistence::DatabaseState>,
) -> Result<Vec<ApprovedStorageRootDto>, String> {
    persistence::list_storage_roots(database.path())
        .map(|roots| roots.into_iter().map(Into::into).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn scan_storage_root(
    storage_root_id: String,
    database: State<'_, persistence::DatabaseState>,
) -> Result<FolderScan, String> {
    Uuid::parse_str(&storage_root_id)
        .map_err(|_| "Invalid storage-root identifier.".to_string())?;

    let approved = persistence::get_storage_root(database.path(), &storage_root_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Storage root is not approved.".to_string())?;

    match std::fs::metadata(&approved.canonical_path) {
        Ok(metadata) if metadata.is_dir() => {
            persistence::set_storage_root_availability(
                database.path(),
                &storage_root_id,
                "AVAILABLE",
            )
            .map_err(|error| error.to_string())?;
        }
        Ok(_) => {
            persistence::set_storage_root_availability(
                database.path(),
                &storage_root_id,
                "DEGRADED",
            )
            .map_err(|error| error.to_string())?;
            return Err("Approved storage root is no longer a directory.".to_string());
        }
        Err(error) => {
            persistence::set_storage_root_availability(
                database.path(),
                &storage_root_id,
                "OFFLINE",
            )
            .map_err(|persistence_error| persistence_error.to_string())?;
            return Err(format!("Approved storage root is unavailable: {error}"));
        }
    }

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
        storage_root_id: approved.storage_root_id,
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
        .setup(|app| {
            let database_path = app
                .path()
                .app_data_dir()?
                .join("data")
                .join("metadata.sqlite");

            persistence::initialize_database(&database_path)?;
            app.manage(persistence::DatabaseState::new(database_path));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            choose_and_register_storage_root,
            list_storage_roots,
            scan_storage_root
        ])
        .run(tauri::generate_context!())
        .expect("error while running Professional DocX");
}
