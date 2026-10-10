//! Native-only durable Normal Data retention-candidate store.
//!
//! A candidate preserves exact staged source A/source B/result bytes for crash
//! recovery, but is deliberately NOT controlled evidence and carries no
//! authentication, approval, workpaper linkage, signoff or promotion authority.
//! A hostile local database/filesystem owner remains outside this integrity model.
use crate::{
    normal_data_comparison::{self, ComparisonResult},
    normal_data_preservation_material::{PreservationMaterial, SourceMaterial},
    normal_data_provenance::{self, RunProvenanceReceipt},
    normal_data_staging::{self, ExpectedStageRun},
    persistence::{self, PersistenceError},
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt::Write,
    fs::{self, File, OpenOptions},
    io::{Read, Write as IoWrite},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const FORMAT_VERSION: u8 = 1;
const MAX_SOURCE_BYTES: usize = 32 * 1024 * 1024;
const MAX_RESULT_BYTES: usize = 8 * 1024 * 1024;
const MAX_MANIFEST_BYTES: usize = 16 * 1024;
const MAX_INVENTORY_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SourceDescriptor {
    file_name: String,
    size_bytes: u64,
    sha256_hex: String,
    dataset_version_id: String,
    document_id: String,
    content_version_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ResultDescriptor {
    file_name: String,
    size_bytes: u64,
    sha256_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CandidateManifest {
    format_version: u8,
    stage_id: String,
    run_id: String,
    workspace_id: String,
    recipe_version_id: String,
    source_a: SourceDescriptor,
    source_b: SourceDescriptor,
    result: ResultDescriptor,
    result_semantic_sha256_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetentionCandidateRecord {
    pub stage_id: String,
    pub run_id: String,
    pub workspace_id: String,
    pub recipe_version_id: String,
    source_a_dataset_version_id: String,
    source_a_document_id: String,
    source_a_content_version_id: String,
    source_a_sha256_hex: String,
    source_a_size_bytes: u64,
    source_b_dataset_version_id: String,
    source_b_document_id: String,
    source_b_content_version_id: String,
    source_b_sha256_hex: String,
    source_b_size_bytes: u64,
    pub result_artifact_sha256_hex: String,
    pub result_semantic_sha256_hex: String,
    result_size_bytes: u64,
    pub retained_at_ms: i64,
}

/// Metadata only; a source identity is not a controlled-evidence version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidateOriginalArtifact {
    pub dataset_version_id: String,
    pub document_id: String,
    pub content_version_id: String,
    pub sha256_hex: String,
    pub size_bytes: u64,
}

/// The serialized result hash is deliberately separate from its semantic hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidateResultArtifact {
    pub artifact_sha256_hex: String,
    pub semantic_sha256_hex: String,
    pub size_bytes: u64,
}

/// A non-authorizing, three-role metadata projection of a verified candidate.
/// Construct only after `inspect_registered_candidate_on_connection` has
/// rebound the ledger, bytes and manifest to the exact historical run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidateArtifactSet {
    pub original_a: CandidateOriginalArtifact,
    pub original_b: CandidateOriginalArtifact,
    pub frozen_result: CandidateResultArtifact,
}

impl RetentionCandidateRecord {
    pub(crate) fn describe_three_artifacts(&self) -> CandidateArtifactSet {
        CandidateArtifactSet {
            original_a: CandidateOriginalArtifact {
                dataset_version_id: self.source_a_dataset_version_id.clone(),
                document_id: self.source_a_document_id.clone(),
                content_version_id: self.source_a_content_version_id.clone(),
                sha256_hex: self.source_a_sha256_hex.clone(),
                size_bytes: self.source_a_size_bytes,
            },
            original_b: CandidateOriginalArtifact {
                dataset_version_id: self.source_b_dataset_version_id.clone(),
                document_id: self.source_b_document_id.clone(),
                content_version_id: self.source_b_content_version_id.clone(),
                sha256_hex: self.source_b_sha256_hex.clone(),
                size_bytes: self.source_b_size_bytes,
            },
            frozen_result: CandidateResultArtifact {
                artifact_sha256_hex: self.result_artifact_sha256_hex.clone(),
                semantic_sha256_hex: self.result_semantic_sha256_hex.clone(),
                size_bytes: self.result_size_bytes,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetentionCandidateStatus {
    RecordedValid,
    OrphanValid,
    Interrupted,
    Corrupt,
    RecordedMissing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetentionCandidateInventoryEntry {
    pub stage_id: String,
    pub status: RetentionCandidateStatus,
}

struct ExpectedOwned {
    provenance: RunProvenanceReceipt,
    source_a_document_id: String,
    source_b_document_id: String,
    result_artifact_sha256_hex: String,
}

fn denied(message: &str) -> PersistenceError {
    PersistenceError::Configuration(format!(
        "Normal Data retention candidate unavailable: {message}"
    ))
}

fn valid_uuid(value: &str) -> Result<(), PersistenceError> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| denied("invalid identifier"))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn digest_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut output, "{byte:02x}").expect("hex to String");
    }
    output
}

fn safe_directory(path: &Path) -> Result<(), PersistenceError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(denied("retention root must be a real directory"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o022 != 0 {
            return Err(denied("retention root is writable by another Unix user"));
        }
    }
    Ok(())
}

fn regular_file(path: &Path) -> Result<(), PersistenceError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(denied(
            "candidate artifact must be a regular non-symlink file",
        ));
    }
    Ok(())
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, PersistenceError> {
    regular_file(path)?;
    if fs::metadata(path)?.len() > maximum as u64 {
        return Err(denied("candidate artifact exceeds byte limit"));
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(denied("candidate artifact grew beyond byte limit"));
    }
    Ok(bytes)
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
        // Portable Rust does not provide a reliable Windows directory fsync.
        // This limitation keeps candidates ineligible for formal evidence.
        let _ = path;
    }
    Ok(())
}

