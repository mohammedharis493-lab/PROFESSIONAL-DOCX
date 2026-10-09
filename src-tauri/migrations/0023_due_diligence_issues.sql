CREATE TABLE due_diligence_issues (
    due_diligence_issue_id TEXT PRIMARY KEY NOT NULL,
    due_diligence_workspace_id TEXT NOT NULL REFERENCES due_diligence_workspaces(due_diligence_workspace_id),
    due_diligence_section_id TEXT REFERENCES due_diligence_sections(due_diligence_section_id),
    due_diligence_request_id TEXT REFERENCES due_diligence_requests(due_diligence_request_id),
    issue_type TEXT NOT NULL CHECK (issue_type IN ('FINDING', 'DEAL_ISSUE')),
    reference TEXT,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    description TEXT,
    category TEXT,
    severity TEXT,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_due_diligence_issues_workspace
    ON due_diligence_issues(due_diligence_workspace_id, created_at_ms DESC);

CREATE INDEX idx_due_diligence_issues_request
    ON due_diligence_issues(due_diligence_request_id)
    WHERE due_diligence_request_id IS NOT NULL;

CREATE TABLE due_diligence_issue_events (
    due_diligence_issue_event_id TEXT PRIMARY KEY NOT NULL,
    due_diligence_issue_id TEXT NOT NULL REFERENCES due_diligence_issues(due_diligence_issue_id),
    sequence_number INTEGER NOT NULL CHECK (sequence_number >= 1),
    status TEXT NOT NULL CHECK (
        status IN ('OPEN', 'UNDER_REVIEW', 'CONFIRMED', 'RESOLVED', 'CLOSED', 'DROPPED')
    ),
    internal_conclusion TEXT,
    deal_impact TEXT,
    recommendation TEXT,
    actor_id TEXT,
    occurred_at_ms INTEGER NOT NULL,
    UNIQUE (due_diligence_issue_id, sequence_number)
);

CREATE INDEX idx_due_diligence_issue_events_issue
    ON due_diligence_issue_events(due_diligence_issue_id, sequence_number);

CREATE TABLE due_diligence_issue_evidence_links (
    due_diligence_issue_evidence_link_id TEXT PRIMARY KEY NOT NULL,
    due_diligence_issue_id TEXT NOT NULL REFERENCES due_diligence_issues(due_diligence_issue_id),
    due_diligence_issue_event_id TEXT REFERENCES due_diligence_issue_events(due_diligence_issue_event_id),
    controlled_evidence_version_id TEXT NOT NULL REFERENCES controlled_evidence_versions(controlled_evidence_version_id),
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    source_content_version_id TEXT NOT NULL REFERENCES content_versions(content_version_id),
    source_sha256 BLOB NOT NULL CHECK (length(source_sha256) = 32),
    description TEXT,
    linked_at_ms INTEGER NOT NULL,
    UNIQUE (
        due_diligence_issue_id,
        due_diligence_issue_event_id,
        controlled_evidence_version_id
    )
);

CREATE INDEX idx_due_diligence_issue_evidence_issue
    ON due_diligence_issue_evidence_links(due_diligence_issue_id, linked_at_ms);

CREATE TRIGGER trg_due_diligence_issues_no_update
BEFORE UPDATE ON due_diligence_issues
BEGIN
    SELECT RAISE(ABORT, 'due diligence issues are immutable');
END;

CREATE TRIGGER trg_due_diligence_issues_no_delete
BEFORE DELETE ON due_diligence_issues
BEGIN
    SELECT RAISE(ABORT, 'due diligence issues are immutable');
END;

CREATE TRIGGER trg_due_diligence_issue_events_no_update
BEFORE UPDATE ON due_diligence_issue_events
BEGIN
    SELECT RAISE(ABORT, 'due diligence issue events are immutable');
END;

CREATE TRIGGER trg_due_diligence_issue_events_no_delete
BEFORE DELETE ON due_diligence_issue_events
BEGIN
    SELECT RAISE(ABORT, 'due diligence issue events are immutable');
END;

CREATE TRIGGER trg_due_diligence_issue_evidence_no_update
BEFORE UPDATE ON due_diligence_issue_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'due diligence issue evidence links are immutable');
END;

CREATE TRIGGER trg_due_diligence_issue_evidence_no_delete
BEFORE DELETE ON due_diligence_issue_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'due diligence issue evidence links are immutable');
END;
