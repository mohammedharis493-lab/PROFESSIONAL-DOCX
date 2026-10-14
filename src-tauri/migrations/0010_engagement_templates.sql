CREATE TABLE engagement_templates (
    engagement_template_id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    normalized_name TEXT NOT NULL UNIQUE CHECK (length(trim(normalized_name)) > 0),
    description TEXT,
    created_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER
);

CREATE TABLE engagement_template_versions (
    engagement_template_version_id TEXT PRIMARY KEY NOT NULL,
    engagement_template_id TEXT NOT NULL REFERENCES engagement_templates(engagement_template_id),
    version_number INTEGER NOT NULL CHECK (version_number >= 1),
    source_engagement_id TEXT REFERENCES engagements(engagement_id),
    service_type_id TEXT NOT NULL REFERENCES service_types(service_type_id),
    definition_json TEXT NOT NULL CHECK (length(trim(definition_json)) > 0),
    created_at_ms INTEGER NOT NULL,
    UNIQUE (engagement_template_id, version_number)
);

CREATE INDEX idx_engagement_template_versions_template
    ON engagement_template_versions(engagement_template_id, version_number DESC);

CREATE TRIGGER trg_engagement_template_versions_no_update
BEFORE UPDATE ON engagement_template_versions
BEGIN
    SELECT RAISE(ABORT, 'engagement template versions are immutable');
END;

CREATE TRIGGER trg_engagement_template_versions_no_delete
BEFORE DELETE ON engagement_template_versions
BEGIN
    SELECT RAISE(ABORT, 'engagement template versions are immutable');
END;