fn package_path(root: &Path, stage_id: &str, suffix: &str) -> Result<PathBuf, PersistenceError> {
    valid_uuid(stage_id)?;
    Ok(root.join(format!("{stage_id}.{suffix}")))
}

fn source_descriptor(name: &str, source: &SourceMaterial) -> SourceDescriptor {
    SourceDescriptor {
        file_name: name.to_string(),
        size_bytes: source.bytes.len() as u64,
        sha256_hex: source.sha256_hex.clone(),
        dataset_version_id: source.dataset_version_id.clone(),
        document_id: source.document_id.clone(),
        content_version_id: source.content_version_id.clone(),
    }
}

fn manifest_for(stage_id: &str, material: &PreservationMaterial) -> CandidateManifest {
    CandidateManifest {
        format_version: FORMAT_VERSION,
        stage_id: stage_id.to_string(),
        run_id: material.run_id.clone(),
        workspace_id: material.workspace_id.clone(),
        recipe_version_id: material.recipe_version_id.clone(),
        source_a: source_descriptor("source-a.bin", &material.source_a),
        source_b: source_descriptor("source-b.bin", &material.source_b),
        result: ResultDescriptor {
            file_name: "result.json".to_string(),
            size_bytes: material.result.bytes.len() as u64,
            sha256_hex: material.result.artifact_sha256_hex.clone(),
        },
        result_semantic_sha256_hex: material.result.semantic_result_sha256_hex.clone(),
    }
}

fn verify_source(
    folder: &Path,
    source: &SourceDescriptor,
    expected_name: &str,
) -> Result<Vec<u8>, PersistenceError> {
    if source.file_name != expected_name
        || source.size_bytes == 0
        || source.size_bytes > MAX_SOURCE_BYTES as u64
        || !valid_digest(&source.sha256_hex)
    {
        return Err(denied("invalid retained source descriptor"));
    }
    for id in [
        &source.dataset_version_id,
        &source.document_id,
        &source.content_version_id,
    ] {
        valid_uuid(id)?;
    }
    let bytes = read_bounded(&folder.join(expected_name), MAX_SOURCE_BYTES)?;
    if bytes.len() as u64 != source.size_bytes || digest_hex(&bytes) != source.sha256_hex {
        return Err(denied("retained source bytes do not match manifest"));
    }
    Ok(bytes)
}

