CREATE TABLE firms (
    firm_id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 200),
    created_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER
);

CREATE UNIQUE INDEX idx_firms_active_name
    ON firms(name COLLATE NOCASE)
    WHERE archived_at_ms IS NULL;

CREATE TABLE users (
    user_id TEXT PRIMARY KEY NOT NULL,
    firm_id TEXT NOT NULL REFERENCES firms(firm_id),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 200),
    status TEXT NOT NULL CHECK (length(trim(status)) BETWEEN 1 AND 80),
    created_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER
);

CREATE INDEX idx_users_firm_active
    ON users(firm_id, display_name COLLATE NOCASE)
    WHERE archived_at_ms IS NULL;

CREATE TABLE clients (
    client_id TEXT PRIMARY KEY NOT NULL,
    firm_id TEXT NOT NULL REFERENCES firms(firm_id),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 240),
    reference TEXT,
    created_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER
);

CREATE UNIQUE INDEX idx_clients_active_name
    ON clients(firm_id, name COLLATE NOCASE)
    WHERE archived_at_ms IS NULL;

CREATE TABLE service_types (
    service_type_id TEXT PRIMARY KEY NOT NULL,
    firm_id TEXT NOT NULL REFERENCES firms(firm_id),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 160),
    description TEXT,
    created_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER
);

CREATE UNIQUE INDEX idx_service_types_active_name
    ON service_types(firm_id, name COLLATE NOCASE)
    WHERE archived_at_ms IS NULL;

CREATE TABLE engagements (
    engagement_id TEXT PRIMARY KEY NOT NULL,
    firm_id TEXT NOT NULL REFERENCES firms(firm_id),
    client_id TEXT NOT NULL REFERENCES clients(client_id),
    service_type_id TEXT NOT NULL REFERENCES service_types(service_type_id),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 240),
    period_start TEXT,
    period_end TEXT,
    status TEXT NOT NULL CHECK (length(trim(status)) BETWEEN 1 AND 80),
    created_at_ms INTEGER NOT NULL,
    created_by TEXT REFERENCES users(user_id),
    archived_at_ms INTEGER,
    CHECK (period_start IS NULL OR length(period_start) = 10),
    CHECK (period_end IS NULL OR length(period_end) = 10),
    CHECK (period_start IS NULL OR period_end IS NULL OR period_start <= period_end)
);

CREATE INDEX idx_engagements_client_active
    ON engagements(client_id, created_at_ms DESC)
    WHERE archived_at_ms IS NULL;

CREATE INDEX idx_engagements_firm_status
    ON engagements(firm_id, status, created_at_ms DESC)
    WHERE archived_at_ms IS NULL;

CREATE TABLE engagement_areas (
    engagement_area_id TEXT PRIMARY KEY NOT NULL,
    engagement_id TEXT NOT NULL REFERENCES engagements(engagement_id),
    parent_engagement_area_id TEXT REFERENCES engagement_areas(engagement_area_id),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 200),
    code TEXT,
    display_order INTEGER NOT NULL DEFAULT 0 CHECK (display_order >= 0),
    status TEXT NOT NULL CHECK (length(trim(status)) BETWEEN 1 AND 80),
    owner_user_id TEXT REFERENCES users(user_id),
    created_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER,
    CHECK (parent_engagement_area_id IS NULL OR parent_engagement_area_id <> engagement_area_id)
);

CREATE UNIQUE INDEX idx_engagement_areas_active_sibling_name
    ON engagement_areas(
        engagement_id,
        COALESCE(parent_engagement_area_id, ''),
        name COLLATE NOCASE
    )
    WHERE archived_at_ms IS NULL;

CREATE INDEX idx_engagement_areas_parent
    ON engagement_areas(engagement_id, parent_engagement_area_id, display_order, name)
    WHERE archived_at_ms IS NULL;
