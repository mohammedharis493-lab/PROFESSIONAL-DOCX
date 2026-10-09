-- Normal data is a platform capability, not an audit engagement type.
-- Workspace identity and its metadata history are append-only.
CREATE TABLE normal_data_workspaces (
    normal_data_workspace_id TEXT PRIMARY KEY NOT NULL,
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE normal_data_workspace_versions (
    normal_data_workspace_version_id TEXT PRIMARY KEY NOT NULL,
    normal_data_workspace_id TEXT NOT NULL
        REFERENCES normal_data_workspaces(normal_data_workspace_id),
    version_number INTEGER NOT NULL CHECK (version_number >= 1),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 240),
    description TEXT CHECK (description IS NULL OR length(description) <= 8192),
    client_id TEXT REFERENCES clients(client_id),
    period_start TEXT,
    period_end TEXT,
    created_at_ms INTEGER NOT NULL,
    CHECK ((period_start IS NULL) = (period_end IS NULL)),
    CHECK (period_start IS NULL OR (
        length(period_start) = 10 AND length(period_end) = 10
        AND period_start <= period_end
    )),
    UNIQUE (normal_data_workspace_id, version_number)
);

CREATE INDEX idx_normal_data_workspace_versions_latest
    ON normal_data_workspace_versions(normal_data_workspace_id, version_number DESC);

CREATE TRIGGER trg_normal_data_workspaces_no_update
BEFORE UPDATE ON normal_data_workspaces
BEGIN
    SELECT RAISE(ABORT, 'normal data workspace identities are immutable');
END;

CREATE TRIGGER trg_normal_data_workspaces_no_delete
BEFORE DELETE ON normal_data_workspaces
BEGIN
    SELECT RAISE(ABORT, 'normal data workspace identities are immutable');
END;

CREATE TRIGGER trg_normal_data_workspace_versions_no_update
BEFORE UPDATE ON normal_data_workspace_versions
BEGIN
    SELECT RAISE(ABORT, 'normal data workspace versions are immutable');
END;

CREATE TRIGGER trg_normal_data_workspace_versions_no_delete
BEFORE DELETE ON normal_data_workspace_versions
BEGIN
    SELECT RAISE(ABORT, 'normal data workspace versions are immutable');
END;
