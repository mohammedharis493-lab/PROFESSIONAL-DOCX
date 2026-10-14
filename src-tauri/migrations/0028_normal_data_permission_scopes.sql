-- Internal-only, default-deny permission registry. No API or authenticated
-- grant issuer is installed by this migration. Local SQLite rows are policy
-- configuration, never an authenticated user/session by themselves.
CREATE TABLE normal_data_permission_subjects (
    subject_id TEXT PRIMARY KEY NOT NULL CHECK (length(subject_id) = 36),
    identity_issuer TEXT NOT NULL CHECK (identity_issuer = 'TRUSTED_NATIVE_IDP'),
    registered_at_ms INTEGER NOT NULL CHECK (registered_at_ms >= 0),
    disabled_at_ms INTEGER CHECK (
        disabled_at_ms IS NULL OR disabled_at_ms >= registered_at_ms
    )
);

CREATE TABLE normal_data_permission_grants (
    grant_id TEXT PRIMARY KEY NOT NULL CHECK (length(grant_id) = 36),
    subject_id TEXT NOT NULL REFERENCES normal_data_permission_subjects(subject_id),
    permission TEXT NOT NULL CHECK (
        permission IN (
            'READ_NORMAL_DATA_WORKSPACE',
            'ATTACH_EVIDENCE_TO_ENGAGEMENT',
            'MODIFY_WORKPAPER_REVISION'
        )
    ),
    resource_id TEXT NOT NULL CHECK (length(resource_id) = 36),
    granted_at_ms INTEGER NOT NULL CHECK (granted_at_ms >= 0),
    expires_at_ms INTEGER CHECK (
        expires_at_ms IS NULL OR expires_at_ms > granted_at_ms
    ),
    revoked_at_ms INTEGER CHECK (
        revoked_at_ms IS NULL OR revoked_at_ms >= granted_at_ms
    ),
    UNIQUE (subject_id, permission, resource_id, granted_at_ms)
);

CREATE UNIQUE INDEX idx_normal_data_permission_active_unique
    ON normal_data_permission_grants(subject_id, permission, resource_id)
    WHERE revoked_at_ms IS NULL;

CREATE INDEX idx_normal_data_permission_grants_subject
    ON normal_data_permission_grants(subject_id, permission, resource_id);

-- Enrollment identities are immutable, except for a one-way disable.
CREATE TRIGGER trg_normal_data_permission_subjects_immutable
BEFORE UPDATE ON normal_data_permission_subjects
WHEN NEW.subject_id IS NOT OLD.subject_id
    OR NEW.identity_issuer IS NOT OLD.identity_issuer
    OR NEW.registered_at_ms IS NOT OLD.registered_at_ms
    OR OLD.disabled_at_ms IS NOT NULL
    OR NEW.disabled_at_ms IS NULL
BEGIN SELECT RAISE(ABORT, 'permission subject may only be disabled once'); END;

CREATE TRIGGER trg_normal_data_permission_subjects_no_delete
BEFORE DELETE ON normal_data_permission_subjects
BEGIN SELECT RAISE(ABORT, 'permission subjects cannot be deleted'); END;

-- Grant scope/owner/expiry is immutable. Revocation is one-way and permanent.
CREATE TRIGGER trg_normal_data_permission_grants_immutable
BEFORE UPDATE ON normal_data_permission_grants
WHEN NEW.grant_id IS NOT OLD.grant_id
    OR NEW.subject_id IS NOT OLD.subject_id
    OR NEW.permission IS NOT OLD.permission
    OR NEW.resource_id IS NOT OLD.resource_id
    OR NEW.granted_at_ms IS NOT OLD.granted_at_ms
    OR NEW.expires_at_ms IS NOT OLD.expires_at_ms
    OR OLD.revoked_at_ms IS NOT NULL
    OR NEW.revoked_at_ms IS NULL
BEGIN SELECT RAISE(ABORT, 'permission grant may only be revoked once'); END;

CREATE TRIGGER trg_normal_data_permission_grants_no_delete
BEFORE DELETE ON normal_data_permission_grants
BEGIN SELECT RAISE(ABORT, 'permission grants cannot be deleted'); END;
