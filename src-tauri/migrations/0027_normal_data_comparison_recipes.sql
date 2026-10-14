-- Versioned Normal Data comparison definitions are working-data configuration,
-- not controlled evidence, and are separate from engagement/workpaper tables.
CREATE TABLE normal_data_comparison_recipes (
    normal_data_comparison_recipe_id TEXT PRIMARY KEY NOT NULL,
    normal_data_workspace_id TEXT NOT NULL REFERENCES normal_data_workspaces(normal_data_workspace_id),
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE normal_data_comparison_recipe_versions (
    normal_data_comparison_recipe_version_id TEXT PRIMARY KEY NOT NULL,
    normal_data_comparison_recipe_id TEXT NOT NULL
        REFERENCES normal_data_comparison_recipes(normal_data_comparison_recipe_id),
    version_number INTEGER NOT NULL CHECK (version_number >= 1),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 240),
    dataset_a_version_id TEXT NOT NULL
        REFERENCES normal_data_dataset_versions(normal_data_dataset_version_id),
    dataset_b_version_id TEXT NOT NULL
        REFERENCES normal_data_dataset_versions(normal_data_dataset_version_id),
    period_basis TEXT NOT NULL CHECK (
        period_basis IN ('FILING_PERIOD', 'INVOICE_MONTH', 'ACCOUNTING_PERIOD')
    ),
    period_column_a TEXT NOT NULL CHECK (length(trim(period_column_a)) BETWEEN 1 AND 240),
    period_column_b TEXT NOT NULL CHECK (length(trim(period_column_b)) BETWEEN 1 AND 240),
    amount_columns_json TEXT NOT NULL CHECK (
        length(amount_columns_json) BETWEEN 2 AND 8192 AND json_valid(amount_columns_json)
    ),
    tolerance_minor_units INTEGER NOT NULL CHECK (tolerance_minor_units >= 0),
    created_at_ms INTEGER NOT NULL,
    CHECK (dataset_a_version_id <> dataset_b_version_id),
    UNIQUE (normal_data_comparison_recipe_id, version_number)
);

CREATE INDEX idx_normal_data_comparison_recipes_workspace
    ON normal_data_comparison_recipes(normal_data_workspace_id, created_at_ms DESC);
CREATE INDEX idx_normal_data_comparison_recipe_versions_recipe
    ON normal_data_comparison_recipe_versions(normal_data_comparison_recipe_id, version_number DESC);

-- Execution adapter will populate runs only after native re-read and SHA-256
-- verification of BOTH registered source identities. No raw frontend-created
-- run API is provided by this migration.
CREATE TABLE normal_data_comparison_runs (
    normal_data_comparison_run_id TEXT PRIMARY KEY NOT NULL,
    normal_data_comparison_recipe_version_id TEXT NOT NULL
        REFERENCES normal_data_comparison_recipe_versions(normal_data_comparison_recipe_version_id),
    dataset_a_source_sha256 BLOB NOT NULL CHECK (length(dataset_a_source_sha256) = 32),
    dataset_b_source_sha256 BLOB NOT NULL CHECK (length(dataset_b_source_sha256) = 32),
    result_sha256 BLOB NOT NULL CHECK (length(result_sha256) = 32),
    result_json TEXT NOT NULL CHECK (length(result_json) BETWEEN 2 AND 8388608 AND json_valid(result_json)),
    started_at_ms INTEGER NOT NULL,
    completed_at_ms INTEGER NOT NULL CHECK (completed_at_ms >= started_at_ms)
);
CREATE INDEX idx_normal_data_comparison_runs_recipe
    ON normal_data_comparison_runs(normal_data_comparison_recipe_version_id, completed_at_ms DESC);

CREATE TRIGGER trg_normal_data_comparison_recipes_no_update
BEFORE UPDATE ON normal_data_comparison_recipes
BEGIN SELECT RAISE(ABORT, 'normal data recipe identity is immutable'); END;
CREATE TRIGGER trg_normal_data_comparison_recipes_no_delete
BEFORE DELETE ON normal_data_comparison_recipes
BEGIN SELECT RAISE(ABORT, 'normal data recipe identity is immutable'); END;
CREATE TRIGGER trg_normal_data_comparison_recipe_versions_no_update
BEFORE UPDATE ON normal_data_comparison_recipe_versions
BEGIN SELECT RAISE(ABORT, 'normal data recipe versions are immutable'); END;
CREATE TRIGGER trg_normal_data_comparison_recipe_versions_no_delete
BEFORE DELETE ON normal_data_comparison_recipe_versions
BEGIN SELECT RAISE(ABORT, 'normal data recipe versions are immutable'); END;
CREATE TRIGGER trg_normal_data_comparison_runs_no_update
BEFORE UPDATE ON normal_data_comparison_runs
BEGIN SELECT RAISE(ABORT, 'normal data comparison runs are immutable'); END;
CREATE TRIGGER trg_normal_data_comparison_runs_no_delete
BEFORE DELETE ON normal_data_comparison_runs
BEGIN SELECT RAISE(ABORT, 'normal data comparison runs are immutable'); END;
