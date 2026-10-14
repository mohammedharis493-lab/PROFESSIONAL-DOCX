CREATE TABLE financial_statement_schedules (
    financial_statement_schedule_id TEXT PRIMARY KEY NOT NULL,
    engagement_id TEXT NOT NULL REFERENCES engagements(engagement_id),
    reference TEXT NOT NULL CHECK (length(trim(reference)) > 0),
    normalized_reference TEXT NOT NULL CHECK (length(trim(normalized_reference)) > 0),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    created_at_ms INTEGER NOT NULL,
    UNIQUE (engagement_id, normalized_reference)
);

CREATE INDEX idx_financial_statement_schedules_engagement
    ON financial_statement_schedules(engagement_id, normalized_reference);

CREATE TABLE trial_balance_schedule_mappings (
    trial_balance_schedule_mapping_id TEXT PRIMARY KEY NOT NULL,
    trial_balance_import_id TEXT NOT NULL REFERENCES trial_balance_imports(trial_balance_import_id),
    trial_balance_account_id TEXT NOT NULL REFERENCES trial_balance_accounts(trial_balance_account_id),
    financial_statement_schedule_id TEXT NOT NULL REFERENCES financial_statement_schedules(financial_statement_schedule_id),
    version_number INTEGER NOT NULL CHECK (version_number >= 1),
    supersedes_mapping_id TEXT REFERENCES trial_balance_schedule_mappings(trial_balance_schedule_mapping_id),
    mapped_at_ms INTEGER NOT NULL,
    UNIQUE (trial_balance_import_id, trial_balance_account_id, version_number)
);

CREATE INDEX idx_trial_balance_schedule_mappings_account
    ON trial_balance_schedule_mappings(
        trial_balance_import_id,
        trial_balance_account_id,
        version_number DESC
    );

CREATE INDEX idx_trial_balance_schedule_mappings_schedule
    ON trial_balance_schedule_mappings(financial_statement_schedule_id);

CREATE TRIGGER trg_financial_statement_schedules_no_update
BEFORE UPDATE ON financial_statement_schedules
BEGIN
    SELECT RAISE(ABORT, 'financial statement schedules are immutable');
END;

CREATE TRIGGER trg_financial_statement_schedules_no_delete
BEFORE DELETE ON financial_statement_schedules
BEGIN
    SELECT RAISE(ABORT, 'financial statement schedules are immutable');
END;

CREATE TRIGGER trg_trial_balance_schedule_mappings_no_update
BEFORE UPDATE ON trial_balance_schedule_mappings
BEGIN
    SELECT RAISE(ABORT, 'trial balance schedule mappings are immutable');
END;

CREATE TRIGGER trg_trial_balance_schedule_mappings_no_delete
BEFORE DELETE ON trial_balance_schedule_mappings
BEGIN
    SELECT RAISE(ABORT, 'trial balance schedule mappings are immutable');
END;
