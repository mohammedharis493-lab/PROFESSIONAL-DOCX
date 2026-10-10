//! Private, recoverable on-disk *staging* of three verified Normal Data
//! artifacts. Not controlled evidence, retention approval, or specialist linkage.
//! No command exposes this module to the frontend or accepts a caller path.
use crate::{
    normal_data_comparison::{self, ComparisonResult},
    normal_data_preservation_material::{PreservationMaterial, SourceMaterial},
    persistence::PersistenceError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fmt::Write,
    fs::{self, File, OpenOptions},
    io::{Read, Write as IoWrite},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MAX_SOURCE_BYTES: usize = 32 * 1024 * 1024;
const MAX_RESULT_BYTES: usize = 8 * 1024 * 1024;
const MAX_MANIFEST_BYTES: usize = 8192;
const MAX_SCAN_ENTRIES: usize = 10_000;
const MANIFEST_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    file_name: String,
    size_bytes: u64,
    sha256_hex: String,
    dataset_version_id: Option<String>,
    document_id: Option<String>,
    content_version_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format_version: u8,
    stage_id: String,
    run_id: String,
    workspace_id: String,
    recipe_version_id: String,
    source_a: Artifact,
    source_b: Artifact,
    result: Artifact,
    result_semantic_sha256_hex: String,
}

/// Metadata only. This is a checked staging receipt, never an evidence grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StageReceipt {
    pub stage_id: String,
    pub run_id: String,
    pub workspace_id: String,
    pub source_a_sha256_hex: String,
    pub source_b_sha256_hex: String,
    pub result_artifact_sha256_hex: String,
    pub result_semantic_sha256_hex: String,
}

/// The scan is a recovery inventory, not a promotion or deletion operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StageStatus {
    ReadyVerified,
    Interrupted,
    Corrupt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StageInventoryEntry {
    pub stage_id: String,
    pub status: StageStatus,
}

fn invalid(message: &str) -> PersistenceError {
    PersistenceError::Configuration(format!("Normal Data staging: {message}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut hex, "{byte:02x}").expect("hex to String");
    }
    hex
}

fn valid_digest(digest: &str) -> bool {
    digest.len() == 64 && digest.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

fn uuid(value: &str) -> Result<(), PersistenceError> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| invalid("invalid artifact identity"))
}

fn safe_directory(path: &Path) -> Result<(), PersistenceError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid("staging directory must be a real directory"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o022 != 0 {
            return Err(invalid("staging root is writable by another Unix user"));
        }
    }
    Ok(())
}

fn regular_file(path: &Path) -> Result<(), PersistenceError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(invalid("artifact must be a regular non-symlink file"));
    }
    Ok(())
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, PersistenceError> {
    regular_file(path)?;
    let size = fs::metadata(path)?.len();
    if size > maximum as u64 {
        return Err(invalid("staged artifact exceeds byte limit"));
    }
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(invalid("staged artifact grew beyond byte limit"));
    }
    Ok(bytes)
}

fn artifact(
    name: &str,
    bytes: &[u8],
    source: Option<&SourceMaterial>,
) -> Artifact {
    Artifact {
        file_name: name.to_owned(),
        size_bytes: bytes.len() as u64,
        sha256_hex: sha256_hex(bytes),
        dataset_version_id: source.map(|s| s.dataset_version_id.clone()),
        document_id: source.map(|s| s.document_id.clone()),
        content_version_id: source.map(|s| s.content_version_id.clone()),
    }
}

