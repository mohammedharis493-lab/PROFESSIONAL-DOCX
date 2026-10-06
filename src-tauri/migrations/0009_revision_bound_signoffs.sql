CREATE TABLE signoffs (
    signoff_id TEXT PRIMARY KEY NOT NULL,
    workpaper_id TEXT NOT NULL REFERENCES workpapers(workpaper_id),
    workpaper_revision_id TEXT NOT NULL REFERENCES workpaper_revisions(workpaper_revision_id),
    signoff_type TEXT NOT NULL CHECK (length(trim(signoff_type)) > 0),
    actor_id TEXT NOT NULL CHECK (length(trim(actor_id)) > 0),
    actor_role TEXT NOT NULL CHECK (length(trim(actor_role)) > 0),
    signed_at_ms INTEGER NOT NULL,
    comment TEXT
);

CREATE INDEX idx_signoffs_workpaper_revision
    ON signoffs(workpaper_id, workpaper_revision_id, signed_at_ms);

CREATE INDEX idx_signoffs_type
    ON signoffs(workpaper_revision_id, signoff_type, signed_at_ms);

CREATE TABLE signoff_evidence_links (
    signoff_evidence_link_id TEXT PRIMARY KEY NOT NULL,
    signoff_id TEXT NOT NULL REFERENCES signoffs(signoff_id),
    evidence_link_id TEXT NOT NULL REFERENCES workpaper_evidence_links(evidence_link_id),
    created_at_ms INTEGER NOT NULL,
    UNIQUE (signoff_id, evidence_link_id)
);

CREATE INDEX idx_signoff_evidence_signoff
    ON signoff_evidence_links(signoff_id, created_at_ms);

CREATE TABLE signoff_supersessions (
    signoff_supersession_id TEXT PRIMARY KEY NOT NULL,
    signoff_id TEXT NOT NULL UNIQUE REFERENCES signoffs(signoff_id),
    superseded_at_ms INTEGER NOT NULL,
    superseded_reason TEXT NOT NULL CHECK (length(trim(superseded_reason)) > 0)
);

CREATE INDEX idx_signoff_supersessions_signoff
    ON signoff_supersessions(signoff_id, superseded_at_ms);

CREATE TRIGGER trg_signoffs_no_update
BEFORE UPDATE ON signoffs
BEGIN
    SELECT RAISE(ABORT, 'signoffs are immutable');
END;

CREATE TRIGGER trg_signoffs_no_delete
BEFORE DELETE ON signoffs
BEGIN
    SELECT RAISE(ABORT, 'signoffs are immutable');
END;

CREATE TRIGGER trg_signoff_evidence_no_update
BEFORE UPDATE ON signoff_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'signoff evidence snapshots are immutable');
END;

CREATE TRIGGER trg_signoff_evidence_no_delete
BEFORE DELETE ON signoff_evidence_links
BEGIN
    SELECT RAISE(ABORT, 'signoff evidence snapshots are immutable');
END;

CREATE TRIGGER trg_signoff_supersessions_no_update
BEFORE UPDATE ON signoff_supersessions
BEGIN
    SELECT RAISE(ABORT, 'signoff supersessions are immutable');
END;

CREATE TRIGGER trg_signoff_supersessions_no_delete
BEFORE DELETE ON signoff_supersessions
BEGIN
    SELECT RAISE(ABORT, 'signoff supersessions are immutable');
END;
