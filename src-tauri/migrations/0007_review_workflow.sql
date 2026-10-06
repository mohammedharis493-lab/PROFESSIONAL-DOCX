CREATE TABLE workpaper_workflow_events (
    workpaper_workflow_event_id TEXT PRIMARY KEY NOT NULL,
    workpaper_id TEXT NOT NULL REFERENCES workpapers(workpaper_id),
    workpaper_revision_id TEXT REFERENCES workpaper_revisions(workpaper_revision_id),
    from_state TEXT NOT NULL CHECK (length(trim(from_state)) > 0),
    to_state TEXT NOT NULL CHECK (length(trim(to_state)) > 0),
    actor_id TEXT,
    comment TEXT,
    occurred_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_workpaper_workflow_events_workpaper
    ON workpaper_workflow_events(workpaper_id, occurred_at_ms, rowid);

CREATE TRIGGER trg_workpaper_workflow_events_no_update
BEFORE UPDATE ON workpaper_workflow_events
BEGIN
    SELECT RAISE(ABORT, 'workpaper workflow events are immutable');
END;

CREATE TRIGGER trg_workpaper_workflow_events_no_delete
BEFORE DELETE ON workpaper_workflow_events
BEGIN
    SELECT RAISE(ABORT, 'workpaper workflow events are immutable');
END;

CREATE TABLE review_notes (
    review_note_id TEXT PRIMARY KEY NOT NULL,
    workpaper_id TEXT NOT NULL REFERENCES workpapers(workpaper_id),
    workpaper_revision_id TEXT NOT NULL REFERENCES workpaper_revisions(workpaper_revision_id),
    evidence_link_id TEXT REFERENCES workpaper_evidence_links(evidence_link_id),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    body TEXT NOT NULL CHECK (length(trim(body)) > 0),
    owner_id TEXT,
    due_at_ms INTEGER,
    location_kind TEXT,
    location_value TEXT,
    current_state TEXT NOT NULL CHECK (
        current_state IN ('OPEN', 'RESPONSE_SUBMITTED', 'CLEARED')
    ),
    raised_by TEXT,
    created_at_ms INTEGER NOT NULL,
    latest_event_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_review_notes_workpaper_state
    ON review_notes(workpaper_id, current_state, latest_event_at_ms);

CREATE INDEX idx_review_notes_revision
    ON review_notes(workpaper_revision_id, created_at_ms);

CREATE TABLE review_note_events (
    review_note_event_id TEXT PRIMARY KEY NOT NULL,
    review_note_id TEXT NOT NULL REFERENCES review_notes(review_note_id),
    event_type TEXT NOT NULL CHECK (
        event_type IN ('RAISED', 'RESPONSE_SUBMITTED', 'CLEARED', 'REOPENED')
    ),
    actor_id TEXT,
    response_text TEXT,
    comment TEXT,
    occurred_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_review_note_events_note
    ON review_note_events(review_note_id, occurred_at_ms, rowid);

CREATE TRIGGER trg_review_note_events_no_update
BEFORE UPDATE ON review_note_events
BEGIN
    SELECT RAISE(ABORT, 'review note events are immutable');
END;

CREATE TRIGGER trg_review_note_events_no_delete
BEFORE DELETE ON review_note_events
BEGIN
    SELECT RAISE(ABORT, 'review note events are immutable');
END;
