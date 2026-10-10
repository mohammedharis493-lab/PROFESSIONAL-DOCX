CREATE TABLE storage_roots (
    storage_root_id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('LOCAL', 'NETWORK', 'REMOVABLE', 'OTHER')),
    native_locator BLOB NOT NULL,
    native_locator_encoding TEXT NOT NULL,
    display_locator TEXT NOT NULL,
    canonical_native_locator BLOB,
    canonical_display_locator TEXT,
    availability_state TEXT NOT NULL CHECK (
        availability_state IN ('AVAILABLE', 'DEGRADED', 'OFFLINE', 'NEEDS_RESCAN', 'UNKNOWN')
    ),
    approved_at_ms INTEGER NOT NULL,
    approved_by TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE TABLE documents (
    document_id TEXT PRIMARY KEY NOT NULL,
    storage_state TEXT NOT NULL CHECK (
        storage_state IN ('LINKED', 'CONTROLLED_EVIDENCE', 'MANAGED')
    ),
    display_name TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    created_by TEXT,
    archived_at_ms INTEGER
);

CREATE TABLE scan_generations (
    scan_generation_id TEXT PRIMARY KEY NOT NULL,
    storage_root_id TEXT NOT NULL REFERENCES storage_roots(storage_root_id),
    generation_number INTEGER NOT NULL CHECK (generation_number >= 1),
    started_at_ms INTEGER NOT NULL,
    completed_at_ms INTEGER,
    status TEXT NOT NULL CHECK (
        status IN ('QUEUED', 'RUNNING', 'COMPLETE', 'PARTIAL', 'CANCELLED', 'OFFLINE', 'FAILED', 'INTERRUPTED')
    ),
    is_authoritative INTEGER NOT NULL DEFAULT 0 CHECK (is_authoritative IN (0, 1)),
    directories_seen INTEGER NOT NULL DEFAULT 0 CHECK (directories_seen >= 0),
    files_seen INTEGER NOT NULL DEFAULT 0 CHECK (files_seen >= 0),
    errors_count INTEGER NOT NULL DEFAULT 0 CHECK (errors_count >= 0),
    UNIQUE (storage_root_id, generation_number)
);

CREATE TABLE index_jobs (
    index_job_id TEXT PRIMARY KEY NOT NULL,
    storage_root_id TEXT NOT NULL REFERENCES storage_roots(storage_root_id),
    job_type TEXT NOT NULL CHECK (
        job_type IN ('INITIAL_SCAN', 'FULL_RECONCILIATION', 'TARGETED_RECONCILIATION', 'EXPLICIT_RESCAN', 'SEARCH_REBUILD', 'METADATA_REFRESH')
    ),
    status TEXT NOT NULL CHECK (
        status IN ('QUEUED', 'RUNNING', 'COMPLETE', 'PARTIAL', 'CANCELLED', 'OFFLINE', 'FAILED', 'INTERRUPTED')
    ),
    requested_at_ms INTEGER NOT NULL,
    started_at_ms INTEGER,
    completed_at_ms INTEGER,
    cancel_requested_at_ms INTEGER,
    last_heartbeat_at_ms INTEGER,
    current_phase TEXT,
    directories_seen INTEGER NOT NULL DEFAULT 0 CHECK (directories_seen >= 0),
    files_seen INTEGER NOT NULL DEFAULT 0 CHECK (files_seen >= 0),
    bytes_seen INTEGER NOT NULL DEFAULT 0 CHECK (bytes_seen >= 0),
    files_persisted INTEGER NOT NULL DEFAULT 0 CHECK (files_persisted >= 0),
    errors_count INTEGER NOT NULL DEFAULT 0 CHECK (errors_count >= 0),
    scan_generation_id TEXT REFERENCES scan_generations(scan_generation_id),
    failure_code TEXT,
    failure_message TEXT
);

CREATE TABLE file_instances (
    file_instance_id TEXT PRIMARY KEY NOT NULL,
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    storage_root_id TEXT NOT NULL REFERENCES storage_roots(storage_root_id),
    relative_path_native BLOB NOT NULL,
    path_native_encoding TEXT NOT NULL,
    relative_path_display TEXT NOT NULL,
    relative_path_search TEXT NOT NULL,
    filesystem_identity BLOB,
    volume_identity BLOB,
    creation_time_ms INTEGER,
    last_write_time_ms INTEGER,
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    file_attributes INTEGER,
    reparse_tag INTEGER,
    first_seen_at_ms INTEGER NOT NULL,
    last_seen_at_ms INTEGER NOT NULL,
    first_seen_generation_id TEXT REFERENCES scan_generations(scan_generation_id),
    last_seen_generation_id TEXT REFERENCES scan_generations(scan_generation_id),
    availability_state TEXT NOT NULL CHECK (
        availability_state IN ('AVAILABLE', 'MISSING', 'CHANGED', 'UNAVAILABLE', 'UNKNOWN')
    )
);

