CREATE TABLE clients (
    client_id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    created_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER
);

CREATE TABLE service_types (
    service_type_id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    normalized_name TEXT NOT NULL UNIQUE CHECK (length(trim(normalized_name)) > 0),
    created_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER
);

CREATE TABLE engagements (
    engagement_id TEXT PRIMARY KEY NOT NULL,
    client_id TEXT NOT NULL REFERENCES clients(client_id),
    service_type_id TEXT NOT NULL REFERENCES service_types(service_type_id),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    period_start TEXT,
    period_end TEXT,
    status TEXT NOT NULL CHECK (length(trim(status)) > 0),
    created_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER
);

CREATE TABLE engagement_areas (
    engagement_area_id TEXT PRIMARY KEY NOT NULL,
    engagement_id TEXT NOT NULL REFERENCES engagements(engagement_id),
    parent_area_id TEXT REFERENCES engagement_areas(engagement_area_id),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    code TEXT,
    display_order INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL CHECK (length(trim(status)) > 0),
    created_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER,
    CHECK (parent_area_id IS NULL OR parent_area_id <> engagement_area_id)
);

CREATE UNIQUE INDEX idx_engagement_area_unique_active_sibling
    ON engagement_areas(
        engagement_id,
        COALESCE(parent_area_id, ''),
        lower(trim(name))
    )
    WHERE archived_at_ms IS NULL;

CREATE INDEX idx_engagement_areas_parent_order
    ON engagement_areas(engagement_id, parent_area_id, display_order, name);

CREATE TABLE procedures (
    procedure_id TEXT PRIMARY KEY NOT NULL,
    engagement_id TEXT NOT NULL REFERENCES engagements(engagement_id),
    engagement_area_id TEXT REFERENCES engagement_areas(engagement_area_id),
    reference TEXT,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    description TEXT,
    status TEXT NOT NULL CHECK (length(trim(status)) > 0),
    created_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER
);

CREATE INDEX idx_procedures_engagement_area
    ON procedures(engagement_id, engagement_area_id, created_at_ms);

CREATE TABLE workpapers (
    workpaper_id TEXT PRIMARY KEY NOT NULL,
    engagement_id TEXT NOT NULL REFERENCES engagements(engagement_id),
    engagement_area_id TEXT REFERENCES engagement_areas(engagement_area_id),
    procedure_id TEXT REFERENCES procedures(procedure_id),
    reference TEXT NOT NULL CHECK (length(trim(reference)) > 0),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    workflow_state TEXT NOT NULL CHECK (length(trim(workflow_state)) > 0),
    created_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER
);

CREATE UNIQUE INDEX idx_workpapers_active_reference
    ON workpapers(engagement_id, lower(trim(reference)))
    WHERE archived_at_ms IS NULL;

CREATE INDEX idx_workpapers_engagement_state
    ON workpapers(engagement_id, workflow_state, created_at_ms);

CREATE TABLE workpaper_revisions (
    workpaper_revision_id TEXT PRIMARY KEY NOT NULL,
    workpaper_id TEXT NOT NULL REFERENCES workpapers(workpaper_id),
    revision_number INTEGER NOT NULL CHECK (revision_number >= 1),
    created_at_ms INTEGER NOT NULL,
    revision_reason TEXT,
    supersedes_revision_id TEXT REFERENCES workpaper_revisions(workpaper_revision_id),
    objective TEXT NOT NULL DEFAULT '',
    procedure_performed TEXT NOT NULL DEFAULT '',
    population TEXT NOT NULL DEFAULT '',
    sample TEXT NOT NULL DEFAULT '',
    exceptions TEXT NOT NULL DEFAULT '',
    management_explanation TEXT NOT NULL DEFAULT '',
    conclusion TEXT NOT NULL DEFAULT '',
    content_hash BLOB CHECK (content_hash IS NULL OR length(content_hash) = 32),
    UNIQUE (workpaper_id, revision_number),
    CHECK (
        supersedes_revision_id IS NULL
        OR supersedes_revision_id <> workpaper_revision_id
    )
);

CREATE INDEX idx_workpaper_revisions_workpaper_number
    ON workpaper_revisions(workpaper_id, revision_number DESC);

CREATE TRIGGER trg_workpaper_revisions_no_update
BEFORE UPDATE ON workpaper_revisions
BEGIN
    SELECT RAISE(ABORT, 'workpaper revisions are immutable');
END;

CREATE TRIGGER trg_workpaper_revisions_no_delete
BEFORE DELETE ON workpaper_revisions
BEGIN
    SELECT RAISE(ABORT, 'workpaper revisions are immutable');
END;

CREATE TABLE workpaper_evidence_links (
    evidence_link_id TEXT PRIMARY KEY NOT NULL,
    workpaper_revision_id TEXT NOT NULL REFERENCES workpaper_revisions(workpaper_revision_id),
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    content_version_id TEXT REFERENCES content_versions(content_version_id),
    controlled_evidence_version_id TEXT REFERENCES controlled_evidence_versions(controlled_evidence_version_id),
    relationship_type TEXT NOT NULL CHECK (length(trim(relationship_type)) > 0),
    description TEXT,
    created_at_ms INTEGER NOT NULL,
    CHECK (
        (content_version_id IS NOT NULL AND controlled_evidence_version_id IS NULL)
        OR
        (content_version_id IS NULL AND controlled_evidence_version_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX idx_workpaper_evidence_exact_version
    ON workpaper_evidence_links(
        workpaper_revision_id,
        document_id,
        COALESCE(content_version_id, ''),
        COALESCE(controlled_evidence_version_id, ''),
        lower(trim(relationship_type))
    );

CREATE INDEX idx_workpaper_evidence_revision
    ON workpaper_evidence_links(workpaper_revision_id, created_at_ms);

CREATE TRIGGER trg_workpaper_evidence_links_no_update
BEFORE UPDATE ON workpaper_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'workpaper evidence links are immutable');
END;

CREATE TRIGGER trg_workpaper_evidence_links_no_delete
BEFORE DELETE ON workpaper_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'workpaper evidence links are immutable');
END;
