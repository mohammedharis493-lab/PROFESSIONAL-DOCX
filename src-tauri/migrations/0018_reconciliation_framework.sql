CREATE TABLE reconciliation_runs (
    reconciliation_run_id TEXT PRIMARY KEY NOT NULL,
    engagement_id TEXT NOT NULL REFERENCES engagements(engagement_id),
    reconciliation_type TEXT NOT NULL CHECK (length(trim(reconciliation_type)) > 0),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    rule_code TEXT NOT NULL CHECK (rule_code = 'EXACT_KEY_AMOUNT'),
    parameters_json TEXT NOT NULL CHECK (
        json_valid(parameters_json)
        AND json_type(parameters_json) = 'object'
    ),
    left_item_count INTEGER NOT NULL CHECK (left_item_count >= 0),
    right_item_count INTEGER NOT NULL CHECK (right_item_count >= 0),
    matched_pair_count INTEGER NOT NULL CHECK (matched_pair_count >= 0),
    exception_count INTEGER NOT NULL CHECK (exception_count >= 0),
    ran_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_reconciliation_runs_engagement
    ON reconciliation_runs(engagement_id, ran_at_ms DESC);

CREATE TABLE reconciliation_items (
    reconciliation_item_id TEXT PRIMARY KEY NOT NULL,
    reconciliation_run_id TEXT NOT NULL
        REFERENCES reconciliation_runs(reconciliation_run_id),
    side TEXT NOT NULL CHECK (side IN ('LEFT', 'RIGHT')),
    match_key TEXT NOT NULL CHECK (length(trim(match_key)) > 0),
    amount_minor INTEGER NOT NULL,
    event_date_text TEXT,
    description_text TEXT,
    source_kind TEXT NOT NULL CHECK (length(trim(source_kind)) > 0),
    source_entity_id TEXT NOT NULL CHECK (length(trim(source_entity_id)) > 0),
    controlled_evidence_version_id TEXT NOT NULL
        REFERENCES controlled_evidence_versions(controlled_evidence_version_id),
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    source_content_version_id TEXT NOT NULL REFERENCES content_versions(content_version_id),
    source_sha256 BLOB NOT NULL CHECK (length(source_sha256) = 32),
    sheet_name TEXT,
    source_row_number INTEGER CHECK (
        source_row_number IS NULL OR source_row_number >= 1
    ),
    source_row_hash BLOB CHECK (
        source_row_hash IS NULL OR length(source_row_hash) = 32
    ),
    created_at_ms INTEGER NOT NULL,
    UNIQUE (
        reconciliation_run_id,
        side,
        source_kind,
        source_entity_id
    )
);

CREATE INDEX idx_reconciliation_items_run_side
    ON reconciliation_items(
        reconciliation_run_id,
        side,
        match_key,
        amount_minor
    );

CREATE INDEX idx_reconciliation_items_evidence
    ON reconciliation_items(controlled_evidence_version_id);

CREATE TABLE reconciliation_matches (
    reconciliation_match_id TEXT PRIMARY KEY NOT NULL,
    reconciliation_run_id TEXT NOT NULL
        REFERENCES reconciliation_runs(reconciliation_run_id),
    left_item_id TEXT NOT NULL REFERENCES reconciliation_items(reconciliation_item_id),
    right_item_id TEXT NOT NULL REFERENCES reconciliation_items(reconciliation_item_id),
    matched_at_ms INTEGER NOT NULL,
    UNIQUE (reconciliation_run_id, left_item_id),
    UNIQUE (reconciliation_run_id, right_item_id)
);

CREATE INDEX idx_reconciliation_matches_run
    ON reconciliation_matches(reconciliation_run_id);

CREATE TABLE reconciliation_exceptions (
    reconciliation_exception_id TEXT PRIMARY KEY NOT NULL,
    reconciliation_run_id TEXT NOT NULL
        REFERENCES reconciliation_runs(reconciliation_run_id),
    reconciliation_item_id TEXT NOT NULL
        REFERENCES reconciliation_items(reconciliation_item_id),
    exception_code TEXT NOT NULL CHECK (
        exception_code IN ('UNMATCHED_LEFT', 'UNMATCHED_RIGHT')
    ),
    created_at_ms INTEGER NOT NULL,
    UNIQUE (reconciliation_run_id, reconciliation_item_id)
);

CREATE INDEX idx_reconciliation_exceptions_run
    ON reconciliation_exceptions(reconciliation_run_id, exception_code);

CREATE TRIGGER trg_reconciliation_runs_no_update
BEFORE UPDATE ON reconciliation_runs
BEGIN
    SELECT RAISE(ABORT, 'reconciliation runs are immutable');
END;

CREATE TRIGGER trg_reconciliation_runs_no_delete
BEFORE DELETE ON reconciliation_runs
BEGIN
    SELECT RAISE(ABORT, 'reconciliation runs are immutable');
END;

CREATE TRIGGER trg_reconciliation_items_no_update
BEFORE UPDATE ON reconciliation_items
BEGIN
    SELECT RAISE(ABORT, 'reconciliation items are immutable');
END;

CREATE TRIGGER trg_reconciliation_items_no_delete
BEFORE DELETE ON reconciliation_items
BEGIN
    SELECT RAISE(ABORT, 'reconciliation items are immutable');
END;

CREATE TRIGGER trg_reconciliation_matches_no_update
BEFORE UPDATE ON reconciliation_matches
BEGIN
    SELECT RAISE(ABORT, 'reconciliation matches are immutable');
END;

CREATE TRIGGER trg_reconciliation_matches_no_delete
BEFORE DELETE ON reconciliation_matches
BEGIN
    SELECT RAISE(ABORT, 'reconciliation matches are immutable');
END;

CREATE TRIGGER trg_reconciliation_exceptions_no_update
BEFORE UPDATE ON reconciliation_exceptions
BEGIN
    SELECT RAISE(ABORT, 'reconciliation exceptions are immutable');
END;

CREATE TRIGGER trg_reconciliation_exceptions_no_delete
BEFORE DELETE ON reconciliation_exceptions
BEGIN
    SELECT RAISE(ABORT, 'reconciliation exceptions are immutable');
END;
