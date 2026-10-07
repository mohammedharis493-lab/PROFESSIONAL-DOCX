CREATE TABLE ledger_imports (
    ledger_import_id TEXT PRIMARY KEY NOT NULL,
    engagement_id TEXT NOT NULL REFERENCES engagements(engagement_id),
    controlled_evidence_version_id TEXT NOT NULL REFERENCES controlled_evidence_versions(controlled_evidence_version_id),
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    source_content_version_id TEXT NOT NULL REFERENCES content_versions(content_version_id),
    source_sha256 BLOB NOT NULL CHECK (length(source_sha256) = 32),
    sheet_name TEXT NOT NULL CHECK (length(trim(sheet_name)) > 0),
    header_row_number INTEGER NOT NULL CHECK (header_row_number >= 1),
    amount_column INTEGER NOT NULL CHECK (amount_column >= 0),
    date_column INTEGER CHECK (date_column IS NULL OR date_column >= 0),
    account_column INTEGER CHECK (account_column IS NULL OR account_column >= 0),
    voucher_column INTEGER CHECK (voucher_column IS NULL OR voucher_column >= 0),
    narration_column INTEGER CHECK (narration_column IS NULL OR narration_column >= 0),
    amount_scale INTEGER NOT NULL CHECK (amount_scale BETWEEN 0 AND 6),
    transaction_count INTEGER NOT NULL CHECK (transaction_count >= 0),
    imported_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_ledger_imports_engagement
    ON ledger_imports(engagement_id, imported_at_ms DESC);

CREATE TABLE ledger_transactions (
    ledger_transaction_id TEXT PRIMARY KEY NOT NULL,
    ledger_import_id TEXT NOT NULL REFERENCES ledger_imports(ledger_import_id),
    source_row_number INTEGER NOT NULL CHECK (source_row_number >= 1),
    source_row_json TEXT NOT NULL CHECK (length(trim(source_row_json)) > 0),
    source_row_hash BLOB NOT NULL CHECK (length(source_row_hash) = 32),
    transaction_date_text TEXT,
    account_text TEXT,
    voucher_text TEXT,
    narration_text TEXT,
    amount_minor INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    UNIQUE (ledger_import_id, source_row_number)
);

CREATE INDEX idx_ledger_transactions_import_amount
    ON ledger_transactions(ledger_import_id, amount_minor);

CREATE TABLE ledger_test_runs (
    ledger_test_run_id TEXT PRIMARY KEY NOT NULL,
    ledger_import_id TEXT NOT NULL REFERENCES ledger_imports(ledger_import_id),
    test_type TEXT NOT NULL CHECK (test_type = 'HIGH_VALUE'),
    parameters_json TEXT NOT NULL CHECK (length(trim(parameters_json)) > 0),
    exception_count INTEGER NOT NULL CHECK (exception_count >= 0),
    ran_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_ledger_test_runs_import
    ON ledger_test_runs(ledger_import_id, ran_at_ms DESC);

CREATE TABLE ledger_exceptions (
    ledger_exception_id TEXT PRIMARY KEY NOT NULL,
    ledger_test_run_id TEXT NOT NULL REFERENCES ledger_test_runs(ledger_test_run_id),
    ledger_transaction_id TEXT NOT NULL REFERENCES ledger_transactions(ledger_transaction_id),
    exception_code TEXT NOT NULL CHECK (length(trim(exception_code)) > 0),
    created_at_ms INTEGER NOT NULL,
    UNIQUE (ledger_test_run_id, ledger_transaction_id)
);

CREATE INDEX idx_ledger_exceptions_run
    ON ledger_exceptions(ledger_test_run_id, created_at_ms);

CREATE TRIGGER trg_ledger_imports_no_update
BEFORE UPDATE ON ledger_imports
BEGIN
    SELECT RAISE(ABORT, 'ledger imports are immutable');
END;

CREATE TRIGGER trg_ledger_imports_no_delete
BEFORE DELETE ON ledger_imports
BEGIN
    SELECT RAISE(ABORT, 'ledger imports are immutable');
END;

CREATE TRIGGER trg_ledger_transactions_no_update
BEFORE UPDATE ON ledger_transactions
BEGIN
    SELECT RAISE(ABORT, 'ledger transactions are immutable');
END;

CREATE TRIGGER trg_ledger_transactions_no_delete
BEFORE DELETE ON ledger_transactions
BEGIN
    SELECT RAISE(ABORT, 'ledger transactions are immutable');
END;

CREATE TRIGGER trg_ledger_test_runs_no_update
BEFORE UPDATE ON ledger_test_runs
BEGIN
    SELECT RAISE(ABORT, 'ledger test runs are immutable');
END;

CREATE TRIGGER trg_ledger_test_runs_no_delete
BEFORE DELETE ON ledger_test_runs
BEGIN
    SELECT RAISE(ABORT, 'ledger test runs are immutable');
END;

CREATE TRIGGER trg_ledger_exceptions_no_update
BEFORE UPDATE ON ledger_exceptions
BEGIN
    SELECT RAISE(ABORT, 'ledger exceptions are immutable');
END;

CREATE TRIGGER trg_ledger_exceptions_no_delete
BEFORE DELETE ON ledger_exceptions
BEGIN
    SELECT RAISE(ABORT, 'ledger exceptions are immutable');
END;