fn checked_manifest(
    stage_id: &str,
    material: &PreservationMaterial,
) -> Result<Manifest, PersistenceError> {
    for id in [&material.run_id, &material.workspace_id, &material.recipe_version_id] {
        uuid(id)?;
    }
    for source in [&material.source_a, &material.source_b] {
        for id in [
            &source.dataset_version_id,
            &source.document_id,
            &source.content_version_id,
        ] {
            uuid(id)?;
        }
        if source.bytes.is_empty()
            || source.bytes.len() > MAX_SOURCE_BYTES
            || !valid_digest(&source.sha256_hex)
            || sha256_hex(&source.bytes) != source.sha256_hex
        {
            return Err(invalid("source data does not match frozen digest or size"));
        }
    }
    let output = &material.result;
    if output.bytes.is_empty()
        || output.bytes.len() > MAX_RESULT_BYTES
        || !valid_digest(&output.semantic_result_sha256_hex)
        || !valid_digest(&output.artifact_sha256_hex)
        || sha256_hex(&output.bytes) != output.artifact_sha256_hex
    {
        return Err(invalid("serialized comparison result is invalid"));
    }
    let result: ComparisonResult =
        serde_json::from_slice(&output.bytes).map_err(|_| invalid("result JSON is invalid"))?;
    normal_data_comparison::verify_stored_result_digest(&result)
        .map_err(|_| invalid("result semantic digest is invalid"))?;
    if result.result_sha256_hex != output.semantic_result_sha256_hex {
        return Err(invalid("result calculation digest is not frozen"));
    }
    Ok(Manifest {
        format_version: MANIFEST_VERSION,
        stage_id: stage_id.to_owned(),
        run_id: material.run_id.clone(),
        workspace_id: material.workspace_id.clone(),
        recipe_version_id: material.recipe_version_id.clone(),
        source_a: artifact("source-a.bin", &material.source_a.bytes, Some(&material.source_a)),
        source_b: artifact("source-b.bin", &material.source_b.bytes, Some(&material.source_b)),
        result: artifact("result.json", &output.bytes, None),
        result_semantic_sha256_hex: output.semantic_result_sha256_hex.clone(),
    })
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), PersistenceError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn sync_dir(path: &Path) -> Result<(), PersistenceError> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()?;
    }
    #[cfg(windows)]
    {
        // Rust's portable File::open cannot reliably fsync directories on
        // Windows. Rename provides visibility atomicity, not a power-loss
        // durability guarantee. This staging is NOT formal evidence retention.
        let _ = path;
    }
    Ok(())
}

fn package_path(root: &Path, stage_id: &str, suffix: &str) -> Result<PathBuf, PersistenceError> {
    uuid(stage_id)?;
    Ok(root.join(format!("{stage_id}.{suffix}")))
}

fn verify_artifact(
    folder: &Path,
    entry: &Artifact,
    expected_name: &str,
    maximum: usize,
    requires_identity: bool,
) -> Result<Vec<u8>, PersistenceError> {
    if entry.file_name != expected_name
        || entry.size_bytes == 0
        || entry.size_bytes > maximum as u64
        || !valid_digest(&entry.sha256_hex)
    {
        return Err(invalid("manifest artifact descriptor is invalid"));
    }
    if requires_identity {
        for value in [
            entry.dataset_version_id.as_deref(),
            entry.document_id.as_deref(),
            entry.content_version_id.as_deref(),
        ] {
            uuid(value.ok_or_else(|| invalid("source binding is missing"))?)?;
        }
    } else if entry.dataset_version_id.is_some()
        || entry.document_id.is_some()
        || entry.content_version_id.is_some()
    {
        return Err(invalid("result cannot masquerade as a source"));
    }
    let bytes = read_bounded(&folder.join(expected_name), maximum)?;
    if bytes.len() as u64 != entry.size_bytes || sha256_hex(&bytes) != entry.sha256_hex {
        return Err(invalid("staged artifact bytes do not match manifest"));
    }
    Ok(bytes)
}

