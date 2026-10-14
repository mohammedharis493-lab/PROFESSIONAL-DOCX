# ADR-0006: Indexing Jobs, Scan Generations, Reconciliation, and Search Consistency

- **Status:** Accepted
- **Date:** 2026-10-02
- **Project:** Professional DocX
- **Branch:** develop

## Context

Professional DocX must index existing local and network file trees without copying the source files.

The current development scanner proves basic enumeration, but a production-grade indexer cannot assume that:

- a root is always online,
- every directory can be read,
- a long scan will finish,
- filesystem watcher events are complete,
- a process will not crash between metadata persistence and search indexing,
- "not seen" always means "deleted".

The system must work predictably for small local folders and remain structurally sound for large trees, slow NAS devices, VPN-dependent shares, and future million-file catalogues.

This ADR defines the indexing lifecycle before SQLite persistence is implemented.

## Decision

Professional DocX will use a **job-based, generation-based indexing architecture**.

The high-level pipeline is:

```text
ApprovedStorageRoot
        |
        v
Create IndexJob + ScanGeneration
        |
        v
Background filesystem worker
        |
        v
Stream observations in bounded batches
        |
        v
SQLite authoritative metadata transaction
        |
        +--> file/document/source updates
        |
        +--> search_index_outbox
        |
        v
COMMIT
        |
        v
Tantivy worker applies outbox
        |
        v
Tantivy commit
        |
        v
Outbox acknowledged
```

SQLite is the authoritative local metadata source.

Tantivy is disposable, derived, and fully rebuildable from SQLite.

---

# 1. IndexJob

An `IndexJob` represents a unit of indexing work.

Examples:

- initial root scan,
- full reconciliation,
- targeted watcher reconciliation,
- explicit rescan,
- search-index rebuild,
- metadata refresh.

Conceptual fields:

```text
index_job_id              UUID
storage_root_id
job_type
status
requested_at
started_at                nullable
completed_at              nullable
cancel_requested_at       nullable
last_heartbeat_at          nullable
current_phase
directories_seen
files_seen
bytes_seen
files_persisted
errors_count
scan_generation_id        nullable
failure_code              nullable
failure_message           nullable
```

Suggested job statuses:

```text
QUEUED
RUNNING
COMPLETE
PARTIAL
CANCELLED
OFFLINE
FAILED
INTERRUPTED
```

`INTERRUPTED` is used when the application restarts and discovers a previously running job that never completed.

---

# 2. ScanGeneration

Every authoritative filesystem enumeration creates a `ScanGeneration`.

Conceptual fields:

```text
scan_generation_id        UUID
storage_root_id
generation_number
started_at
completed_at              nullable
status
is_authoritative
directories_seen
files_seen
errors_count
```

Files observed during a generation record:

```text
last_seen_generation_id
last_seen_at
```

The key reconciliation rule is:

> Only a COMPLETE authoritative scan may infer that previously indexed files not observed in that generation are now missing.

Therefore:

```text
COMPLETE
  -> unseen prior records may be marked MISSING

PARTIAL
CANCELLED
OFFLINE
FAILED
INTERRUPTED
  -> unseen prior records MUST NOT be marked missing
```

This prevents a temporary NAS outage or permission problem from looking like mass deletion.

---

# 3. Background Execution

Large filesystem traversal must not run as one long synchronous UI-facing command.

Tauri commands should be control-plane operations such as:

```text
start_index_job(storage_root_id)
get_index_job(job_id)
cancel_index_job(job_id)
```

`start_index_job` should return a `job_id` promptly.

The actual blocking filesystem enumeration runs in a dedicated bounded worker/thread pool.

The UI may receive lightweight progress events, but persisted job state remains authoritative.

The implementation must avoid:

- accumulating the entire file tree in memory,
- creating one unbounded async task per file,
- one database transaction per discovered file,
- blocking the desktop UI while a slow network tree is walked.

---

# 4. Batching

Filesystem observations are persisted in bounded batches.

The first implementation should use a configurable batch size in the range of hundreds or low thousands of records and tune it through benchmarks.

Each batch transaction may include:

- storage-root health updates,
- file-instance observations,
- path-history changes,
- content-version metadata where applicable,
- scan-generation last-seen markers,
- structured scan errors,
- search-index outbox operations.

