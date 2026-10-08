CREATE TABLE statutory_compliance_requirements (
    statutory_compliance_requirement_id TEXT PRIMARY KEY NOT NULL,
    engagement_id TEXT NOT NULL REFERENCES engagements(engagement_id),
    firm_library_item_id TEXT NOT NULL REFERENCES firm_library_items(firm_library_item_id),
    firm_library_version_id TEXT NOT NULL REFERENCES firm_library_versions(firm_library_version_id),
    requirement_name TEXT NOT NULL CHECK (length(trim(requirement_name)) > 0),
    requirement_description TEXT,
    definition_hash BLOB NOT NULL CHECK (length(definition_hash) = 32),
    created_at_ms INTEGER NOT NULL,
    UNIQUE (engagement_id, firm_library_item_id)
);

CREATE INDEX idx_statutory_compliance_requirements_engagement
    ON statutory_compliance_requirements(engagement_id, created_at_ms, statutory_compliance_requirement_id);

CREATE TABLE statutory_compliance_assessments (
    statutory_compliance_assessment_id TEXT PRIMARY KEY NOT NULL,
    statutory_compliance_requirement_id TEXT NOT NULL REFERENCES statutory_compliance_requirements(statutory_compliance_requirement_id),
    version_number INTEGER NOT NULL CHECK (version_number >= 1),
    supersedes_assessment_id TEXT REFERENCES statutory_compliance_assessments(statutory_compliance_assessment_id),
    applicability TEXT NOT NULL CHECK (
        applicability IN ('UNDETERMINED', 'APPLICABLE', 'NOT_APPLICABLE')
    ),
    due_date TEXT,
    actual_compliance_date TEXT,
    status TEXT NOT NULL CHECK (
        status IN ('UNASSESSED', 'PENDING', 'COMPLIANT', 'EXCEPTION', 'NOT_APPLICABLE')
    ),
    exception_text TEXT,
    conclusion TEXT,
    assessed_at_ms INTEGER NOT NULL,
    UNIQUE (statutory_compliance_requirement_id, version_number)
);

CREATE INDEX idx_statutory_compliance_assessments_requirement
    ON statutory_compliance_assessments(
        statutory_compliance_requirement_id,
        version_number DESC,
        assessed_at_ms DESC
    );

CREATE TABLE statutory_compliance_evidence_links (
    statutory_compliance_evidence_link_id TEXT PRIMARY KEY NOT NULL,
    statutory_compliance_assessment_id TEXT NOT NULL REFERENCES statutory_compliance_assessments(statutory_compliance_assessment_id),
    controlled_evidence_version_id TEXT NOT NULL REFERENCES controlled_evidence_versions(controlled_evidence_version_id),
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    source_content_version_id TEXT NOT NULL REFERENCES content_versions(content_version_id),
    source_sha256 BLOB NOT NULL CHECK (length(source_sha256) = 32),
    linked_at_ms INTEGER NOT NULL,
    UNIQUE (statutory_compliance_assessment_id, controlled_evidence_version_id)
);

CREATE INDEX idx_statutory_compliance_evidence_assessment
    ON statutory_compliance_evidence_links(statutory_compliance_assessment_id, linked_at_ms);

CREATE TRIGGER trg_statutory_compliance_requirements_no_update
BEFORE UPDATE ON statutory_compliance_requirements
BEGIN
    SELECT RAISE(ABORT, 'statutory compliance requirements are immutable');
END;

CREATE TRIGGER trg_statutory_compliance_requirements_no_delete
BEFORE DELETE ON statutory_compliance_requirements
BEGIN
    SELECT RAISE(ABORT, 'statutory compliance requirements are immutable');
END;

CREATE TRIGGER trg_statutory_compliance_assessments_no_update
BEFORE UPDATE ON statutory_compliance_assessments
BEGIN
    SELECT RAISE(ABORT, 'statutory compliance assessments are immutable');
END;

CREATE TRIGGER trg_statutory_compliance_assessments_no_delete
BEFORE DELETE ON statutory_compliance_assessments
BEGIN
    SELECT RAISE(ABORT, 'statutory compliance assessments are immutable');
END;

CREATE TRIGGER trg_statutory_compliance_evidence_no_update
BEFORE UPDATE ON statutory_compliance_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'statutory compliance evidence links are immutable');
END;

CREATE TRIGGER trg_statutory_compliance_evidence_no_delete
BEFORE DELETE ON statutory_compliance_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'statutory compliance evidence links are immutable');
END;