fn verify_package(root: &Path, stage_id: &str) -> Result<StageReceipt, PersistenceError> {
    safe_directory(root)?;
    let folder = package_path(root, stage_id, "ready")?;
    safe_directory(&folder)?;
    let manifest_bytes = read_bounded(&folder.join("manifest.json"), MAX_MANIFEST_BYTES)?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| invalid("staging manifest is invalid"))?;
    if manifest.format_version != MANIFEST_VERSION || manifest.stage_id != stage_id {
        return Err(invalid("staging manifest version or identity mismatch"));
    }
    for id in [
        &manifest.run_id,
        &manifest.workspace_id,
        &manifest.recipe_version_id,
    ] {
        uuid(id)?;
    }
    let _source_a = verify_artifact(
        &folder, &manifest.source_a, "source-a.bin", MAX_SOURCE_BYTES, true,
    )?;
    let _source_b = verify_artifact(
        &folder, &manifest.source_b, "source-b.bin", MAX_SOURCE_BYTES, true,
    )?;
    let result_bytes = verify_artifact(
        &folder, &manifest.result, "result.json", MAX_RESULT_BYTES, false,
    )?;
    if !valid_digest(&manifest.result_semantic_sha256_hex) {
        return Err(invalid("semantic result digest format is invalid"));
    }
    let result: ComparisonResult = serde_json::from_slice(&result_bytes)
        .map_err(|_| invalid("staged result JSON is invalid"))?;
    normal_data_comparison::verify_stored_result_digest(&result)
        .map_err(|_| invalid("staged result semantic digest is invalid"))?;
    if result.result_sha256_hex != manifest.result_semantic_sha256_hex {
        return Err(invalid("staged result semantic digest mismatch"));
    }
    Ok(StageReceipt {
        stage_id: stage_id.to_owned(),
        run_id: manifest.run_id,
        workspace_id: manifest.workspace_id,
        source_a_sha256_hex: manifest.source_a.sha256_hex,
        source_b_sha256_hex: manifest.source_b.sha256_hex,
        result_artifact_sha256_hex: manifest.result.sha256_hex,
        result_semantic_sha256_hex: manifest.result_semantic_sha256_hex,
    })
}

/// Write exact native-memory bytes to a new private staging directory. An
/// incomplete directory uses `<UUID>.partial` and is never considered ready.
/// Three fsynced files + an fsynced identity-only manifest are verified before
/// a same-directory rename to `<UUID>.ready`. A failed write cleans up only
/// its own fresh partial directory; a crash may leave an incomplete directory
/// that the recovery inventory reports. No DB or controlled evidence mutation.
///
/// Only trusted native code can choose `root`; never expose this as a Tauri
/// API or use a frontend path. The caller must provide an access-controlled,
/// existing, non-symlink staging root on a local filesystem. Directory fsync
/// is not portable to Windows; this is crash-*inspectable* staging, not an
/// end-to-end durable/power-loss-safe evidence capture guarantee.
pub(crate) fn stage_material(
    root: &Path,
    material: &PreservationMaterial,
) -> Result<StageReceipt, PersistenceError> {
    safe_directory(root)?;
    let stage_id = Uuid::new_v4().to_string();
    let manifest = checked_manifest(&stage_id, material)?;
    let encoded_manifest = serde_json::to_vec(&manifest)
        .map_err(|_| invalid("could not encode staging manifest"))?;
    if encoded_manifest.len() > MAX_MANIFEST_BYTES {
        return Err(invalid("staging manifest exceeds byte limit"));
    }
    let partial = package_path(root, &stage_id, "partial")?;
    let ready = package_path(root, &stage_id, "ready")?;
    if ready.exists() {
        return Err(invalid("staging identifier already exists"));
    }
    fs::create_dir(&partial)?;
    let result = (|| -> Result<(), PersistenceError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&partial, fs::Permissions::from_mode(0o700))?;
        }
        write_synced(&partial.join("source-a.bin"), &material.source_a.bytes)?;
        write_synced(&partial.join("source-b.bin"), &material.source_b.bytes)?;
        write_synced(&partial.join("result.json"), &material.result.bytes)?;
        write_synced(&partial.join("manifest.json"), &encoded_manifest)?;
        sync_dir(&partial)?;
        // Verify the exact staged bytes before publishing the ready marker.
        let a = verify_artifact(
            &partial, &manifest.source_a, "source-a.bin", MAX_SOURCE_BYTES, true,
        )?;
        let b = verify_artifact(
            &partial, &manifest.source_b, "source-b.bin", MAX_SOURCE_BYTES, true,
        )?;
        let result_bytes = verify_artifact(
            &partial, &manifest.result, "result.json", MAX_RESULT_BYTES, false,
        )?;
        if a != material.source_a.bytes
            || b != material.source_b.bytes
            || result_bytes != material.result.bytes
        {
            return Err(invalid("staged data is not the exact supplied material"));
        }
        if ready.exists() {
            return Err(invalid("stage destination already exists"));
        }
        fs::rename(&partial, &ready)?;
        sync_dir(root)?;
        Ok(())
    })();
    if let Err(error) = result {
        // Never delete a published ready package if a post-rename directory
        // fsync failed: recovery can inspect it without trusting the result.
        let _ = fs::remove_dir_all(&partial);
        return Err(error);
    }
    verify_package(root, &stage_id)
}

