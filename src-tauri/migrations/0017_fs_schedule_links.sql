CREATE TABLE financial_statement_schedule_links (
    financial_statement_schedule_link_id TEXT PRIMARY KEY NOT NULL,
    financial_statement_schedule_id TEXT NOT NULL
        REFERENCES financial_statement_schedules(financial_statement_schedule_id),
    controlled_evidence_version_id TEXT NOT NULL
        REFERENCES controlled_evidence_versions(controlled_evidence_version_id),
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    source_content_version_id TEXT NOT NULL REFERENCES content_versions(content_version_id),
    source_sha256 BLOB NOT NULL CHECK (length(source_sha256) = 32),
    location_kind TEXT NOT NULL CHECK (
        location_kind IN ('PAGE', 'WORKSHEET', 'CELL', 'RANGE')
    ),
    location_value TEXT NOT NULL CHECK (length(trim(location_value)) > 0),
    version_number INTEGER NOT NULL CHECK (version_number >= 1),
    supersedes_link_id TEXT
        REFERENCES financial_statement_schedule_links(financial_statement_schedule_link_id),
    linked_at_ms INTEGER NOT NULL,
    UNIQUE (financial_statement_schedule_id, version_number)
);

CREATE INDEX idx_financial_statement_schedule_links_schedule
    ON financial_statement_schedule_links(
        financial_statement_schedule_id,
        version_number DESC
    );

CREATE INDEX idx_financial_statement_schedule_links_evidence
    ON financial_statement_schedule_links(controlled_evidence_version_id);

CREATE TRIGGER trg_financial_statement_schedule_links_no_update
BEFORE UPDATE ON financial_statement_schedule_links
BEGIN
    SELECT RAISE(ABORT, 'financial statement schedule links are immutable');
END;

CREATE TRIGGER trg_financial_statement_schedule_links_no_delete
BEFORE DELETE ON financial_statement_schedule_links
BEGIN
    SELECT RAISE(ABORT, 'financial statement schedule links are immutable');
END;
