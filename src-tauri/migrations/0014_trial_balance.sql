CREATE TABLE trial_balance_imports (
    trial_balance_import_id TEXT PRIMARY KEY NOT NULL,
    engagement_id TEXT NOT NULL REFERENCES engagements(engagement_id),
    controlled_evidence_version_id TEXT NOT NULL REFERENCES controlled_evidence_versions(controlled_evidence_version_id),
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    source_content_version_id TEXT NOT NULL REFERENCES content_versions(content_version_id),
    source_sha256 BLOB NOT NULL CHECK (length(source_sha256) = 32),
    sheet_name TEXT NOT NULL CHECK (length(trim(sheet_name)) > 0),
    header_row_number INTEGER NOT NULL CHECK (header_row_number >= 1),
    account_name_column INTEGER NOT NULL CHECK (account_name_column >= 0),
    account_code_column INTEGER CHECK (account_code_column IS NULL OR account_code_column >= 0),
    opening_balance_column INTEGER CHECK (
        opening_balance_column IS NULL OR opening_balance_column >= 0
    ),
    closing_balance_column INTEGER NOT NULL CHECK (closing_balance_column >= 0),
    amount_scale INTEGER NOT NULL CHECK (amount_scale BETWEEN 0 AND 6),
    account_count INTEGER NOT NULL CHECK (account_count >= 0),
    opening_total_minor INTEGER NOT NULL,
    closing_total_minor INTEGER NOT NULL,
    imported_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_trial_balance_imports_engagement
    ON trial_balance_imports(engagement_id, imported_at_ms DESC);

CREATE TABLE trial_balance_accounts (
    trial_balance_account_id TEXT PRIMARY KEY NOT NULL,
    trial_balance_import_id TEXT NOT NULL REFERENCES trial_balance_imports(trial_balance_import_id),
    source_row_number INTEGER NOT NULL CHECK (source_row_number >= 1),
    source_row_hash BLOB NOT NULL CHECK (length(source_row_hash) = 32),
    account_code_text TEXT,
    account_name_text TEXT NOT NULL CHECK (length(trim(account_name_text)) > 0),
    opening_minor INTEGER NOT NULL,
    closing_minor INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    UNIQUE (trial_balance_import_id, source_row_number)
);

CREATE INDEX idx_trial_balance_accounts_import
    ON trial_balance_accounts(trial_balance_import_id, source_row_number);

CREATE TRIGGER trg_trial_balance_imports_no_update
BEFORE UPDATE ON trial_balance_imports
BEGIN
    SELECT RAISE(ABORT, 'trial balance imports are immutable');
END;

CREATE TRIGGER trg_trial_balance_imports_no_delete
BEFORE DELETE ON trial_balance_imports
BEGIN
    SELECT RAISE(ABORT, 'trial balance imports are immutable');
END;

CREATE TRIGGER trg_trial_balance_accounts_no_update
BEFORE UPDATE ON trial_balance_accounts
BEGIN
    SELECT RAISE(ABORT, 'trial balance accounts are immutable');
END;

CREATE TRIGGER trg_trial_balance_accounts_no_delete
BEFORE DELETE ON trial_balance_accounts
BEGIN
    SELECT RAISE(ABORT, 'trial balance accounts are immutable');
END;