/// Inspect a known staged package by re-reading all three disk artifacts.
/// Returns no raw bytes and never creates an evidence/approval record.
pub(crate) fn inspect_stage(
    root: &Path,
    stage_id: &str,
) -> Result<StageReceipt, PersistenceError> {
    verify_package(root, stage_id)
}

/// Recovery inventory. No mutation, automatic deletion, or authorization.
/// A '.partial' directory is always incomplete, even if all files happen to
/// be present; any '.ready' directory must pass exact-byte verification.
pub(crate) fn scan_stages(
    root: &Path,
) -> Result<Vec<StageInventoryEntry>, PersistenceError> {
    safe_directory(root)?;
    let mut output = Vec::new();
    let mut entries_seen = 0usize;
    for entry in fs::read_dir(root)? {
        entries_seen += 1;
        if entries_seen > MAX_SCAN_ENTRIES {
            return Err(invalid("staging inventory exceeds safe scan limit"));
        }
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some((id, suffix)) = name.rsplit_once('.') else {
            continue;
        };
        if uuid(id).is_err() || !matches!(suffix, "ready" | "partial") {
            continue;
        }
        let status = if suffix == "partial" {
            StageStatus::Interrupted
        } else if verify_package(root, id).is_ok() {
            StageStatus::ReadyVerified
        } else {
            StageStatus::Corrupt
        };
        output.push(StageInventoryEntry {
            stage_id: id.to_owned(),
            status,
        });
    }
    output.sort_by(|a, b| a.stage_id.cmp(&b.stage_id));
    Ok(output)
}

/// Discard ONLY a known incomplete package. A ready package is never deleted
/// by recovery; future authorized retention controls must own that lifecycle.
pub(crate) fn discard_interrupted(
    root: &Path,
    stage_id: &str,
) -> Result<(), PersistenceError> {
    safe_directory(root)?;
    let partial = package_path(root, stage_id, "partial")?;
    safe_directory(&partial)?;
    fs::remove_dir_all(partial)?;
    sync_dir(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("pdox-staging-test-{}", Uuid::new_v4()));
            fs::create_dir(&path).expect("create dedicated staging root");
            Self(path)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn scan_identifies_incomplete_and_corrupt_ready_and_discards_only_partial() {
        let root = TestRoot::new();
        let interrupted_id = Uuid::new_v4().to_string();
        let corrupted_id = Uuid::new_v4().to_string();
        fs::create_dir(root.0.join(format!("{interrupted_id}.partial")))
            .expect("orphan unfinished stage");
        fs::create_dir(root.0.join(format!("{corrupted_id}.ready")))
            .expect("orphan malformed ready stage");
        let inventory = scan_stages(&root.0).expect("scan after interrupted write");
        assert_eq!(
            inventory,
            vec![
                StageInventoryEntry {
                    stage_id: interrupted_id.clone(),
                    status: StageStatus::Interrupted
                },
                StageInventoryEntry {
                    stage_id: corrupted_id.clone(),
                    status: StageStatus::Corrupt
                }
            ].into_iter().collect::<Vec<_>>()
        );
        assert!(inspect_stage(&root.0, &interrupted_id).is_err());
        assert!(inspect_stage(&root.0, &corrupted_id).is_err());
        assert!(discard_interrupted(&root.0, &corrupted_id).is_err(),
            "recovery must not delete ready directories");
        assert!(discard_interrupted(&root.0, "../other").is_err(),
            "recovery cannot accept path traversal");
        discard_interrupted(&root.0, &interrupted_id)
            .expect("clean incomplete package by exact generated ID");
        assert!(!root.0.join(format!("{interrupted_id}.partial")).exists());
        assert!(root.0.join(format!("{corrupted_id}.ready")).exists(),
            "corrupt ready package remains for explicit examination");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_staging_root_and_insecure_directory() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = TestRoot::new();
        let alias = root.0.with_extension("alias");
        symlink(&root.0, &alias).expect("symlink root");
        assert!(scan_stages(&alias).is_err());
        fs::remove_file(alias).expect("remove symlink");
        fs::set_permissions(&root.0, fs::Permissions::from_mode(0o777))
            .expect("make untrusted root writable");
        assert!(scan_stages(&root.0).is_err());
    }
}
