-- Durable local Normal Data retention *candidates* for crash recovery.
-- These rows and package bytes are NOT controlled evidence, authenticated
-- approvals, workpaper links, signoffs, or specialist promotion authority.
CREATE TABLE normal_data_retention_candidates (
    stage_id TEXT PRIMARY KEY NOT NULL CHECK (length(stage_id) = 36),
    normal_data_comparison_run_id TEXT NOT NULL
        REFERENCES normal_data_comparison_runs(normal_data_comparison_run_id),
    normal_data_workspace_id TEXT NOT NULL
        REFERENCES normal_data_workspaces(normal_data_workspace_id),
    normal_data_comparison_recipe_version_id TEXT NOT NULL
        REFERENCES normal_data_comparison_recipe_versions(normal_data_comparison_recipe_version_id),

    source_a_dataset_version_id TEXT NOT NULL
        REFERENCES normal_data_dataset_versions(normal_data_dataset_version_id),
    source_a_document_id TEXT NOT NULL REFERENCES documents(document_id),
    source_a_content_version_id TEXT NOT NULL REFERENCES content_versions(content_version_id),
    source_a_sha256_hex TEXT NOT NULL CHECK (
        length(source_a_sha256_hex) = 64
        AND source_a_sha256_hex NOT GLOB '*[^0-9a-f]*'
    ),
    source_a_size_bytes INTEGER NOT NULL CHECK (source_a_size_bytes > 0),

    source_b_dataset_version_id TEXT NOT NULL
        REFERENCES normal_data_dataset_versions(normal_data_dataset_version_id),
    source_b_document_id TEXT NOT NULL REFERENCES documents(document_id),
    source_b_content_version_id TEXT NOT NULL REFERENCES content_versions(content_version_id),
    source_b_sha256_hex TEXT NOT NULL CHECK (
        length(source_b_sha256_hex) = 64
        AND source_b_sha256_hex NOT GLOB '*[^0-9a-f]*'
    ),
    source_b_size_bytes INTEGER NOT NULL CHECK (source_b_size_bytes > 0),

    result_artifact_sha256_hex TEXT NOT NULL CHECK (
        length(result_artifact_sha256_hex) = 64
        AND result_artifact_sha256_hex NOT GLOB '*[^0-9a-f]*'
    ),
    result_semantic_sha256_hex TEXT NOT NULL CHECK (
        length(result_semantic_sha256_hex) = 64
        AND result_semantic_sha256_hex NOT GLOB '*[^0-9a-f]*'
    ),
    result_size_bytes INTEGER NOT NULL CHECK (result_size_bytes > 0),
    retained_at_ms INTEGER NOT NULL CHECK (retained_at_ms >= 0)
);

CREATE INDEX idx_normal_data_retention_candidates_run
    ON normal_data_retention_candidates(normal_data_comparison_run_id, retained_at_ms);

CREATE TRIGGER trg_normal_data_retention_candidates_no_update
BEFORE UPDATE ON normal_data_retention_candidates
BEGIN SELECT RAISE(ABORT, 'normal data retention candidate is immutable'); END;

CREATE TRIGGER trg_normal_data_retention_candidates_no_delete
BEFORE DELETE ON normal_data_retention_candidates
BEGIN SELECT RAISE(ABORT, 'normal data retention candidate cannot be deleted'); END;