fn verify_candidate_folder(
    folder: &Path,
    stage_id: &str,
) -> Result<CandidateManifest, PersistenceError> {
    safe_directory(folder)?;
    let manifest_bytes = read_bounded(&folder.join("candidate-manifest.json"), MAX_MANIFEST_BYTES)?;
    let manifest: CandidateManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| denied("candidate manifest is invalid"))?;
    if manifest.format_version != FORMAT_VERSION || manifest.stage_id != stage_id {
        return Err(denied("candidate manifest identity/version mismatch"));
    }
    for id in [
        &manifest.stage_id,
        &manifest.run_id,
        &manifest.workspace_id,
        &manifest.recipe_version_id,
    ] {
        valid_uuid(id)?;
    }
    let _source_a = verify_source(folder, &manifest.source_a, "source-a.bin")?;
    let _source_b = verify_source(folder, &manifest.source_b, "source-b.bin")?;
    if manifest.result.file_name != "result.json"
        || manifest.result.size_bytes == 0
        || manifest.result.size_bytes > MAX_RESULT_BYTES as u64
        || !valid_digest(&manifest.result.sha256_hex)
        || !valid_digest(&manifest.result_semantic_sha256_hex)
    {
        return Err(denied("invalid retained result descriptor"));
    }
    let result_bytes = read_bounded(&folder.join("result.json"), MAX_RESULT_BYTES)?;
    if result_bytes.len() as u64 != manifest.result.size_bytes
        || digest_hex(&result_bytes) != manifest.result.sha256_hex
    {
        return Err(denied("retained result bytes do not match manifest"));
    }
    let result: ComparisonResult = serde_json::from_slice(&result_bytes)
        .map_err(|_| denied("retained result JSON invalid"))?;
    normal_data_comparison::verify_stored_result_digest(&result)
        .map_err(|_| denied("retained result semantic digest invalid"))?;
    if result.result_sha256_hex != manifest.result_semantic_sha256_hex {
        return Err(denied("retained result semantic digest mismatch"));
    }
    Ok(manifest)
}

fn verify_candidate(root: &Path, stage_id: &str) -> Result<CandidateManifest, PersistenceError> {
    safe_directory(root)?;
    let folder = package_path(root, stage_id, "candidate")?;
    verify_candidate_folder(&folder, stage_id)
}