CREATE TABLE file_path_history (
    file_path_history_id TEXT PRIMARY KEY NOT NULL,
    file_instance_id TEXT NOT NULL REFERENCES file_instances(file_instance_id),
    storage_root_id TEXT NOT NULL REFERENCES storage_roots(storage_root_id),
    relative_path_native BLOB NOT NULL,
    path_native_encoding TEXT NOT NULL,
    relative_path_display TEXT NOT NULL,
    observed_from_ms INTEGER NOT NULL,
    observed_until_ms INTEGER,
    change_reason TEXT NOT NULL CHECK (
        change_reason IN ('DISCOVERED', 'RENAMED', 'MOVED', 'RELINKED', 'OTHER')
    ),
    actor_id TEXT,
    scan_generation_id TEXT REFERENCES scan_generations(scan_generation_id)
);

CREATE TABLE content_versions (
    content_version_id TEXT PRIMARY KEY NOT NULL,
    document_id TEXT NOT NULL REFERENCES documents(document_id),
    file_instance_id TEXT NOT NULL REFERENCES file_instances(file_instance_id),
    observed_at_ms INTEGER NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    last_write_time_ms INTEGER,
    quick_fingerprint BLOB,
    sha256 BLOB CHECK (sha256 IS NULL OR length(sha256) = 32),
    verification_state TEXT NOT NULL CHECK (
        verification_state IN ('UNVERIFIED', 'METADATA_ONLY', 'FINGERPRINTED', 'HASH_VERIFIED', 'FAILED')
    ),
    source_stable_during_read INTEGER CHECK (
        source_stable_during_read IS NULL OR source_stable_during_read IN (0, 1)
    )
);

CREATE TABLE scan_errors (
    scan_error_id TEXT PRIMARY KEY NOT NULL,
    index_job_id TEXT NOT NULL REFERENCES index_jobs(index_job_id),
    scan_generation_id TEXT REFERENCES scan_generations(scan_generation_id),
    storage_root_id TEXT NOT NULL REFERENCES storage_roots(storage_root_id),
    relative_path_native BLOB,
    path_native_encoding TEXT,
    relative_path_display TEXT,
    category TEXT NOT NULL CHECK (
        category IN (
            'PERMISSION_DENIED',
            'SOURCE_OFFLINE',
            'PATH_NOT_FOUND',
            'METADATA_FAILED',
            'PATH_TOO_LONG',
            'REPARSE_SKIPPED',
            'CLOUD_PLACEHOLDER_UNAVAILABLE',
            'LOCKED',
            'OTHER_IO'
        )
    ),
    os_error_code INTEGER,
    message TEXT NOT NULL,
    observed_at_ms INTEGER NOT NULL
);

CREATE TABLE search_index_outbox (
    operation_id TEXT PRIMARY KEY NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    operation TEXT NOT NULL CHECK (operation IN ('UPSERT', 'DELETE')),
    payload_version INTEGER NOT NULL CHECK (payload_version >= 1),
    payload_json TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    acknowledged_at_ms INTEGER,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    last_error TEXT
);

CREATE INDEX idx_storage_roots_availability
    ON storage_roots(availability_state);

CREATE INDEX idx_scan_generations_root_status
    ON scan_generations(storage_root_id, status);

CREATE INDEX idx_index_jobs_root_status
    ON index_jobs(storage_root_id, status);

CREATE INDEX idx_file_instances_document
    ON file_instances(document_id);

CREATE INDEX idx_file_instances_root_last_seen
    ON file_instances(storage_root_id, last_seen_generation_id);

CREATE INDEX idx_file_instances_search_path
    ON file_instances(storage_root_id, relative_path_search);

CREATE INDEX idx_file_path_history_instance
    ON file_path_history(file_instance_id, observed_from_ms);

CREATE INDEX idx_content_versions_document_observed
    ON content_versions(document_id, observed_at_ms);

CREATE INDEX idx_content_versions_file_instance
    ON content_versions(file_instance_id, observed_at_ms);

CREATE INDEX idx_scan_errors_job
    ON scan_errors(index_job_id, observed_at_ms);

CREATE INDEX idx_search_index_outbox_pending
    ON search_index_outbox(acknowledged_at_ms, created_at_ms);