Batch size is an implementation parameter, not a domain invariant.

---

# 5. Cancellation

Cancellation is cooperative.

When cancellation is requested:

1. Persist `cancel_requested_at`.
2. Signal the worker cancellation token.
3. Stop beginning new expensive work.
4. Safely finish or roll back the active persistence batch.
5. Mark the job and generation `CANCELLED`.
6. Do not perform missing-file reconciliation.

Already committed observations may remain valid.

Cancellation must not require rolling back the entire scan.

---

# 6. Structured Scan Errors

A scan must not reduce all failures to one `skipped_entries` counter.

Structured error categories include at least:

```text
PERMISSION_DENIED
SOURCE_OFFLINE
PATH_NOT_FOUND
METADATA_FAILED
PATH_TOO_LONG
REPARSE_SKIPPED
CLOUD_PLACEHOLDER_UNAVAILABLE
LOCKED
OTHER_IO
```

Conceptual fields:

```text
scan_error_id
index_job_id
scan_generation_id
storage_root_id
relative_path_native        nullable
category
os_error_code               nullable
message
observed_at
```

Errors may be summarized for the UI while preserving enough detail for diagnostics and reconciliation.

---

# 7. Root Health

Each approved storage root has a health state separate from scan status.

Suggested states:

```text
AVAILABLE
DEGRADED
OFFLINE
NEEDS_RESCAN
UNKNOWN
```

When a root becomes unavailable:

- existing indexed metadata remains searchable,
- results clearly indicate that the original source cannot currently be opened,
- documents are not deleted,
- missing-file reconciliation is not performed.

When the root becomes available again, Professional DocX schedules reconciliation.

---

# 8. Watcher Model

Filesystem watchers are **acceleration signals**, not an authoritative transaction log.

Watcher events should:

```text
watch event
    |
    v
coalesce / debounce
    |
    v
enqueue targeted reconciliation
```

If the watcher reports overflow, loses continuity, or encounters an unrecoverable error:

```text
root -> NEEDS_RESCAN
        |
        v
schedule authoritative reconciliation
```

Periodic reconciliation should exist even when watchers appear healthy.

Unknown Windows reparse-point directories are not traversed by default.

Watcher and reparse-point implementation details may evolve without changing this contract.

---

# 9. SQLite Is Authoritative

For the Level-A local desktop architecture:

> SQLite is the sole authoritative metadata database.

Authoritative state includes:

- approved storage roots,
- logical Documents,
- FileInstances,
- path history,
- observed ContentVersions,
- index jobs,
- scan generations,
- root health,
- structured scan errors,
- client/engagement metadata,
- later audit/workpaper metadata.

The application must remain able to function at the metadata level even if the Tantivy index is absent or corrupt.

---

# 10. Tantivy Is Derived State

Tantivy is a performance/search projection.

It is not an independent source of truth.

Every Tantivy record must contain stable application identifiers, especially:

```text
document_id
```

and, where needed:

```text
file_instance_id
content_version_id
storage_root_id
```

Search results are hydrated from SQLite before being returned as authoritative application objects.

If a Tantivy hit references an object SQLite no longer recognizes, that hit is stale and must not be trusted.

---

# 11. Transactional Search Outbox

SQLite metadata changes and search-index intentions are committed together.

Conceptually:

```text
BEGIN SQLITE TRANSACTION

  upsert authoritative metadata

  insert search_index_outbox(
      operation_id,
      entity_type,
      entity_id,
      operation,
      payload_version,
      created_at
  )

COMMIT
```

A separate search worker then:

1. reads pending outbox operations,
2. updates Tantivy,
3. commits the Tantivy index,
4. marks the outbox operation acknowledged/completed.

This handles the failure case:

```text
SQLite commit succeeds
process crashes before Tantivy update
```

because the pending outbox item remains available for replay.

The reverse order—committing Tantivy before authoritative SQLite metadata—is prohibited for ordinary metadata mutations.

---

# 12. Search Rebuild

Tantivy must be fully rebuildable.

A rebuild uses SQLite as source:

```text
SQLite authoritative metadata
        |
        v
build new search generation
        |
        v
validate
        |
        v
atomically switch active generation
        |
        v
retire old generation later
```

Suggested directory pattern:

```text
search-index-v1/
search-index-v2-building/
```

