CREATE TABLE workpaper_signoffs (
    signoff_id TEXT PRIMARY KEY NOT NULL,
    workpaper_id TEXT NOT NULL REFERENCES workpapers(workpaper_id),
    workpaper_revision_id TEXT NOT NULL REFERENCES workpaper_revisions(workpaper_revision_id),
    signoff_type TEXT NOT NULL CHECK (length(trim(signoff_type)) > 0),
    actor_id TEXT NOT NULL CHECK (length(trim(actor_id)) > 0),
    actor_role TEXT NOT NULL CHECK (length(trim(actor_role)) > 0),
    signed_at_ms INTEGER NOT NULL,
    comment TEXT
);

CREATE INDEX idx_workpaper_signoffs_workpaper_revision
    ON workpaper_signoffs(workpaper_id, workpaper_revision_id, signed_at_ms);

CREATE TABLE workpaper_signoff_evidence (
    signoff_evidence_id TEXT PRIMARY KEY NOT NULL,
    signoff_id TEXT NOT NULL REFERENCES workpaper_signoffs(signoff_id),
    evidence_link_id TEXT NOT NULL REFERENCES workpaper_evidence_links(evidence_link_id),
    created_at_ms INTEGER NOT NULL,
    UNIQUE (signoff_id, evidence_link_id)
);

CREATE INDEX idx_workpaper_signoff_evidence_signoff
    ON workpaper_signoff_evidence(signoff_id, created_at_ms);

CREATE TABLE workpaper_signoff_supersessions (
    signoff_supersession_id TEXT PRIMARY KEY NOT NULL,
    signoff_id TEXT NOT NULL UNIQUE REFERENCES workpaper_signoffs(signoff_id),
    superseded_by_revision_id TEXT NOT NULL REFERENCES workpaper_revisions(workpaper_revision_id),
    superseded_at_ms INTEGER NOT NULL,
    superseded_reason TEXT NOT NULL CHECK (length(trim(superseded_reason)) > 0),
    actor_id TEXT
);

CREATE INDEX idx_workpaper_signoff_supersessions_revision
    ON workpaper_signoff_supersessions(superseded_by_revision_id, superseded_at_ms);

CREATE TRIGGER trg_workpaper_signoffs_no_update
BEFORE UPDATE ON workpaper_signoffs
BEGIN
    SELECT RAISE(ABORT, 'workpaper sign-offs are immutable');
END;

CREATE TRIGGER trg_workpaper_signoffs_no_delete
BEFORE DELETE ON workpaper_signoffs
BEGIN
    SELECT RAISE(ABORT, 'workpaper sign-offs are immutable');
END;

CREATE TRIGGER trg_workpaper_signoff_evidence_no_update
BEFORE UPDATE ON workpaper_signoff_evidence
BEGIN
    SELECT RAISE(ABORT, 'workpaper sign-off evidence snapshots are immutable');
END;

CREATE TRIGGER trg_workpaper_signoff_evidence_no_delete
BEFORE DELETE ON workpaper_signoff_evidence
BEGIN
    SELECT RAISE(ABORT, 'workpaper sign-off evidence snapshots are immutable');
END;

CREATE TRIGGER trg_workpaper_signoff_supersessions_no_update
BEFORE UPDATE ON workpaper_signoff_supersessions
BEGIN
    SELECT RAISE(ABORT, 'workpaper sign-off supersessions are immutable');
END;

CREATE TRIGGER trg_workpaper_signoff_supersessions_no_delete
BEFORE DELETE ON workpaper_signoff_supersessions
BEGIN
    SELECT RAISE(ABORT, 'workpaper sign-off supersessions are immutable');
END;
