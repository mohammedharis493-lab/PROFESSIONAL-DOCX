CREATE TABLE ledger_tb_mappings (
    ledger_tb_mapping_id TEXT PRIMARY KEY NOT NULL,
    ledger_import_id TEXT NOT NULL REFERENCES ledger_imports(ledger_import_id),
    trial_balance_import_id TEXT NOT NULL REFERENCES trial_balance_imports(trial_balance_import_id),
    ledger_account_key TEXT NOT NULL CHECK (length(trim(ledger_account_key)) > 0),
    ledger_account_text TEXT NOT NULL CHECK (length(trim(ledger_account_text)) > 0),
    trial_balance_account_id TEXT NOT NULL REFERENCES trial_balance_accounts(trial_balance_account_id),
    version_number INTEGER NOT NULL CHECK (version_number >= 1),
    supersedes_mapping_id TEXT REFERENCES ledger_tb_mappings(ledger_tb_mapping_id),
    mapped_at_ms INTEGER NOT NULL,
    UNIQUE (
        ledger_import_id,
        trial_balance_import_id,
        ledger_account_key,
        version_number
    )
);

CREATE INDEX idx_ledger_tb_mappings_pair
    ON ledger_tb_mappings(
        ledger_import_id,
        trial_balance_import_id,
        ledger_account_key,
        version_number DESC
    );

CREATE INDEX idx_ledger_tb_mappings_tb_account
    ON ledger_tb_mappings(trial_balance_account_id);

CREATE TRIGGER trg_ledger_tb_mappings_no_update
BEFORE UPDATE ON ledger_tb_mappings
BEGIN
    SELECT RAISE(ABORT, 'ledger to trial balance mappings are immutable');
END;

CREATE TRIGGER trg_ledger_tb_mappings_no_delete
BEFORE DELETE ON ledger_tb_mappings
BEGIN
    SELECT RAISE(ABORT, 'ledger to trial balance mappings are immutable');
END;
