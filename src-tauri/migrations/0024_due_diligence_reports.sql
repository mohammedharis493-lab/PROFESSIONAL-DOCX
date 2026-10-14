CREATE TABLE due_diligence_reports (
    due_diligence_report_id TEXT PRIMARY KEY NOT NULL,
    due_diligence_workspace_id TEXT NOT NULL
        REFERENCES due_diligence_workspaces(due_diligence_workspace_id),
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_due_diligence_reports_workspace
    ON due_diligence_reports(due_diligence_workspace_id, created_at_ms DESC);

CREATE TABLE due_diligence_report_versions (
    due_diligence_report_version_id TEXT PRIMARY KEY NOT NULL,
    due_diligence_report_id TEXT NOT NULL
        REFERENCES due_diligence_reports(due_diligence_report_id),
    version_number INTEGER NOT NULL CHECK (version_number >= 1),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    executive_summary TEXT,
    scope_summary TEXT,
    overall_conclusion TEXT,
    issue_count INTEGER NOT NULL CHECK (issue_count >= 0),
    issue_snapshot_hash BLOB NOT NULL CHECK (length(issue_snapshot_hash) = 32),
    created_by TEXT,
    created_at_ms INTEGER NOT NULL,
    UNIQUE (due_diligence_report_id, version_number)
);

CREATE INDEX idx_due_diligence_report_versions_report
    ON due_diligence_report_versions(due_diligence_report_id, version_number DESC);

CREATE TABLE due_diligence_report_issue_links (
    due_diligence_report_issue_link_id TEXT PRIMARY KEY NOT NULL,
    due_diligence_report_version_id TEXT NOT NULL
        REFERENCES due_diligence_report_versions(due_diligence_report_version_id),
    due_diligence_issue_id TEXT NOT NULL
        REFERENCES due_diligence_issues(due_diligence_issue_id),
    due_diligence_issue_event_id TEXT NOT NULL
        REFERENCES due_diligence_issue_events(due_diligence_issue_event_id),
    linked_at_ms INTEGER NOT NULL,
    UNIQUE (due_diligence_report_version_id, due_diligence_issue_id)
);

CREATE INDEX idx_due_diligence_report_issue_links_version
    ON due_diligence_report_issue_links(due_diligence_report_version_id);

CREATE TRIGGER trg_due_diligence_reports_no_update
BEFORE UPDATE ON due_diligence_reports
BEGIN
    SELECT RAISE(ABORT, 'due diligence reports are immutable');
END;

CREATE TRIGGER trg_due_diligence_reports_no_delete
BEFORE DELETE ON due_diligence_reports
BEGIN
    SELECT RAISE(ABORT, 'due diligence reports are immutable');
END;

CREATE TRIGGER trg_due_diligence_report_versions_no_update
BEFORE UPDATE ON due_diligence_report_versions
BEGIN
    SELECT RAISE(ABORT, 'due diligence report versions are immutable');
END;

CREATE TRIGGER trg_due_diligence_report_versions_no_delete
BEFORE DELETE ON due_diligence_report_versions
BEGIN
    SELECT RAISE(ABORT, 'due diligence report versions are immutable');
END;

CREATE TRIGGER trg_due_diligence_report_issue_links_no_update
BEFORE UPDATE ON due_diligence_report_issue_links
BEGIN
    SELECT RAISE(ABORT, 'due diligence report issue links are immutable');
END;

CREATE TRIGGER trg_due_diligence_report_issue_links_no_delete
BEFORE DELETE ON due_diligence_report_issue_links
BEGIN
    SELECT RAISE(ABORT, 'due diligence report issue links are immutable');
END;
