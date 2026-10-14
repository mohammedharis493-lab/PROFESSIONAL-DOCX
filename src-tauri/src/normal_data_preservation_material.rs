//! Native-only, bounded working-data materialization for a future controlled
//! preservation workflow. Nothing is written, captured, linked, or authorized.
//! Raw input bytes never enter a Tauri command, event, audit log, or DTO.
use crate::{
    normal_data_comparison::{self, ComparisonResult},
    normal_data_provenance::{self, RunProvenanceReceipt, RunSourceReceipt},
    normal_data_source_reader,
    persistence::{self, PersistenceError},
};
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
use std::{fmt::Write, path::Path};

const MAX_RESULT_JSON_BYTES: usize = 8 * 1024 * 1024;

/// Exact bytes returned by one approved-root, size-bounded source read.
/// This value is transient, not a retained controlled-evidence version.
pub(crate) struct SourceMaterial {
    pub dataset_version_id: String,
    pub document_id: String,
    pub content_version_id: String,
    pub sha256_hex: String,
    pub bytes: Vec<u8>,
}

/// Exact persisted JSON bytes, with two deliberately different SHA-256
/// meanings: semantic calculation digest and serialized-artifact byte digest.
pub(crate) struct ResultMaterial {
    pub semantic_result_sha256_hex: String,
    pub artifact_sha256_hex: String,
    pub bytes: Vec<u8>,
}

/// Three self-contained *working-data* artifacts. Neither an atomic two-file
/// snapshot nor proof of durable retention, identity, authorization or signoff.
pub(crate) struct PreservationMaterial {
    pub run_id: String,
    pub workspace_id: String,
    pub recipe_version_id: String,
    pub source_a: SourceMaterial,
    pub source_b: SourceMaterial,
    pub result: ResultMaterial,
}

fn invalid() -> PersistenceError {
    PersistenceError::Configuration(
        "comparison material does not match the verified historical run".to_string(),
    )
}

fn digest_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        write!(&mut hex, "{byte:02x}").expect("hex encoding cannot fail");
    }
    hex
}

fn materialize_source(
    database_path: &Path,
    frozen: &RunSourceReceipt,
) -> Result<SourceMaterial, PersistenceError> {
    let verified = normal_data_source_reader::read_verified_dataset(
        database_path,
        &frozen.dataset_version_id,
    )?;
    if verified.dataset_version_id != frozen.dataset_version_id
        || verified.content_version_id != frozen.content_version_id
        || digest_hex(&verified.bytes) != frozen.sha256_hex
        || Sha256::digest(&verified.bytes)[..] != verified.sha256[..]
    {
        return Err(invalid());
    }
    Ok(SourceMaterial {
        dataset_version_id: verified.dataset_version_id,
        document_id: verified.document_id,
        content_version_id: verified.content_version_id,
        sha256_hex: frozen.sha256_hex.clone(),
        bytes: verified.bytes,
    })
}

fn materialize_result(
    database_path: &Path,
    receipt: &RunProvenanceReceipt,
) -> Result<ResultMaterial, PersistenceError> {
    let connection = persistence::open_configured_connection(database_path)?;
    let frozen: Option<(String, Vec<u8>)> = connection
        .query_row(
            "SELECT result_json, result_sha256
             FROM normal_data_comparison_runs
             WHERE normal_data_comparison_run_id = ?1
               AND normal_data_comparison_recipe_version_id = ?2",
            [
                &receipt.normal_data_comparison_run_id,
                &receipt.normal_data_comparison_recipe_version_id,
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (json, stored_digest) = frozen.ok_or_else(invalid)?;
    if json.is_empty() || json.len() > MAX_RESULT_JSON_BYTES || stored_digest.len() != 32 {
        return Err(invalid());
    }
    let result: ComparisonResult = serde_json::from_str(&json).map_err(|_| invalid())?;
    normal_data_comparison::verify_stored_result_digest(&result).map_err(|_| invalid())?;
    let mut semantic_hex = String::with_capacity(64);
    for byte in stored_digest {
        write!(&mut semantic_hex, "{byte:02x}").expect("hex encoding cannot fail");
    }
    if semantic_hex != receipt.result_sha256_hex || result.result_sha256_hex != semantic_hex {
        return Err(invalid());
    }
    let bytes = json.into_bytes();
    let artifact_sha256_hex = digest_hex(&bytes);
    Ok(ResultMaterial {
        semantic_result_sha256_hex: semantic_hex,
        artifact_sha256_hex,
        bytes,
    })
}

/// Builds transient bounded material for a future privileged retention adapter.
/// Provenance is checked BEFORE reading, but linked originals are read in
/// sequence. Their contents can change again after this function returns.
/// No capture is allowed to use earlier hash observations as substitute bytes.
///
/// Sources: each <=32 MiB, independently verified from a frozen approved-root
/// content version. Result: <=8 MiB exact stored JSON, checked against the
/// frozen recipe and semantic result digest. Returned byte hashes can serve as
/// expected digests for an as-yet-unimplemented atomic retention workflow.
pub(crate) fn prepare_preservation_material(
    database_path: &Path,
    run_id: &str,
) -> Result<PreservationMaterial, PersistenceError> {
    let receipt = normal_data_provenance::inspect_run(database_path, run_id)?;
    let source_a = materialize_source(database_path, &receipt.source_a)?;
    let source_b = materialize_source(database_path, &receipt.source_b)?;
    let result = materialize_result(database_path, &receipt)?;
    Ok(PreservationMaterial {
        run_id: receipt.normal_data_comparison_run_id,
        workspace_id: receipt.normal_data_workspace_id,
        recipe_version_id: receipt.normal_data_comparison_recipe_version_id,
        source_a,
        source_b,
        result,
    })
}
