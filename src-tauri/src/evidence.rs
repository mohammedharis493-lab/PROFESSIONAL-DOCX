use crate::{
    filesystem,
    launcher,
    persistence::{self, ControlledEvidenceVersionRecord, EvidenceCaptureSourceRecord},
};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fmt, fs,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

const CAPTURE_REASON: &str = "MANUAL_CAPTURE";
const CAPTURE_POLICY: &str = "MANUAL_USER_REQUEST";
const COPY_BUFFER_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct EvidenceState {
    root: PathBuf,
}

impl EvidenceState {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

#[derive(Debug)]
pub enum EvidenceError {
    Io(std::io::Error),
    Persistence(persistence::PersistenceError),
    Launch(launcher::LaunchError),
    SourceChanged(String),
    Integrity(String),
    Configuration(String),
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Evidence I/O error: {error}"),
            Self::Persistence(error) => write!(f, "Evidence persistence error: {error}"),
            Self::Launch(error) => write!(f, "Evidence source error: {error}"),
            Self::SourceChanged(message) => write!(f, "Evidence source changed: {message}"),
            Self::Integrity(message) => write!(f, "Evidence integrity error: {message}"),
            Self::Configuration(message) => write!(f, "Evidence configuration error: {message}"),
        }
    }
}

impl Error for EvidenceError {}

impl From<std::io::Error> for EvidenceError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<persistence::PersistenceError> for EvidenceError {
    fn from(value: persistence::PersistenceError) -> Self {
        Self::Persistence(value)
    }
}

