-- v33 binds new recovery observations to the canonical private root scanned.
-- A v32 historical scan lacks a root binding and must NOT be compared to
-- arbitrary current roots; legacy scan/entries remain immutable and readable.
-- Hash-only root binding is NOT authenticated custody or filesystem ownership.
CREATE TABLE normal_data_retention_recovery_root_scopes (
    recovery_scan_id TEXT PRIMARY KEY NOT NULL
        REFERENCES normal_data_retention_recovery_scans(recovery_scan_id),
    canonical_root_sha256_hex TEXT NOT NULL
        CHECK (length(canonical_root_sha256_hex) = 64
               AND canonical_root_sha256_hex NOT GLOB '*[^0-9a-f]*')
);

CREATE TRIGGER trg_normal_data_retention_recovery_root_scopes_no_update
BEFORE UPDATE ON normal_data_retention_recovery_root_scopes
BEGIN SELECT RAISE(ABORT, 'candidate recovery root scope is immutable'); END;
CREATE TRIGGER trg_normal_data_retention_recovery_root_scopes_no_delete
BEFORE DELETE ON normal_data_retention_recovery_root_scopes
BEGIN SELECT RAISE(ABORT, 'candidate recovery root scope cannot be deleted'); END;
