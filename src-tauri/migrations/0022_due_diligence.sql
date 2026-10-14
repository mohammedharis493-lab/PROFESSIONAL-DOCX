CREATE TABLE due_diligence_workspaces (
    due_diligence_workspace_id TEXT PRIMARY KEY NOT NULL,
    engagement_id TEXT NOT NULL REFERENCES engagements(engagement_id),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    created_at_ms INTEGER NOT NULL,
    UNIQUE (engagement_id)
);

CREATE TABLE due_diligence_sections (
    due_diligence_section_id TEXT PRIMARY KEY NOT NULL,
    due_diligence_workspace_id TEXT NOT NULL REFERENCES due_diligence_workspaces(due_diligence_workspace_id),
    parent_section_id TEXT REFERENCES due_diligence_sections(due_diligence_section_id),
    code TEXT,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    description TEXT,
    display_order INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_due_diligence_sections_workspace
    ON due_diligence_sections(due_diligence_workspace_id, display_order, created_at_ms);

CREATE TABLE due_diligence_requests (
    due_diligence_request_id TEXT PRIMARY KEY NOT NULL,
    due_diligence_workspace_id TEXT NOT NULL REFERENCES due_diligence_workspaces(due_diligence_workspace_id),
    due_diligence_section_id TEXT REFERENCES due_diligence_sections(due_diligence_section_id),
    reference TEXT,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    description TEXT,
    requested_from_party TEXT,
    due_date TEXT,
    internal_notes TEXT,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_due_diligence_requests_workspace
    ON due_diligence_requests(due_diligence_workspace_id, created_at_ms DESC);

CREATE TABLE due_diligence_request_events (
    due_diligence_request_event_id TEXT PRIMARY KEY NOT NULL,
    due_diligence_request_id TEXT NOT NULL REFERENCES due_diligence_requests(due_diligence_request_id),
    sequence_number INTEGER NOT NULL CHECK (sequence_number >= 1),
    status TEXT NOT NULL CHECK (
        status IN ('OPEN', 'PARTIALLY_RESPONDED', 'RESPONDED', 'CLOSED', 'WITHDRAWN')
    ),
    response_text TEXT,
    internal_assessment TEXT,
    actor_id TEXT,
    occurred_at_ms INTEGER NOT NULL,
    UNIQUE (due_diligence_request_id, sequence_number)
);

CREATE INDEX idx_due_diligence_request_events_request
    ON due_diligence_request_events(due_diligence_request_id, sequence_number);

CREATE TABLE due_diligence_request_evidence_links (
    due_diligence_request_evidence_link_id TEXT PRIMARY KEY NOT NULL,
    due_diligence_request_id TEXT NOT NULL REFERENCES due_diligence_requests(due_diligence_request_id),
    due_diligence_request_event_id TEXT REFERENCES due_diligence_request_events(due_diligence_request_event_id),
    controlled_evidence_version_id TEXT NOT NULL REFERENCES controlled_evidence_versions(controlled_evidence_version_id),
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    source_content_version_id TEXT NOT NULL REFERENCES content_versions(content_version_id),
    source_sha256 BLOB NOT NULL CHECK (length(source_sha256) = 32),
    description TEXT,
    linked_at_ms INTEGER NOT NULL,
    UNIQUE (
        due_diligence_request_id,
        due_diligence_request_event_id,
        controlled_evidence_version_id
    )
);

CREATE INDEX idx_due_diligence_request_evidence_request
    ON due_diligence_request_evidence_links(due_diligence_request_id, linked_at_ms);

CREATE TRIGGER trg_due_diligence_workspaces_no_update
BEFORE UPDATE ON due_diligence_workspaces
BEGIN
    SELECT RAISE(ABORT, 'due diligence workspaces are immutable');
END;

CREATE TRIGGER trg_due_diligence_workspaces_no_delete
BEFORE DELETE ON due_diligence_workspaces
BEGIN
    SELECT RAISE(ABORT, 'due diligence workspaces are immutable');
END;

CREATE TRIGGER trg_due_diligence_sections_no_update
BEFORE UPDATE ON due_diligence_sections
BEGIN
    SELECT RAISE(ABORT, 'due diligence sections are immutable');
END;

CREATE TRIGGER trg_due_diligence_sections_no_delete
BEFORE DELETE ON due_diligence_sections
BEGIN
    SELECT RAISE(ABORT, 'due diligence sections are immutable');
END;

CREATE TRIGGER trg_due_diligence_requests_no_update
BEFORE UPDATE ON due_diligence_requests
BEGIN
    SELECT RAISE(ABORT, 'due diligence requests are immutable');
END;

CREATE TRIGGER trg_due_diligence_requests_no_delete
BEFORE DELETE ON due_diligence_requests
BEGIN
    SELECT RAISE(ABORT, 'due diligence requests are immutable');
END;

CREATE TRIGGER trg_due_diligence_request_events_no_update
BEFORE UPDATE ON due_diligence_request_events
BEGIN
    SELECT RAISE(ABORT, 'due diligence request events are immutable');
END;

CREATE TRIGGER trg_due_diligence_request_events_no_delete
BEFORE DELETE ON due_diligence_request_events
BEGIN
    SELECT RAISE(ABORT, 'due diligence request events are immutable');
END;

CREATE TRIGGER trg_due_diligence_request_evidence_no_update
BEFORE UPDATE ON due_diligence_request_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'due diligence request evidence links are immutable');
END;

CREATE TRIGGER trg_due_diligence_request_evidence_no_delete
BEFORE DELETE ON due_diligence_request_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'due diligence request evidence links are immutable');
END;
