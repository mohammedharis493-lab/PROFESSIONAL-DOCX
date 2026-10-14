-- Dedicated, append-only administrator-action receipt. The application has
-- no production authenticator or privilege to call admin functions yet.
-- This is NOT protection against a hostile local database owner.
CREATE TABLE normal_data_permission_admin_events (
    event_id TEXT PRIMARY KEY NOT NULL CHECK (length(event_id) = 36),
    event_type TEXT NOT NULL CHECK (
        event_type IN ('SUBJECT_ENROLLED', 'GRANT_ISSUED', 'GRANT_REVOKED', 'SUBJECT_DISABLED')
    ),
    administrator_subject_id TEXT NOT NULL CHECK (length(administrator_subject_id) = 36),
    subject_id TEXT NOT NULL REFERENCES normal_data_permission_subjects(subject_id),
    grant_id TEXT REFERENCES normal_data_permission_grants(grant_id),
    occurred_at_ms INTEGER NOT NULL CHECK (occurred_at_ms >= 0),
    CHECK (
        (event_type IN ('SUBJECT_ENROLLED', 'SUBJECT_DISABLED') AND grant_id IS NULL)
        OR (event_type IN ('GRANT_ISSUED', 'GRANT_REVOKED') AND grant_id IS NOT NULL)
    )
);
CREATE INDEX idx_normal_data_permission_admin_events_subject
    ON normal_data_permission_admin_events(subject_id, occurred_at_ms);

CREATE TRIGGER trg_normal_data_permission_admin_events_no_update
BEFORE UPDATE ON normal_data_permission_admin_events
BEGIN SELECT RAISE(ABORT, 'permission administrator history is immutable'); END;

CREATE TRIGGER trg_normal_data_permission_admin_events_no_delete
BEFORE DELETE ON normal_data_permission_admin_events
BEGIN SELECT RAISE(ABORT, 'permission administrator history cannot be deleted'); END;