fn frozen_document_id(
    connection: &Connection,
    dataset_version_id: &str,
    content_version_id: &str,
) -> Result<String, PersistenceError> {
    connection
        .query_row(
            "SELECT v.document_id
             FROM normal_data_dataset_versions v
             JOIN content_versions cv
               ON cv.content_version_id = v.content_version_id
              AND cv.document_id = v.document_id
              AND cv.file_instance_id = v.file_instance_id
             WHERE v.normal_data_dataset_version_id = ?1
               AND v.content_version_id = ?2
               AND v.source_verification_state = 'HASH_VERIFIED'
               AND cv.verification_state = 'HASH_VERIFIED'
               AND v.source_sha256 = cv.sha256",
            params![dataset_version_id, content_version_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| denied("source identity is not frozen and hash verified"))
}

fn exact_result_artifact_hash(
    connection: &Connection,
    run_id: &str,
    recipe_version_id: &str,
) -> Result<String, PersistenceError> {
    let json: Option<String> = connection
        .query_row(
            "SELECT result_json FROM normal_data_comparison_runs
             WHERE normal_data_comparison_run_id = ?1
               AND normal_data_comparison_recipe_version_id = ?2",
            params![run_id, recipe_version_id],
            |row| row.get(0),
        )
        .optional()?;
    let json = json.ok_or_else(|| denied("exact frozen result is unavailable"))?;
    if json.is_empty() || json.len() > MAX_RESULT_BYTES {
        return Err(denied("exact frozen result exceeds byte limit"));
    }
    Ok(digest_hex(json.as_bytes()))
}

fn expected_on_connection(
    connection: &Connection,
    run_id: &str,
) -> Result<ExpectedOwned, PersistenceError> {
    let provenance = normal_data_provenance::inspect_run_on_connection(connection, run_id)?;
    let source_a_document_id = frozen_document_id(
        connection,
        &provenance.source_a.dataset_version_id,
        &provenance.source_a.content_version_id,
    )?;
    let source_b_document_id = frozen_document_id(
        connection,
        &provenance.source_b.dataset_version_id,
        &provenance.source_b.content_version_id,
    )?;
    let result_artifact_sha256_hex = exact_result_artifact_hash(
        connection,
        &provenance.normal_data_comparison_run_id,
        &provenance.normal_data_comparison_recipe_version_id,
    )?;
    Ok(ExpectedOwned {
        provenance,
        source_a_document_id,
        source_b_document_id,
        result_artifact_sha256_hex,
    })
}

fn manifest_matches_expected(manifest: &CandidateManifest, expected: &ExpectedOwned) -> bool {
    let frozen = &expected.provenance;
    manifest.run_id == frozen.normal_data_comparison_run_id
        && manifest.workspace_id == frozen.normal_data_workspace_id
        && manifest.recipe_version_id == frozen.normal_data_comparison_recipe_version_id
        && manifest.source_a.dataset_version_id == frozen.source_a.dataset_version_id
        && manifest.source_a.document_id == expected.source_a_document_id
        && manifest.source_a.content_version_id == frozen.source_a.content_version_id
        && manifest.source_a.sha256_hex == frozen.source_a.sha256_hex
        && manifest.source_b.dataset_version_id == frozen.source_b.dataset_version_id
        && manifest.source_b.document_id == expected.source_b_document_id
        && manifest.source_b.content_version_id == frozen.source_b.content_version_id
        && manifest.source_b.sha256_hex == frozen.source_b.sha256_hex
        && manifest.result.sha256_hex == expected.result_artifact_sha256_hex
        && manifest.result_semantic_sha256_hex == frozen.result_sha256_hex
}

fn record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RetentionCandidateRecord> {
    Ok(RetentionCandidateRecord {
        stage_id: row.get(0)?,
        run_id: row.get(1)?,
        workspace_id: row.get(2)?,
        recipe_version_id: row.get(3)?,
        source_a_dataset_version_id: row.get(4)?,
        source_a_document_id: row.get(5)?,
        source_a_content_version_id: row.get(6)?,
        source_a_sha256_hex: row.get(7)?,
        source_a_size_bytes: row.get::<_, i64>(8)? as u64,
        source_b_dataset_version_id: row.get(9)?,
        source_b_document_id: row.get(10)?,
        source_b_content_version_id: row.get(11)?,
        source_b_sha256_hex: row.get(12)?,
        source_b_size_bytes: row.get::<_, i64>(13)? as u64,
        result_artifact_sha256_hex: row.get(14)?,
        result_semantic_sha256_hex: row.get(15)?,
        result_size_bytes: row.get::<_, i64>(16)? as u64,
        retained_at_ms: row.get(17)?,
    })
}

fn load_record(
    connection: &Connection,
    stage_id: &str,
) -> Result<Option<RetentionCandidateRecord>, PersistenceError> {
    connection
        .query_row(
            "SELECT stage_id, normal_data_comparison_run_id, normal_data_workspace_id,
                    normal_data_comparison_recipe_version_id,
                    source_a_dataset_version_id, source_a_document_id,
                    source_a_content_version_id, source_a_sha256_hex, source_a_size_bytes,
                    source_b_dataset_version_id, source_b_document_id,
                    source_b_content_version_id, source_b_sha256_hex, source_b_size_bytes,
                    result_artifact_sha256_hex, result_semantic_sha256_hex,
                    result_size_bytes, retained_at_ms
             FROM normal_data_retention_candidates WHERE stage_id = ?1",
            [stage_id],
            record_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn record_matches_manifest(
    record: &RetentionCandidateRecord,
    manifest: &CandidateManifest,
) -> bool {
    record.stage_id == manifest.stage_id
        && record.run_id == manifest.run_id
        && record.workspace_id == manifest.workspace_id
        && record.recipe_version_id == manifest.recipe_version_id
        && record.source_a_dataset_version_id == manifest.source_a.dataset_version_id
        && record.source_a_document_id == manifest.source_a.document_id
        && record.source_a_content_version_id == manifest.source_a.content_version_id
        && record.source_a_sha256_hex == manifest.source_a.sha256_hex
        && record.source_a_size_bytes == manifest.source_a.size_bytes
        && record.source_b_dataset_version_id == manifest.source_b.dataset_version_id
        && record.source_b_document_id == manifest.source_b.document_id
        && record.source_b_content_version_id == manifest.source_b.content_version_id
        && record.source_b_sha256_hex == manifest.source_b.sha256_hex
        && record.source_b_size_bytes == manifest.source_b.size_bytes
        && record.result_artifact_sha256_hex == manifest.result.sha256_hex
        && record.result_semantic_sha256_hex == manifest.result_semantic_sha256_hex
        && record.result_size_bytes == manifest.result.size_bytes
}

fn register_verified_candidate(
    database_path: &Path,
    manifest: &CandidateManifest,
) -> Result<RetentionCandidateRecord, PersistenceError> {
    let mut connection = persistence::open_configured_connection(database_path)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let expected = expected_on_connection(&tx, &manifest.run_id)?;
    if !manifest_matches_expected(manifest, &expected) {
        return Err(denied("candidate does not match the exact immutable run"));
    }
    if let Some(existing) = load_record(&tx, &manifest.stage_id)? {
        if !record_matches_manifest(&existing, manifest) {
            return Err(denied(
                "stored candidate metadata conflicts with retained bytes",
            ));
        }
        tx.commit()?;
        return Ok(existing);
    }

    let retained_at_ms = persistence::now_unix_ms()?;
    tx.execute(
        "INSERT INTO normal_data_retention_candidates (
            stage_id, normal_data_comparison_run_id, normal_data_workspace_id,
            normal_data_comparison_recipe_version_id,
            source_a_dataset_version_id, source_a_document_id,
            source_a_content_version_id, source_a_sha256_hex, source_a_size_bytes,
            source_b_dataset_version_id, source_b_document_id,
            source_b_content_version_id, source_b_sha256_hex, source_b_size_bytes,
            result_artifact_sha256_hex, result_semantic_sha256_hex,
            result_size_bytes, retained_at_ms
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
            ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18
         )",
        params![
            manifest.stage_id,
            manifest.run_id,
            manifest.workspace_id,
            manifest.recipe_version_id,
            manifest.source_a.dataset_version_id,
            manifest.source_a.document_id,
            manifest.source_a.content_version_id,
            manifest.source_a.sha256_hex,
            manifest.source_a.size_bytes as i64,
            manifest.source_b.dataset_version_id,
            manifest.source_b.document_id,
            manifest.source_b.content_version_id,
            manifest.source_b.sha256_hex,
            manifest.source_b.size_bytes as i64,
            manifest.result.sha256_hex,
            manifest.result_semantic_sha256_hex,
            manifest.result.size_bytes as i64,
            retained_at_ms,
        ],
    )?;
    let record = load_record(&tx, &manifest.stage_id)?
        .ok_or_else(|| denied("candidate registration did not persist"))?;
    tx.commit()?;
    Ok(record)
}

fn write_candidate_package(
    retention_root: &Path,
    stage_id: &str,
    material: &PreservationMaterial,
) -> Result<(), PersistenceError> {
    safe_directory(retention_root)?;
    let partial = package_path(retention_root, stage_id, "partial")?;
    let candidate = package_path(retention_root, stage_id, "candidate")?;
    if candidate.exists() {
        return Ok(());
    }
    if partial.exists() {
        return Err(denied("interrupted candidate requires explicit recovery"));
    }

    let manifest = manifest_for(stage_id, material);
    let manifest_bytes =
        serde_json::to_vec(&manifest).map_err(|_| denied("could not encode candidate manifest"))?;
    if manifest_bytes.len() > MAX_MANIFEST_BYTES {
        return Err(denied("candidate manifest exceeds byte limit"));
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
        write_synced(&partial.join("candidate-manifest.json"), &manifest_bytes)?;
        sync_dir(&partial)?;
        let verified = verify_candidate_folder(&partial, stage_id)?;
        if verified != manifest {
            return Err(denied("candidate verification changed metadata"));
        }
        if candidate.exists() {
            return Err(denied("candidate destination already exists"));
        }
        fs::rename(&partial, &candidate)?;
        sync_dir(retention_root)?;
        Ok(())
    })();

    if let Err(error) = result {
        // Only an unpublished partial is cleaned after an ordinary error.
        // A published candidate is never deleted automatically.
        let _ = fs::remove_dir_all(&partial);
        return Err(error);
    }
    let verified = verify_candidate(retention_root, stage_id)?;
    if verified != manifest {
        return Err(denied("published candidate verification failed"));
    }
    Ok(())
}

/// Preserve a ready stage as an idempotent retention candidate. The stage is
/// first bound to exact immutable SQLite provenance and exact result JSON bytes.
/// The published package is then independently reverified and registered.
///
/// Filesystem publication happens before the SQLite registration. If the
/// process/database fails between those operations, the immutable package is
/// intentionally left as an ORPHAN_VALID recovery candidate. No formal evidence
/// row, workpaper link, approval or signoff is created.
pub(crate) fn retain_staged_candidate(
    database_path: &Path,
    staging_root: &Path,
    retention_root: &Path,
    stage_id: &str,
    run_id: &str,
) -> Result<RetentionCandidateRecord, PersistenceError> {
    valid_uuid(stage_id)?;
    valid_uuid(run_id)?;
    safe_directory(retention_root)?;
    let candidate = package_path(retention_root, stage_id, "candidate")?;
    if candidate.exists() {
        let record = recover_retained_candidate(database_path, retention_root, stage_id)?;
        if record.run_id != run_id {
            return Err(denied("existing candidate belongs to another run"));
        }
        return Ok(record);
    }

    let connection = persistence::open_configured_connection(database_path)?;
    let expected_owned = expected_on_connection(&connection, run_id)?;
    let expected = ExpectedStageRun {
        provenance: &expected_owned.provenance,
        source_a_document_id: &expected_owned.source_a_document_id,
        source_b_document_id: &expected_owned.source_b_document_id,
        result_artifact_sha256_hex: &expected_owned.result_artifact_sha256_hex,
    };
    let verified = normal_data_staging::read_stage_against_run(staging_root, stage_id, &expected)?;
    if verified.receipt.stage_id != stage_id || verified.receipt.run_id != run_id {
        return Err(denied("verified stage identity changed"));
    }
    write_candidate_package(retention_root, stage_id, &verified.material)?;
    recover_retained_candidate(database_path, retention_root, stage_id)
}

/// Reconcile one already-published candidate after a crash. Every retained
/// byte is rehashed, its manifest is checked against immutable SQLite run/source
/// identity, and registration is idempotent. This still does not confer any
/// evidence or specialist authority.
pub(crate) fn recover_retained_candidate(
    database_path: &Path,
    retention_root: &Path,
    stage_id: &str,
) -> Result<RetentionCandidateRecord, PersistenceError> {
    valid_uuid(stage_id)?;
    let manifest = verify_candidate(retention_root, stage_id)?;
    register_verified_candidate(database_path, &manifest)
}

/// Verify a published candidate without registering or repairing it.
/// Caller MUST hold the same SQLite transaction used for grant/target checks.
/// This method requires an existing v31 record: an orphan is not an approval.
/// Retained bytes may change after the read; no durable authorization is issued.
pub(crate) fn inspect_registered_candidate_on_connection(
    connection: &Connection,
    retention_root: &Path,
    stage_id: &str,
    expected_run_id: &str,
) -> Result<RetentionCandidateRecord, PersistenceError> {
    valid_uuid(stage_id)?;
    valid_uuid(expected_run_id)?;
    let manifest = verify_candidate(retention_root, stage_id)?;
    if manifest.run_id != expected_run_id {
        return Err(denied("retained candidate is for another run"));
    }
    let expected = expected_on_connection(connection, expected_run_id)?;
    if !manifest_matches_expected(&manifest, &expected) {
        return Err(denied("retained bytes do not match frozen provenance"));
    }
    let record = load_record(connection, stage_id)?
        .ok_or_else(|| denied("candidate must be registered before inspection"))?;
    if !record_matches_manifest(&record, &manifest) {
        return Err(denied("candidate ledger does not match retained artifacts"));
    }
    Ok(record)
}

fn stored_status(
    database_path: &Path,
    manifest: &CandidateManifest,
) -> Result<RetentionCandidateStatus, PersistenceError> {
    let connection = persistence::open_configured_connection(database_path)?;
    let expected = match expected_on_connection(&connection, &manifest.run_id) {
        Ok(expected) => expected,
        Err(_) => return Ok(RetentionCandidateStatus::Corrupt),
    };
    if !manifest_matches_expected(manifest, &expected) {
        return Ok(RetentionCandidateStatus::Corrupt);
    }
    match load_record(&connection, &manifest.stage_id)? {
        None => Ok(RetentionCandidateStatus::OrphanValid),
        Some(record) if record_matches_manifest(&record, manifest) => {
            Ok(RetentionCandidateStatus::RecordedValid)
        }
        Some(_) => Ok(RetentionCandidateStatus::Corrupt),
    }
}

/// Bounded recovery inventory across the private candidate root and immutable
/// registration ledger. It never deletes or promotes anything.
pub(crate) fn scan_retention_candidates(
    database_path: &Path,
    retention_root: &Path,
) -> Result<Vec<RetentionCandidateInventoryEntry>, PersistenceError> {
    safe_directory(retention_root)?;
    let mut observed = BTreeMap::new();
    let mut seen = 0usize;
    for entry in fs::read_dir(retention_root)? {
        seen += 1;
        if seen > MAX_INVENTORY_ENTRIES {
            return Err(denied("retention inventory exceeds safe scan limit"));
        }
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some((stage_id, suffix)) = name.rsplit_once('.') else {
            continue;
        };
        if valid_uuid(stage_id).is_err() || !matches!(suffix, "candidate" | "partial") {
            continue;
        }
        let status = if suffix == "partial" {
            RetentionCandidateStatus::Interrupted
        } else {
            match verify_candidate(retention_root, stage_id) {
                Ok(manifest) => stored_status(database_path, &manifest)?,
                Err(_) => RetentionCandidateStatus::Corrupt,
            }
        };
        if observed.insert(stage_id.to_string(), status).is_some() {
            return Err(denied("duplicate candidate identity in retention root"));
        }
    }

    let connection = persistence::open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT stage_id FROM normal_data_retention_candidates
         ORDER BY stage_id LIMIT 10001",
    )?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    let mut recorded = Vec::new();
    for row in rows {
        recorded.push(row?);
    }
    if recorded.len() > MAX_INVENTORY_ENTRIES {
        return Err(denied(
            "registered candidate inventory exceeds safe scan limit",
        ));
    }
    for stage_id in recorded {
        observed
            .entry(stage_id)
            .or_insert(RetentionCandidateStatus::RecordedMissing);
    }

    Ok(observed
        .into_iter()
        .map(|(stage_id, status)| RetentionCandidateInventoryEntry { stage_id, status })
        .collect())
}

/// Delete only an exact interrupted partial candidate. Published candidates are
/// never automatically removed by this recovery layer.
pub(crate) fn discard_interrupted_candidate(
    retention_root: &Path,
    stage_id: &str,
) -> Result<(), PersistenceError> {
    safe_directory(retention_root)?;
    let partial = package_path(retention_root, stage_id, "partial")?;
    safe_directory(&partial)?;
    fs::remove_dir_all(partial)?;
    sync_dir(retention_root)
}
