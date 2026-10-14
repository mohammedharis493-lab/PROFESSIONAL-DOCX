-- Local, append-only recovery *observations* of private Normal Data staging.
-- These are NOT controlled evidence, capture approvals, trusted filesystem
-- attestations, retained artifacts, workpaper links, or promotion privileges.
-- No production authenticator, authority or Tauri command is introduced.
CREATE TABLE normal_data_stage_recovery_scans (
    recovery_scan_id TEXT PRIMARY KEY NOT NULL CHECK (length(recovery_scan_id) = 36),
    observed_at_ms INTEGER NOT NULL CHECK (observed_at_ms >= 0),
    entry_count INTEGER NOT NULL CHECK (entry_count BETWEEN 0 AND 10000)
);
CREATE TABLE normal_data_stage_recovery_entries (
    recovery_scan_id TEXT NOT NULL
      REFERENCES normal_data_stage_recovery_scans(recovery_scan_id),
    stage_id TEXT NOT NULL CHECK (length(stage_id) = 36),
    observed_state TEXT NOT NULL CHECK (observed_state IN (
        'READY_LOCAL_HASH_VALID', 'READY_CORRUPT', 'PARTIAL_INTERRUPTED'
    )),
    PRIMARY KEY (recovery_scan_id, stage_id)
);
CREATE INDEX idx_normal_data_stage_recovery_entries_stage
    ON normal_data_stage_recovery_entries(stage_id, recovery_scan_id);

CREATE TRIGGER trg_normal_data_stage_recovery_scans_no_update
BEFORE UPDATE ON normal_data_stage_recovery_scans
BEGIN SELECT RAISE(ABORT, 'recovery scan observation is immutable'); END;
CREATE TRIGGER trg_normal_data_stage_recovery_scans_no_delete
BEFORE DELETE ON normal_data_stage_recovery_scans
BEGIN SELECT RAISE(ABORT, 'recovery scan observation cannot be deleted'); END;
CREATE TRIGGER trg_normal_data_stage_recovery_entries_no_update
BEFORE UPDATE ON normal_data_stage_recovery_entries
BEGIN SELECT RAISE(ABORT, 'recovery entry observation is immutable'); END;
CREATE TRIGGER trg_normal_data_stage_recovery_entries_no_delete
BEFORE DELETE ON normal_data_stage_recovery_entries
BEGIN SELECT RAISE(ABORT, 'recovery entry observation cannot be deleted'); END;