impl From<launcher::LaunchError> for EvidenceError {
    fn from(value: launcher::LaunchError) -> Self {
        Self::Launch(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceSnapshot {
    size_bytes: u64,
    creation_time_ms: Option<i64>,
    last_write_time_ms: Option<i64>,
    filesystem_identity: Option<Vec<u8>>,
    volume_identity: Option<Vec<u8>>,
}

pub fn capture_controlled_evidence(
    database_path: &Path,
    state: &EvidenceState,
    file_instance_id: &str,
) -> Result<ControlledEvidenceVersionRecord, EvidenceError> {
    capture_controlled_evidence_internal(database_path, state, file_instance_id, |_| {})
}

fn capture_controlled_evidence_internal<F>(
    database_path: &Path,
    state: &EvidenceState,
    file_instance_id: &str,
    after_copy: F,
) -> Result<ControlledEvidenceVersionRecord, EvidenceError>
where
    F: FnOnce(&Path),
{
    let capture_job_id = Uuid::new_v4().to_string();
    let controlled_evidence_version_id = Uuid::new_v4().to_string();

    let source = persistence::begin_evidence_capture(
        database_path,
        &capture_job_id,
        file_instance_id,
        CAPTURE_REASON,
        CAPTURE_POLICY,
    )?;

    let result = perform_capture(
        database_path,
        state,
        &capture_job_id,
        &controlled_evidence_version_id,
        &source,
        after_copy,
    );

    if let Err(error) = &result {
        let (status, code) = match error {
            EvidenceError::Integrity(_) => ("QUARANTINED", "CAPTURE_INTEGRITY_FAILED"),
            EvidenceError::SourceChanged(_) => ("FAILED", "SOURCE_CHANGED_DURING_CAPTURE"),
            EvidenceError::Launch(_) => ("FAILED", "SOURCE_UNAVAILABLE"),
            EvidenceError::Io(_) => ("FAILED", "CAPTURE_IO_FAILED"),
            EvidenceError::Persistence(_) => ("FAILED", "CAPTURE_PERSISTENCE_FAILED"),
            EvidenceError::Configuration(_) => ("FAILED", "CAPTURE_CONFIGURATION_FAILED"),
        };

        let _ = persistence::finish_evidence_capture_failure(
            database_path,
            &capture_job_id,
            status,
            code,
            &error.to_string(),
        );
    }

    result
}

fn perform_capture<F>(
    database_path: &Path,
    state: &EvidenceState,
    capture_job_id: &str,
    controlled_evidence_version_id: &str,
    source: &EvidenceCaptureSourceRecord,
    after_copy: F,
) -> Result<ControlledEvidenceVersionRecord, EvidenceError>
where
    F: FnOnce(&Path),
{
    Uuid::parse_str(&source.document_id).map_err(|_| {
        EvidenceError::Configuration("document identifier is not a UUID".to_string())
    })?;
    Uuid::parse_str(controlled_evidence_version_id).map_err(|_| {
        EvidenceError::Configuration("controlled evidence identifier is not a UUID".to_string())
    })?;

    let source_path = launcher::validated_existing_path(&source.source)?;
    let mut source_file = File::open(&source_path)?;
    let before = snapshot_open_file(&source_path, &source_file)?;

    verify_indexed_source(source, &before)?;

    let document_dir = state.root.join(&source.document_id);
    fs::create_dir_all(&document_dir)?;

    let temp_path = document_dir.join(format!(".{controlled_evidence_version_id}.capturing"));
    let final_path = document_dir.join(controlled_evidence_version_id);
    if final_path.exists() {
        return Err(EvidenceError::Configuration(
            "controlled evidence target already exists".to_string(),
        ));
    }

    let copy_result = (|| -> Result<([u8; 32], u64), EvidenceError> {
        let mut target = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
        let mut copied = 0_u64;

        loop {
            let read = source_file.read(&mut buffer)?;
            if read == 0 {
                break;
            }

            target.write_all(&buffer[..read])?;
            hasher.update(&buffer[..read]);
            copied = copied.checked_add(read as u64).ok_or_else(|| {
                EvidenceError::Configuration("captured byte count overflowed".to_string())
            })?;
        }

        target.flush()?;
        target.sync_all()?;

        let digest = hasher.finalize();
        let mut sha256 = [0_u8; 32];
        sha256.copy_from_slice(&digest);
        Ok((sha256, copied))
    })();

    let (sha256, copied) = match copy_result {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
    };

    after_copy(&source_path);

    let after_handle = snapshot_open_file(&source_path, &source_file)?;
    let after_path = snapshot_path(&source_path)?;

    if copied != before.size_bytes || before != after_handle || before != after_path {
        let _ = fs::remove_file(&temp_path);
        return Err(EvidenceError::SourceChanged(
            "size, timestamps, or filesystem identity changed while bytes were being preserved"
                .to_string(),
        ));
    }

    fs::rename(&temp_path, &final_path)?;

    let mut permissions = fs::metadata(&final_path)?.permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&final_path, permissions)?;

    let verified_sha256 = hash_file(&final_path)?;
    if verified_sha256 != sha256 {
        return Err(EvidenceError::Integrity(
            "preserved bytes do not verify against the capture hash".to_string(),
        ));
    }

    let locator = format!(
        "{}/{}",
        source.document_id, controlled_evidence_version_id
    );

    persistence::complete_evidence_capture(
        database_path,
        persistence::EvidenceCaptureCompletion {
            capture_job_id,
            controlled_evidence_version_id,
            document_id: &source.document_id,
            file_instance_id: &source.file_instance_id,
            controlled_storage_locator: &locator,
            sha256: &sha256,
            size_bytes: copied,
            last_write_time_ms: before.last_write_time_ms,
        },
    )
    .map_err(EvidenceError::from)
}

fn verify_indexed_source(
    source: &EvidenceCaptureSourceRecord,
    actual: &SourceSnapshot,
) -> Result<(), EvidenceError> {
    let matches = actual.size_bytes == source.size_bytes
        && expected_optional_matches(source.creation_time_ms, actual.creation_time_ms)
        && expected_optional_matches(source.last_write_time_ms, actual.last_write_time_ms)
        && expected_optional_matches_ref(
            source.filesystem_identity.as_ref(),
            actual.filesystem_identity.as_ref(),
        )
        && expected_optional_matches_ref(
            source.volume_identity.as_ref(),
            actual.volume_identity.as_ref(),
        );

    if matches {
        Ok(())
    } else {
        Err(EvidenceError::SourceChanged(
            "source no longer matches the last indexed file identity; reconcile before capture"
                .to_string(),
        ))
    }
}

fn expected_optional_matches<T: PartialEq>(expected: Option<T>, actual: Option<T>) -> bool {
    match expected {
        Some(expected) => actual == Some(expected),
        None => true,
    }
}

fn expected_optional_matches_ref<T: PartialEq>(
    expected: Option<&T>,
    actual: Option<&T>,
) -> bool {
    match expected {
        Some(expected) => actual == Some(expected),
        None => true,
    }
}

fn snapshot_open_file(path: &Path, file: &File) -> Result<SourceSnapshot, EvidenceError> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(EvidenceError::SourceChanged(
            "source is no longer a regular file".to_string(),
        ));
    }

    let platform = filesystem::platform_file_metadata_from_open_file(file, &metadata);
    Ok(SourceSnapshot {
        size_bytes: metadata.len(),
        creation_time_ms: metadata.created().ok().and_then(system_time_to_ms),
        last_write_time_ms: metadata.modified().ok().and_then(system_time_to_ms),
        filesystem_identity: platform.filesystem_identity,
        volume_identity: platform.volume_identity,
    })
}

