-- Normal Data datasets are ordinary working inputs, not controlled evidence.
-- An immutable source binding freezes the identity and verification state of one observed content version.
CREATE TABLE normal_data_datasets (
    normal_data_dataset_id TEXT PRIMARY KEY NOT NULL,
    normal_data_workspace_id TEXT NOT NULL
        REFERENCES normal_data_workspaces(normal_data_workspace_id),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 240),
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_normal_data_datasets_workspace
    ON normal_data_datasets(normal_data_workspace_id, created_at_ms DESC);

CREATE TABLE normal_data_dataset_versions (
    normal_data_dataset_version_id TEXT PRIMARY KEY NOT NULL,
    normal_data_dataset_id TEXT NOT NULL
        REFERENCES normal_data_datasets(normal_data_dataset_id),
    version_number INTEGER NOT NULL CHECK (version_number >= 1),
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    file_instance_id TEXT NOT NULL REFERENCES file_instances(file_instance_id),
    content_version_id TEXT NOT NULL REFERENCES content_versions(content_version_id),
    source_observed_at_ms INTEGER NOT NULL,
    source_size_bytes INTEGER NOT NULL CHECK (source_size_bytes >= 0),
    source_verification_state TEXT NOT NULL CHECK (
        source_verification_state IN (
            'UNVERIFIED', 'METADATA_ONLY', 'FINGERPRINTED', 'HASH_VERIFIED', 'FAILED'
        )
    ),
    source_stable_during_read INTEGER CHECK (
        source_stable_during_read IS NULL OR source_stable_during_read IN (0, 1)
    ),
    source_quick_fingerprint BLOB,
    source_sha256 BLOB CHECK (source_sha256 IS NULL OR length(source_sha256) = 32),
    created_at_ms INTEGER NOT NULL,
    CHECK (source_verification_state != 'HASH_VERIFIED' OR source_sha256 IS NOT NULL),
    UNIQUE(normal_data_dataset_id, version_number)
);
CREATE INDEX idx_normal_data_dataset_versions_dataset
    ON normal_data_dataset_versions(normal_data_dataset_id, version_number DESC);

-- User-declared semantic roles; NOT proof of parsing or validating source headers.
CREATE TABLE normal_data_column_semantics (
    normal_data_column_semantic_id TEXT PRIMARY KEY NOT NULL,
    normal_data_dataset_version_id TEXT NOT NULL
        REFERENCES normal_data_dataset_versions(normal_data_dataset_version_id),
    column_name TEXT NOT NULL COLLATE NOCASE
        CHECK (length(trim(column_name)) BETWEEN 1 AND 240),
    semantic_role TEXT NOT NULL CHECK (
        semantic_role IN (
            'BUSINESS_KEY', 'FILING_PERIOD', 'INVOICE_DATE', 'POSTING_DATE',
            'ACCOUNTING_PERIOD', 'TRANSACTION_DATE', 'NUMERIC_VALUE', 'OTHER'
        )
    ),
    data_type TEXT NOT NULL CHECK (data_type IN ('TEXT', 'DATE', 'PERIOD', 'DECIMAL')),
    created_at_ms INTEGER NOT NULL,
    UNIQUE(normal_data_dataset_version_id, column_name)
);
CREATE INDEX idx_normal_data_column_semantics_version
    ON normal_data_column_semantics(normal_data_dataset_version_id, created_at_ms);

CREATE TRIGGER trg_normal_data_datasets_no_update BEFORE UPDATE ON normal_data_datasets
BEGIN SELECT RAISE(ABORT, 'normal data datasets are immutable'); END;
CREATE TRIGGER trg_normal_data_datasets_no_delete BEFORE DELETE ON normal_data_datasets
BEGIN SELECT RAISE(ABORT, 'normal data datasets are immutable'); END;
CREATE TRIGGER trg_normal_data_dataset_versions_no_update BEFORE UPDATE ON normal_data_dataset_versions
BEGIN SELECT RAISE(ABORT, 'normal data dataset versions are immutable'); END;
CREATE TRIGGER trg_normal_data_dataset_versions_no_delete BEFORE DELETE ON normal_data_dataset_versions
BEGIN SELECT RAISE(ABORT, 'normal data dataset versions are immutable'); END;
CREATE TRIGGER trg_normal_data_column_semantics_no_update BEFORE UPDATE ON normal_data_column_semantics
BEGIN SELECT RAISE(ABORT, 'normal data column semantics are immutable'); END;
CREATE TRIGGER trg_normal_data_column_semantics_no_delete BEFORE DELETE ON normal_data_column_semantics
BEGIN SELECT RAISE(ABORT, 'normal data column semantics are immutable'); END;