The exact naming is implementation-specific.

If the active index is corrupt, the recovery path is:

```text
stop using corrupt index
        |
        v
rebuild from SQLite
        |
        v
activate validated new index
```

No authoritative document/evidence metadata should be lost merely because the search index is deleted.

---

# 13. Crash Recovery

On startup:

1. inspect jobs left in `RUNNING`,
2. mark them `INTERRUPTED`,
3. do not infer deletions from their incomplete generations,
4. replay pending search-index outbox operations,
5. validate root health as appropriate,
6. allow interrupted scans to restart or be explicitly resumed/requeued.

A scan heartbeat may help distinguish stale jobs from active ones in future multi-process designs.

---

# 14. File Changes During Observation

A file can change while it is being read.

For initial discovery, metadata observation is allowed to be eventually consistent.

For operations requiring exact content identity—especially controlled evidence capture—the application must verify stability before finalizing the version.

Controlled-evidence atomicity is defined in ADR-0007.

Initial discovery must not eagerly SHA-256 every file merely to catalogue it.

---

# 15. Reconciliation Examples

## Root disconnects halfway through

```text
generation 42
  80,000 files observed
  NAS disconnects
  status = OFFLINE or PARTIAL
```

Result:

- keep the 80,000 observations,
- keep all previously indexed unseen records,
- do not mark them missing,
- mark root degraded/offline,
- reconcile after recovery.

## Complete scan does not see prior file

```text
generation 43 = COMPLETE
old file last seen in generation 42
not observed in 43
```

Result:

- mark its source instance `MISSING`,
- do not automatically delete the logical Document,
- retain path/history/provenance.

## Watcher reports rename

Result:

- enqueue targeted reconciliation,
- use ADR-0005 continuity rules,
- preserve FileInstance when identity confirms continuity,
- append path history.

---

# 16. Current Prototype Scanner

The current synchronous `scan_storage_root` implementation is a temporary development proof only.

It must be replaced before filesystem persistence is considered production architecture.

No SQLite schema should be designed around the assumption that a scan returns one complete in-memory `FolderScan` object.

The target contract is job-based streaming persistence.

---

# 17. Persistence Schema Consequences

The first SQLite migration must include or reserve explicit structures for:

```text
storage_roots
index_jobs
scan_generations
scan_errors
search_index_outbox
```

alongside the identity entities from ADR-0005.

The schema must support future migrations from version 1 onward.

Database migration, durability, connection settings, and backup rules are addressed during the persistence remediation step.

---

# 18. Acceptance Tests

Before indexing persistence is considered stable, tests must cover:

1. Starting a scan returns a job ID without waiting for full traversal.
2. Progress can be queried while the job is running.
3. A cancelled scan never marks unseen files missing.
4. A partial scan never marks unseen files missing.
5. An offline root never causes mass deletion.
6. Only a COMPLETE authoritative generation may reconcile unseen files to MISSING.
7. Process interruption leaves the generation non-authoritative for deletion reconciliation.
8. SQLite commit without Tantivy update is repaired by outbox replay.
9. Tantivy can be deleted and rebuilt entirely from SQLite.
10. A stale Tantivy hit that cannot hydrate from SQLite is discarded.
11. A watcher overflow/error schedules authoritative reconciliation.
12. Scanning streams/batches records rather than retaining an entire million-file catalogue in RAM.
13. Initial discovery does not require SHA-256 of every source file.

## Consequences

### Positive

- Slow or offline NAS paths cannot masquerade as mass deletion.
- UI responsiveness is decoupled from filesystem traversal.
- Long-running work gains progress and cancellation.
- SQLite/Tantivy crash consistency has a clear authority model.
- Search indexes become safely disposable/rebuildable.
- The architecture scales conceptually from small roots toward very large catalogues.

### Trade-offs

- Indexing requires durable job state and worker coordination.
- Reconciliation becomes more explicit than a simple recursive scan.
- Search indexing is eventually consistent with SQLite rather than one atomic cross-engine transaction.
- More recovery states must be tested.
- Watchers require periodic reconciliation instead of being trusted absolutely.

## Related ADRs

- ADR-0002 — Desktop-First Application Boundary
- ADR-0004 — Initial Implementation Stack
- ADR-0005 — File Identity, Source Instances, Paths, and Content Versions
