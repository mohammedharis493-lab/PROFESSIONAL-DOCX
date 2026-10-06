CREATE TABLE pbc_requests (
    pbc_request_id TEXT PRIMARY KEY NOT NULL,
    engagement_id TEXT NOT NULL REFERENCES engagements(engagement_id),
    engagement_area_id TEXT REFERENCES engagement_areas(engagement_area_id),
    request_number TEXT NOT NULL CHECK (length(trim(request_number)) > 0),
    description TEXT NOT NULL CHECK (length(trim(description)) > 0),
    requested_from_party TEXT NOT NULL CHECK (length(trim(requested_from_party)) > 0),
    due_at_ms INTEGER,
    status TEXT NOT NULL CHECK (length(trim(status)) > 0),
    client_visible_content TEXT,
    internal_notes TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER
);

CREATE UNIQUE INDEX idx_pbc_requests_active_number
    ON pbc_requests(engagement_id, lower(trim(request_number)))
    WHERE archived_at_ms IS NULL;

CREATE INDEX idx_pbc_requests_engagement_status
    ON pbc_requests(engagement_id, status, due_at_ms, created_at_ms);

CREATE TABLE pbc_request_events (
    pbc_request_event_id TEXT PRIMARY KEY NOT NULL,
    pbc_request_id TEXT NOT NULL REFERENCES pbc_requests(pbc_request_id),
    event_type TEXT NOT NULL CHECK (
        event_type IN ('CREATED', 'STATUS_CHANGED', 'ASSESSMENT_ADDED')
    ),
    actor_id TEXT,
    from_status TEXT,
    to_status TEXT,
    assessment_text TEXT,
    comment TEXT,
    occurred_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_pbc_request_events_request
    ON pbc_request_events(pbc_request_id, occurred_at_ms);

CREATE TRIGGER trg_pbc_request_events_no_update
BEFORE UPDATE ON pbc_request_events
BEGIN
    SELECT RAISE(ABORT, 'PBC request events are immutable');
END;

CREATE TRIGGER trg_pbc_request_events_no_delete
BEFORE DELETE ON pbc_request_events
BEGIN
    SELECT RAISE(ABORT, 'PBC request events are immutable');
END;

CREATE TABLE pbc_request_evidence_links (
    pbc_request_evidence_link_id TEXT PRIMARY KEY NOT NULL,
    pbc_request_id TEXT NOT NULL REFERENCES pbc_requests(pbc_request_id),
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    content_version_id TEXT REFERENCES content_versions(content_version_id),
    controlled_evidence_version_id TEXT REFERENCES controlled_evidence_versions(controlled_evidence_version_id),
    description TEXT,
    created_at_ms INTEGER NOT NULL,
    CHECK (
        (content_version_id IS NOT NULL AND controlled_evidence_version_id IS NULL)
        OR
        (content_version_id IS NULL AND controlled_evidence_version_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX idx_pbc_request_evidence_exact_version
    ON pbc_request_evidence_links(
        pbc_request_id,
        document_id,
        COALESCE(content_version_id, ''),
        COALESCE(controlled_evidence_version_id, '')
    );

CREATE INDEX idx_pbc_request_evidence_request
    ON pbc_request_evidence_links(pbc_request_id, created_at_ms);

CREATE TRIGGER trg_pbc_request_evidence_no_update
BEFORE UPDATE ON pbc_request_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'PBC request evidence links are immutable');
END;

CREATE TRIGGER trg_pbc_request_evidence_no_delete
BEFORE DELETE ON pbc_request_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'PBC request evidence links are immutable');
END;