fn snapshot_path(path: &Path) -> Result<SourceSnapshot, EvidenceError> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(EvidenceError::SourceChanged(
            "source path no longer resolves to a regular file".to_string(),
        ));
    }

    let platform = filesystem::platform_file_metadata(path, &metadata);
    Ok(SourceSnapshot {
        size_bytes: metadata.len(),
        creation_time_ms: metadata.created().ok().and_then(system_time_to_ms),
        last_write_time_ms: metadata.modified().ok().and_then(system_time_to_ms),
        filesystem_identity: platform.filesystem_identity,
        volume_identity: platform.volume_identity,
    })
}

fn hash_file(path: &Path) -> Result<[u8; 32], EvidenceError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let digest = hasher.finalize();
    let mut sha256 = [0_u8; 32];
    sha256.copy_from_slice(&digest);
    Ok(sha256)
}

fn system_time_to_ms(value: SystemTime) -> Option<i64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer;
    use std::sync::{
        atomic::AtomicBool,
        Arc,
    };

    struct TestEvidence {
        directory: PathBuf,
        database_path: PathBuf,
        source_root: PathBuf,
        evidence_state: EvidenceState,
    }

    impl TestEvidence {
        fn new() -> Self {
            let directory =
                std::env::temp_dir().join(format!("professional-docx-evidence-{}", Uuid::new_v4()));
            let database_path = directory.join("data").join("metadata.sqlite");
            let source_root = directory.join("source");
            let evidence_state = EvidenceState::new(directory.join("data").join("controlled-evidence"));

            fs::create_dir_all(&source_root).expect("source root should be created");
            persistence::initialize_database(&database_path)
                .expect("test database should initialize");

            Self {
                directory,
                database_path,
                source_root,
                evidence_state,
            }
        }

        fn index_file(&self, name: &str, bytes: &[u8]) -> String {
            fs::write(self.source_root.join(name), bytes).expect("source file should be written");
            let canonical =
                fs::canonicalize(&self.source_root).expect("source root should canonicalize");
            let root = persistence::register_storage_root(
                &self.database_path,
                "root-evidence",
                &self.source_root,
                &canonical,
            )
            .expect("storage root should register");

            let job_id = Uuid::new_v4().to_string();
            let generation_id = Uuid::new_v4().to_string();
            persistence::create_index_job(
                &self.database_path,
                &root.storage_root_id,
                &job_id,
                &generation_id,
            )
            .expect("index job should be created");

            indexer::run_index_job(
                self.database_path.clone(),
                root.clone(),
                job_id,
                generation_id,
                Arc::new(AtomicBool::new(false)),
            )
            .expect("indexing should succeed");

            persistence::list_indexed_file_preview(
                &self.database_path,
                &root.storage_root_id,
                10,
            )
            .expect("indexed files should list")
            .into_iter()
            .find(|file| file.name == name)
            .expect("indexed file should exist")
            .file_instance_id
        }
    }

    impl Drop for TestEvidence {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn captured_evidence_remains_unchanged_after_source_edit() {
        let test = TestEvidence::new();
        let file_instance_id = test.index_file("Invoice 1001.txt", b"original evidence");

        let captured = capture_controlled_evidence(
            &test.database_path,
            &test.evidence_state,
            &file_instance_id,
        )
        .expect("capture should succeed");

        let stored = test
            .evidence_state
            .root
            .join(&captured.controlled_storage_locator);
        assert_eq!(
            fs::read(&stored).expect("controlled evidence should be readable"),
            b"original evidence"
        );
        assert_eq!(
            hash_file(&stored).expect("controlled evidence hash should verify").to_vec(),
            captured.sha256
        );

        fs::write(test.source_root.join("Invoice 1001.txt"), b"changed later")
            .expect("source should change after capture");

        assert_eq!(
            fs::read(&stored).expect("controlled evidence should remain readable"),
            b"original evidence"
        );
        assert_eq!(captured.verification_state, "HASH_VERIFIED");
        assert_eq!(captured.version_number, 1);
    }

    #[test]
    fn source_change_during_capture_prevents_completion() {
        let test = TestEvidence::new();
        let file_instance_id = test.index_file("Bank Confirmation.txt", b"stable source");
        let database_path = test.database_path.clone();
        let evidence_state = test.evidence_state.clone();
        let source_path = test.source_root.join("Bank Confirmation.txt");

        let error = capture_controlled_evidence_internal(
            &database_path,
            &evidence_state,
            &file_instance_id,
            |_| {
                fs::write(&source_path, b"source changed during capture")
                    .expect("test should mutate source");
            },
        )
        .expect_err("changing source during capture must fail");

        assert!(matches!(error, EvidenceError::SourceChanged(_)));
        assert_eq!(
            persistence::count_controlled_evidence_versions_for_test(&database_path)
                .expect("controlled evidence count should load"),
            0
        );
        assert_eq!(
            persistence::latest_evidence_capture_status_for_test(
                &database_path,
                &file_instance_id,
            )
            .expect("capture status should load")
            .as_deref(),
            Some("FAILED")
        );
    }
}
