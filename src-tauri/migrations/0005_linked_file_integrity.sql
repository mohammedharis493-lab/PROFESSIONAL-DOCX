ALTER TABLE file_instances
ADD COLUMN integrity_state TEXT NOT NULL DEFAULT 'UNCHANGED'
CHECK (integrity_state IN ('UNCHANGED', 'PATH_CHANGED', 'CONTENT_CHANGED', 'REPLACED'));

ALTER TABLE file_instances
ADD COLUMN integrity_changed_at_ms INTEGER;

ALTER TABLE file_instances
ADD COLUMN integrity_acknowledged_at_ms INTEGER;

CREATE INDEX idx_file_instances_integrity_state
    ON file_instances(integrity_state, integrity_acknowledged_at_ms);
