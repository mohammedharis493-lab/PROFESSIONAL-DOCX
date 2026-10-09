//! Native-only verified dataset byte reader.
//!
//! Dataset versions are immutable identities, not retained file copies. This
//! helper rejects sources without an authoritative SHA-256, resolves only an
//! existing indexed file through the approved-root boundary, and hashes the
//! actual bytes it reads. Nothing here promotes working data to evidence.
use crate::{
    launcher,
    persistence::{self, PersistenceError},
};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};
use uuid::Uuid;

const MAX_DATASET_BYTES: u64 = 32 * 1024 * 1024;

pub struct VerifiedDatasetBytes {
    pub dataset_version_id: String,
    pub document_id: String,
    pub content_version_id: String,
    pub sha256: [u8; 32],
    pub bytes: Vec<u8>,
}

struct FrozenBinding {
    document_id: String,
    file_instance_id: String,
    content_version_id: String,
    size_bytes: i64,
    sha256: Vec<u8>,
}

fn invalid(message: impl Into<String>) -> PersistenceError {
    PersistenceError::Configuration(message.into())
}

/// Revalidates the exact frozen content version against current bytes.
/// No data leaves this native boundary unless every check succeeds.
///
/// Source bytes are transient; no historical availability is guaranteed.
/// A verified result only applies to the exact bytes returned in this call.
pub fn read_verified_dataset(
    database_path: &Path,
    dataset_version_id: &str,
) -> Result<VerifiedDatasetBytes, PersistenceError> {
    Uuid::parse_str(dataset_version_id).map_err(|_| invalid("invalid dataset version UUID"))?;
    let connection = persistence::open_configured_connection(database_path)?;
    let binding: Option<FrozenBinding> = connection
        .query_row(
            "SELECT v.document_id, v.file_instance_id, v.content_version_id,
                v.source_size_bytes, v.source_sha256
         FROM normal_data_dataset_versions v
         JOIN normal_data_datasets nd
           ON nd.normal_data_dataset_id = v.normal_data_dataset_id
         JOIN documents d ON d.document_id = v.document_id
         JOIN file_instances fi
           ON fi.file_instance_id = v.file_instance_id
          AND fi.document_id = v.document_id
         JOIN storage_roots sr ON sr.storage_root_id = fi.storage_root_id
         JOIN content_versions cv
           ON cv.content_version_id = v.content_version_id
          AND cv.file_instance_id = v.file_instance_id
          AND cv.document_id = v.document_id
         WHERE v.normal_data_dataset_version_id = ?1
           AND d.archived_at_ms IS NULL
           AND fi.availability_state = 'AVAILABLE'
           AND sr.availability_state = 'AVAILABLE'
           AND v.source_verification_state = 'HASH_VERIFIED'
           AND v.source_stable_during_read = 1
           AND cv.verification_state = 'HASH_VERIFIED'
           AND cv.source_stable_during_read = 1
           AND cv.size_bytes = v.source_size_bytes
           AND cv.sha256 = v.source_sha256",
            [dataset_version_id],
            |row| {
                Ok(FrozenBinding {
                    document_id: row.get(0)?,
                    file_instance_id: row.get(1)?,
                    content_version_id: row.get(2)?,
                    size_bytes: row.get(3)?,
                    sha256: row.get(4)?,
                })
            },
        )
        .optional()?;
    let binding = binding.ok_or_else(|| {
        invalid("dataset version lacks an available, stable, hash-verified indexed source")
    })?;
    if binding.sha256.len() != 32 || binding.size_bytes < 0 {
        return Err(invalid("dataset source hash or size is invalid"));
    }
    let size =
        u64::try_from(binding.size_bytes).map_err(|_| invalid("dataset source size is invalid"))?;
    if size > MAX_DATASET_BYTES {
        return Err(invalid("dataset exceeds 32 MiB verified-read limit"));
    }

    let source =
        persistence::resolve_file_instance_source(database_path, &binding.file_instance_id)?
            .ok_or_else(|| invalid("indexed file identity is unavailable"))?;
    let validated = launcher::validated_existing_path(&source).map_err(|error| {
        invalid(format!(
            "source outside approved root or unavailable: {error}"
        ))
    })?;
    let mut file = File::open(&validated)?;
    let before = file.metadata()?;
    if !before.is_file() || before.len() != size {
        return Err(invalid(
            "dataset source bytes differ from the frozen indexed size",
        ));
    }
    let modified_before = before.modified().ok();
    let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or_default());
    // A hard read bound also protects against a source growing during the read.
    (&mut file)
        .take(MAX_DATASET_BYTES + 1)
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if bytes.len() as u64 != size
        || after.len() != before.len()
        || (modified_before.is_some() && after.modified().ok() != modified_before)
    {
        return Err(invalid(
            "dataset source was modified during verified reading",
        ));
    }
    let actual = Sha256::digest(&bytes);
    if actual[..] != binding.sha256[..] {
        return Err(invalid(
            "dataset source SHA-256 differs from immutable indexed content version",
        ));
    }
    let mut sha256 = [0u8; 32];
    sha256.copy_from_slice(&actual);
    Ok(VerifiedDatasetBytes {
        dataset_version_id: dataset_version_id.to_string(),
        document_id: binding.document_id,
        content_version_id: binding.content_version_id,
        sha256,
        bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::{fs, path::PathBuf};

    #[cfg(unix)]
    fn encode_relative(path: &Path) -> (Vec<u8>, &'static str) {
        use std::os::unix::ffi::OsStrExt;
        (path.as_os_str().as_bytes().to_vec(), "unix-bytes")
    }
    #[cfg(windows)]
    fn encode_relative(path: &Path) -> (Vec<u8>, &'static str) {
        use std::os::windows::ffi::OsStrExt;
        let bytes = path
            .as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect();
        (bytes, "windows-utf16le")
    }

    struct Fixture {
        folder: PathBuf,
        database_path: PathBuf,
        source_path: PathBuf,
        dataset_version_id: String,
    }
    impl Fixture {
        fn new(verification_state: &str) -> Self {
            let folder =
                std::env::temp_dir().join(format!("pdox-verified-dataset-{}", Uuid::new_v4()));
            fs::create_dir_all(&folder).expect("fixture folder");
            let database_path = folder.join("state.sqlite");
            persistence::initialize_database(&database_path).expect("schema");
            let source_dir = folder.join("source");
            fs::create_dir_all(&source_dir).expect("source root");
            let source_path = source_dir.join("invoices.csv");
            let bytes = b"key,period,amount\nA,2026-08,100\n";
            fs::write(&source_path, bytes).expect("fixture source");
            let canonical = fs::canonicalize(&source_dir).expect("canonical approved root");
            let root_id = Uuid::new_v4().to_string();
            let root = persistence::register_storage_root(
                &database_path,
                &root_id,
                &source_dir,
                &canonical,
            )
            .expect("approve source directory");
            let document_id = Uuid::new_v4().to_string();
            let file_instance_id = Uuid::new_v4().to_string();
            let content_version_id = Uuid::new_v4().to_string();
            let dataset_id = Uuid::new_v4().to_string();
            let dataset_version_id = Uuid::new_v4().to_string();
            let (relative_bytes, encoding) = encode_relative(Path::new("invoices.csv"));
            let digest = Sha256::digest(bytes);
            let conn = persistence::open_configured_connection(&database_path)
                .expect("open fixture database");
            conn.execute(
                "INSERT INTO documents (
                    document_id, storage_state, display_name, created_at_ms
                ) VALUES (?1, 'LINKED', 'invoices.csv', 1)",
                [&document_id],
            )
            .expect("document");
            conn.execute(
                "INSERT INTO file_instances (
                    file_instance_id, document_id, storage_root_id,
                    relative_path_native, path_native_encoding, relative_path_display,
                    relative_path_search, size_bytes, first_seen_at_ms,
                    last_seen_at_ms, availability_state
                ) VALUES (?1, ?2, ?3, ?4, ?5, 'invoices.csv', 'invoices.csv', ?6, 1, 1, 'AVAILABLE')",
                params![&file_instance_id, &document_id, &root.storage_root_id,
                        relative_bytes, encoding, bytes.len() as i64],
            ).expect("indexed file");
            conn.execute(
                "INSERT INTO content_versions (
                    content_version_id, document_id, file_instance_id,
                    observed_at_ms, size_bytes, sha256,
                    verification_state, source_stable_during_read
                ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, 1)",
                params![
                    &content_version_id,
                    &document_id,
                    &file_instance_id,
                    bytes.len() as i64,
                    &digest[..],
                    verification_state
                ],
            )
            .expect("indexed content");
            let workspace_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO normal_data_workspaces (
                    normal_data_workspace_id, created_at_ms
                ) VALUES (?1, 1)",
                [&workspace_id],
            )
            .expect("workspace");
            conn.execute(
                "INSERT INTO normal_data_datasets (
                    normal_data_dataset_id, normal_data_workspace_id, name, created_at_ms
                ) VALUES (?1, ?2, 'Source', 1)",
                params![&dataset_id, &workspace_id],
            )
            .expect("dataset");
            conn.execute(
                "INSERT INTO normal_data_dataset_versions (
                    normal_data_dataset_version_id, normal_data_dataset_id,
                    version_number, document_id, file_instance_id, content_version_id,
                    source_observed_at_ms, source_size_bytes,
                    source_verification_state, source_stable_during_read,
                    source_sha256, created_at_ms
                ) VALUES (?1, ?2, 1, ?3, ?4, ?5, 1, ?6, ?7, 1, ?8, 1)",
                params![
                    &dataset_version_id,
                    &dataset_id,
                    &document_id,
                    &file_instance_id,
                    &content_version_id,
                    bytes.len() as i64,
                    verification_state,
                    &digest[..]
                ],
            )
            .expect("frozen dataset version");
            drop(conn);
            Self {
                folder,
                database_path,
                source_path,
                dataset_version_id,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.folder);
        }
    }

    #[test]
    fn exact_hash_verified_source_bytes_can_be_returned() {
        let fixture = Fixture::new("HASH_VERIFIED");
        let verified = read_verified_dataset(&fixture.database_path, &fixture.dataset_version_id)
            .expect("verified registered dataset");
        assert_eq!(verified.dataset_version_id, fixture.dataset_version_id);
        assert_eq!(
            verified.bytes,
            fs::read(&fixture.source_path).expect("source")
        );
        assert_eq!(&verified.sha256[..], &Sha256::digest(&verified.bytes)[..]);
    }

    #[test]
    fn modified_linked_source_cannot_impersonate_frozen_version() {
        let fixture = Fixture::new("HASH_VERIFIED");
        // Same byte count, different bytes: size-only validation is insufficient.
        fs::write(&fixture.source_path, b"key,period,amount\nB,2026-08,100\n")
            .expect("modify linked file");
        assert!(
            read_verified_dataset(&fixture.database_path, &fixture.dataset_version_id).is_err()
        );
    }

    #[test]
    fn fingerprinted_only_source_is_not_eligible_for_verified_execution() {
        let fixture = Fixture::new("FINGERPRINTED");
        assert!(
            read_verified_dataset(&fixture.database_path, &fixture.dataset_version_id).is_err()
        );
    }

    #[test]
    fn indexed_source_outside_approved_root_is_rejected() {
        let fixture = Fixture::new("HASH_VERIFIED");
        let conn = persistence::open_configured_connection(&fixture.database_path)
            .expect("open fixture database");
        let file_instance_id: String = conn
            .query_row(
                "SELECT file_instance_id FROM normal_data_dataset_versions
             WHERE normal_data_dataset_version_id = ?1",
                [&fixture.dataset_version_id],
                |row| row.get(0),
            )
            .expect("file instance");
        let (native, encoding) = encode_relative(Path::new("../invoices.csv"));
        conn.execute(
            "UPDATE file_instances SET relative_path_native = ?1,
             path_native_encoding = ?2 WHERE file_instance_id = ?3",
            params![native, encoding, file_instance_id],
        )
        .expect("simulate malicious stored path");
        assert!(
            read_verified_dataset(&fixture.database_path, &fixture.dataset_version_id).is_err()
        );
    }
}
