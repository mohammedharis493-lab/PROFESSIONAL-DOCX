CREATE TABLE internal_audit_processes (
    internal_audit_process_id TEXT PRIMARY KEY NOT NULL,
    engagement_id TEXT NOT NULL REFERENCES engagements(engagement_id),
    parent_process_id TEXT REFERENCES internal_audit_processes(internal_audit_process_id),
    code TEXT,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    description TEXT,
    display_order INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL CHECK (length(trim(status)) > 0),
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_internal_audit_processes_engagement
    ON internal_audit_processes(engagement_id, display_order, created_at_ms);

CREATE TABLE internal_audit_objectives (
    internal_audit_objective_id TEXT PRIMARY KEY NOT NULL,
    internal_audit_process_id TEXT NOT NULL REFERENCES internal_audit_processes(internal_audit_process_id),
    reference TEXT,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    description TEXT,
    status TEXT NOT NULL CHECK (length(trim(status)) > 0),
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_internal_audit_objectives_process
    ON internal_audit_objectives(internal_audit_process_id, created_at_ms);

CREATE TABLE internal_audit_risks (
    internal_audit_risk_id TEXT PRIMARY KEY NOT NULL,
    internal_audit_objective_id TEXT NOT NULL REFERENCES internal_audit_objectives(internal_audit_objective_id),
    reference TEXT,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    description TEXT,
    risk_classification TEXT,
    inherent_rating TEXT,
    status TEXT NOT NULL CHECK (length(trim(status)) > 0),
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_internal_audit_risks_objective
    ON internal_audit_risks(internal_audit_objective_id, created_at_ms);

CREATE TABLE internal_audit_controls (
    internal_audit_control_id TEXT PRIMARY KEY NOT NULL,
    internal_audit_risk_id TEXT NOT NULL REFERENCES internal_audit_risks(internal_audit_risk_id),
    reference TEXT,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    description TEXT,
    control_type TEXT,
    frequency TEXT,
    owner_text TEXT,
    status TEXT NOT NULL CHECK (length(trim(status)) > 0),
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_internal_audit_controls_risk
    ON internal_audit_controls(internal_audit_risk_id, created_at_ms);

CREATE TABLE internal_audit_tests (
    internal_audit_test_id TEXT PRIMARY KEY NOT NULL,
    internal_audit_control_id TEXT NOT NULL REFERENCES internal_audit_controls(internal_audit_control_id),
    reference TEXT,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    procedure_text TEXT NOT NULL CHECK (length(trim(procedure_text)) > 0),
    sample_strategy TEXT,
    expected_result TEXT,
    status TEXT NOT NULL CHECK (length(trim(status)) > 0),
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_internal_audit_tests_control
    ON internal_audit_tests(internal_audit_control_id, created_at_ms);

CREATE TABLE internal_audit_test_evidence_links (
    internal_audit_test_evidence_link_id TEXT PRIMARY KEY NOT NULL,
    internal_audit_test_id TEXT NOT NULL REFERENCES internal_audit_tests(internal_audit_test_id),
    controlled_evidence_version_id TEXT NOT NULL REFERENCES controlled_evidence_versions(controlled_evidence_version_id),
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    source_content_version_id TEXT NOT NULL REFERENCES content_versions(content_version_id),
    source_sha256 BLOB NOT NULL CHECK (length(source_sha256) = 32),
    description TEXT,
    linked_at_ms INTEGER NOT NULL,
    UNIQUE (internal_audit_test_id, controlled_evidence_version_id)
);

CREATE INDEX idx_internal_audit_test_evidence_links_test
    ON internal_audit_test_evidence_links(internal_audit_test_id, linked_at_ms);

CREATE TRIGGER trg_internal_audit_test_evidence_links_no_update
BEFORE UPDATE ON internal_audit_test_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'internal audit test evidence links are immutable');
END;

CREATE TRIGGER trg_internal_audit_test_evidence_links_no_delete
BEFORE DELETE ON internal_audit_test_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'internal audit test evidence links are immutable');
END;
