CREATE TABLE firm_library_items (
    firm_library_item_id TEXT PRIMARY KEY NOT NULL,
    category TEXT NOT NULL CHECK (
        category IN (
            'CHECKLIST',
            'AUDIT_QUERY',
            'RISK_TEMPLATE',
            'CONTROL_TEMPLATE',
            'LEDGER_SCRUTINY_TEST',
            'REPORT_TEMPLATE',
            'MANAGEMENT_LETTER_POINT',
            'STATUTORY_COMPLIANCE_REQUIREMENT'
        )
    ),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    normalized_name TEXT NOT NULL CHECK (length(trim(normalized_name)) > 0),
    description TEXT,
    service_type_id TEXT REFERENCES service_types(service_type_id),
    created_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER
);

CREATE UNIQUE INDEX idx_firm_library_items_active_identity
    ON firm_library_items(
        category,
        COALESCE(service_type_id, ''),
        normalized_name
    )
    WHERE archived_at_ms IS NULL;

CREATE TABLE firm_library_versions (
    firm_library_version_id TEXT PRIMARY KEY NOT NULL,
    firm_library_item_id TEXT NOT NULL REFERENCES firm_library_items(firm_library_item_id),
    version_number INTEGER NOT NULL CHECK (version_number >= 1),
    definition_json TEXT NOT NULL CHECK (length(trim(definition_json)) > 0),
    definition_hash BLOB NOT NULL CHECK (length(definition_hash) = 32),
    created_at_ms INTEGER NOT NULL,
    UNIQUE (firm_library_item_id, version_number)
);

CREATE INDEX idx_firm_library_versions_item
    ON firm_library_versions(firm_library_item_id, version_number DESC);

CREATE TRIGGER trg_firm_library_versions_no_update
BEFORE UPDATE ON firm_library_versions
BEGIN
    SELECT RAISE(ABORT, 'firm library versions are immutable');
END;

CREATE TRIGGER trg_firm_library_versions_no_delete
BEFORE DELETE ON firm_library_versions
BEGIN
    SELECT RAISE(ABORT, 'firm library versions are immutable');
END;
