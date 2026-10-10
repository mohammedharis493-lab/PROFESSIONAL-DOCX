CREATE TABLE evidence_capture_jobs (
    evidence_capture_job_id TEXT PRIMARY KEY NOT NULL,
    file_instance_id TEXT NOT NULL REFERENCES file_instances(file_instance_id),
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    status TEXT NOT NULL CHECK (
        status IN ('CAPTURING', 'COMPLETE', 'FAILED', 'QUARANTINED')
    ),
    requested_at_ms INTEGER NOT NULL,
    started_at_ms INTEGER NOT NULL,
    completed_at_ms INTEGER,
    capture_reason TEXT NOT NULL,
    capture_policy TEXT NOT NULL,
    failure_code TEXT,
    failure_message TEXT
);

CREATE TABLE controlled_evidence_versions (
    controlled_evidence_version_id TEXT PRIMARY KEY NOT NULL,
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    source_file_instance_id TEXT NOT NULL REFERENCES file_instances(file_instance_id),
    source_content_version_id TEXT NOT NULL REFERENCES content_versions(content_version_id),
    evidence_capture_job_id TEXT NOT NULL UNIQUE REFERENCES evidence_capture_jobs(evidence_capture_job_id),
    version_number INTEGER NOT NULL CHECK (version_number >= 1),
    controlled_storage_locator TEXT NOT NULL UNIQUE,
    sha256 BLOB NOT NULL CHECK (length(sha256) = 32),
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    captured_at_ms INTEGER NOT NULL,
    captured_by TEXT,
    capture_reason TEXT NOT NULL,
    capture_policy TEXT NOT NULL,
    retention_state TEXT NOT NULL CHECK (retention_state IN ('RETAINED')),
    verification_state TEXT NOT NULL CHECK (verification_state IN ('HASH_VERIFIED', 'QUARANTINED')),
    source_stable_during_read INTEGER NOT NULL CHECK (source_stable_during_read = 1),
    UNIQUE (document_id, version_number)
);

CREATE TABLE audit_events (
    audit_event_id TEXT PRIMARY KEY NOT NULL,
    event_type TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    related_entity_type TEXT,
    related_entity_id TEXT,
    occurred_at_ms INTEGER NOT NULL,
    actor_id TEXT,
    details_json TEXT NOT NULL
);

CREATE INDEX idx_evidence_capture_jobs_document_status
    ON evidence_capture_jobs(document_id, status);

CREATE INDEX idx_controlled_evidence_document_version
    ON controlled_evidence_versions(document_id, version_number DESC);

CREATE INDEX idx_audit_events_entity_time
    ON audit_events(entity_type, entity_id, occurred_at_ms);
