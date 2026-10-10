CREATE TABLE internal_audit_findings (
    internal_audit_finding_id TEXT PRIMARY KEY NOT NULL,
    origin_engagement_id TEXT NOT NULL REFERENCES engagements(engagement_id),
    internal_audit_process_id TEXT NOT NULL REFERENCES internal_audit_processes(internal_audit_process_id),
    internal_audit_risk_id TEXT REFERENCES internal_audit_risks(internal_audit_risk_id),
    internal_audit_control_id TEXT REFERENCES internal_audit_controls(internal_audit_control_id),
    internal_audit_test_id TEXT REFERENCES internal_audit_tests(internal_audit_test_id),
    workpaper_id TEXT REFERENCES workpapers(workpaper_id),
    repeated_from_finding_id TEXT REFERENCES internal_audit_findings(internal_audit_finding_id),
    reference TEXT,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    condition_text TEXT NOT NULL CHECK (length(trim(condition_text)) > 0),
    criteria_text TEXT,
    cause_text TEXT,
    risk_effect_text TEXT,
    recommendation_text TEXT,
    risk_classification TEXT,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_internal_audit_findings_origin_engagement
    ON internal_audit_findings(origin_engagement_id, created_at_ms);

CREATE INDEX idx_internal_audit_findings_process
    ON internal_audit_findings(internal_audit_process_id, created_at_ms);

CREATE INDEX idx_internal_audit_findings_repeated_from
    ON internal_audit_findings(repeated_from_finding_id);

CREATE TABLE internal_audit_finding_followups (
    internal_audit_finding_followup_id TEXT PRIMARY KEY NOT NULL,
    internal_audit_finding_id TEXT NOT NULL REFERENCES internal_audit_findings(internal_audit_finding_id),
    tracking_engagement_id TEXT NOT NULL REFERENCES engagements(engagement_id),
    sequence_number INTEGER NOT NULL CHECK (sequence_number >= 1),
    status TEXT NOT NULL CHECK (
        status IN (
            'OPEN',
            'MANAGEMENT_RESPONDED',
            'ACTION_IN_PROGRESS',
            'IMPLEMENTED_PENDING_VERIFICATION',
            'CLOSED',
            'RISK_ACCEPTED'
        )
    ),
    management_response TEXT,
    action_owner TEXT,
    target_date TEXT,
    follow_up_text TEXT,
    verification_conclusion TEXT,
    actor_id TEXT,
    occurred_at_ms INTEGER NOT NULL,
    UNIQUE (internal_audit_finding_id, sequence_number)
);

CREATE INDEX idx_internal_audit_finding_followups_finding
    ON internal_audit_finding_followups(
        internal_audit_finding_id,
        sequence_number DESC
    );

CREATE INDEX idx_internal_audit_finding_followups_tracking_engagement
    ON internal_audit_finding_followups(
        tracking_engagement_id,
        occurred_at_ms
    );

CREATE TABLE internal_audit_finding_evidence_links (
    internal_audit_finding_evidence_link_id TEXT PRIMARY KEY NOT NULL,
    internal_audit_finding_id TEXT NOT NULL REFERENCES internal_audit_findings(internal_audit_finding_id),
    internal_audit_finding_followup_id TEXT REFERENCES internal_audit_finding_followups(internal_audit_finding_followup_id),
    evidence_scope_key TEXT NOT NULL CHECK (length(trim(evidence_scope_key)) > 0),
    controlled_evidence_version_id TEXT NOT NULL REFERENCES controlled_evidence_versions(controlled_evidence_version_id),
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    source_content_version_id TEXT NOT NULL REFERENCES content_versions(content_version_id),
    source_sha256 BLOB NOT NULL CHECK (length(source_sha256) = 32),
    description TEXT,
    linked_at_ms INTEGER NOT NULL,
    CHECK (
        (internal_audit_finding_followup_id IS NULL AND evidence_scope_key = 'FINDING')
        OR
        (
            internal_audit_finding_followup_id IS NOT NULL
            AND evidence_scope_key = internal_audit_finding_followup_id
        )
    ),
    UNIQUE (
        internal_audit_finding_id,
        evidence_scope_key,
        controlled_evidence_version_id
    )
);

CREATE INDEX idx_internal_audit_finding_evidence_finding
    ON internal_audit_finding_evidence_links(
        internal_audit_finding_id,
        linked_at_ms
    );

CREATE INDEX idx_internal_audit_finding_evidence_followup
    ON internal_audit_finding_evidence_links(
        internal_audit_finding_followup_id,
        linked_at_ms
    );

CREATE TRIGGER trg_internal_audit_findings_no_update
BEFORE UPDATE ON internal_audit_findings
BEGIN
    SELECT RAISE(ABORT, 'internal audit findings are immutable');
END;

CREATE TRIGGER trg_internal_audit_findings_no_delete
BEFORE DELETE ON internal_audit_findings
BEGIN
    SELECT RAISE(ABORT, 'internal audit findings are immutable');
END;

CREATE TRIGGER trg_internal_audit_finding_followups_no_update
BEFORE UPDATE ON internal_audit_finding_followups
BEGIN
    SELECT RAISE(ABORT, 'internal audit finding follow-ups are immutable');
END;

CREATE TRIGGER trg_internal_audit_finding_followups_no_delete
BEFORE DELETE ON internal_audit_finding_followups
BEGIN
    SELECT RAISE(ABORT, 'internal audit finding follow-ups are immutable');
END;

CREATE TRIGGER trg_internal_audit_finding_evidence_no_update
BEFORE UPDATE ON internal_audit_finding_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'internal audit finding evidence links are immutable');
END;

CREATE TRIGGER trg_internal_audit_finding_evidence_no_delete
BEFORE DELETE ON internal_audit_finding_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'internal audit finding evidence links are immutable');
END;
