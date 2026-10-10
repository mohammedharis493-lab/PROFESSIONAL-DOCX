-- Append-only local observations of private Normal Data retention candidates.
-- These are not authenticated attestations, controlled evidence, custody
-- guarantees, workpaper links, approvals, or specialist promotion authority.
-- Filesystem scanning and SQLite commit are NOT an atomic snapshot.
CREATE TABLE normal_data_retention_recovery_scans (
    recovery_scan_id TEXT PRIMARY KEY NOT NULL CHECK (length(recovery_scan_id) = 36),
    observed_at_ms INTEGER NOT NULL CHECK (observed_at_ms >= 0),
    entry_count INTEGER NOT NULL CHECK (entry_count BETWEEN 0 AND 10000)
);

CREATE TABLE normal_data_retention_recovery_entries (
    recovery_scan_id TEXT NOT NULL
        REFERENCES normal_data_retention_recovery_scans(recovery_scan_id),
    stage_id TEXT NOT NULL CHECK (length(stage_id) = 36),
    observed_state TEXT NOT NULL CHECK (observed_state IN (
        'RECORDED_VALID', 'ORPHAN_VALID', 'PARTIAL_INTERRUPTED',
        'CANDIDATE_CORRUPT', 'RECORDED_MISSING'
    )),
    PRIMARY KEY (recovery_scan_id, stage_id)
);

CREATE INDEX idx_normal_data_retention_recovery_entries_stage
    ON normal_data_retention_recovery_entries(stage_id, recovery_scan_id);

CREATE TRIGGER trg_normal_data_retention_recovery_scans_no_update
BEFORE UPDATE ON normal_data_retention_recovery_scans
BEGIN SELECT RAISE(ABORT, 'retention recovery scan is immutable'); END;
CREATE TRIGGER trg_normal_data_retention_recovery_scans_no_delete
BEFORE DELETE ON normal_data_retention_recovery_scans
BEGIN SELECT RAISE(ABORT, 'retention recovery scan cannot be deleted'); END;
CREATE TRIGGER trg_normal_data_retention_recovery_entries_no_update
BEFORE UPDATE ON normal_data_retention_recovery_entries
BEGIN SELECT RAISE(ABORT, 'retention recovery entry is immutable'); END;
CREATE TRIGGER trg_normal_data_retention_recovery_entries_no_delete
BEFORE DELETE ON normal_data_retention_recovery_entries
BEGIN SELECT RAISE(ABORT, 'retention recovery entry cannot be deleted'); END;
