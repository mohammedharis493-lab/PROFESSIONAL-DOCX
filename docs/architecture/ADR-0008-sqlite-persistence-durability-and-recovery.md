# ADR-0008: SQLite Persistence, Durability, Migration, Backup, and Recovery

- **Status:** Accepted
- **Date:** 2026-10-02
- **Project:** Professional DocX
- **Branch:** develop

## Context

Professional DocX Level-A desktop deployment uses SQLite as the authoritative local metadata database.

The database will eventually hold professional metadata that must survive:

- application crashes,
- operating-system crashes,
- power loss,
- interrupted upgrades,
- interrupted indexing,
- search-index corruption,
- application restarts.

It must also support:

- explicit schema migrations,
- foreign-key integrity,
- bounded concurrent access,
- backup/restore,
- startup validation,
- recovery from incomplete work.

Because Tantivy is a derived search projection, SQLite is the local source from which search state must be rebuildable.

The database itself must not become a fragile hidden implementation detail.

## Decision

Professional DocX will use a local SQLite database with explicit durability and migration rules.

The Level-A persistence contract is:

```text
Local application data directory
        |
        +-> metadata.sqlite
        |     authoritative metadata
        |
        +-> search-index/
        |     derived / rebuildable
        |
        +-> preview-cache/
        |     derived / rebuildable
        |
        +-> logs/
              diagnostic / operational
```

The authoritative SQLite database must reside on a local filesystem controlled by the Professional DocX application.

It must not be placed on a NAS, SMB share, synced cloud folder, or other multi-host shared location for concurrent access.

---

# 1. Authority

SQLite is authoritative for Level-A structured application state.

This includes, as implemented over time:

- approved storage roots,
- Documents,
- FileInstances,
- path history,
- ContentVersions,
- index jobs,
- scan generations,
- scan errors,
- search-index outbox,
- clients,
- engagements,
- areas,
- workpapers,
- revisions,
- evidence links,
- sign-offs,
- audit events,
- user-local preferences that require persistence.

Tantivy is not authoritative.

If SQLite and Tantivy disagree, SQLite wins.

---

# 2. Database Location

The database must live under the application-specific local data directory.

Conceptually:

```text
Professional DocX/
  data/
    metadata.sqlite
    metadata.sqlite-wal
    metadata.sqlite-shm
```

The exact operating-system path is resolved through the desktop platform/application-data API.

Do not place `metadata.sqlite` inside:

- a client source folder,
- a NAS share,
- a mapped network drive,
- OneDrive/Dropbox-style sync storage,
- a removable client-evidence directory.

The database is Professional DocX application state, not client source evidence.

---

# 3. Journal Mode

The database will use:

```sql
PRAGMA journal_mode = WAL;
```

WAL mode is chosen because it supports concurrent readers with a writer and provides robust crash recovery for the local desktop architecture.

The application must verify that the requested journal mode actually became `wal`.

If WAL cannot be enabled on the selected local database environment, startup must fail with a clear diagnostic rather than silently continuing under materially different durability/concurrency assumptions.

---

# 4. Synchronous Durability

The authoritative database will use:

```sql
PRAGMA synchronous = FULL;
```

Professional DocX prioritizes preservation of committed professional metadata over marginal write-performance gains.

In WAL mode, `FULL` adds synchronization at transaction commit and provides durability across operating-system crash or power loss.

This is deliberately more conservative than `NORMAL`.

Performance optimization may later be evaluated with benchmarks, but changing authoritative metadata to `NORMAL` requires an explicit ADR amendment.

---

# 5. Foreign Keys

Every application database connection must enable:

```sql
PRAGMA foreign_keys = ON;
```

Foreign-key enforcement is a connection-level setting and must not be assumed merely because schema definitions contain `REFERENCES`.

Connection initialization must verify:

```sql
PRAGMA foreign_keys;
```

returns enabled before the connection is used for application work.

Migrations must not rely on orphaned references being tolerated.

---

# 6. Busy Timeout and Contention

Every normal application connection will configure a bounded busy timeout.

Initial default:

```text
5,000 ms
```

The implementation may use the SQLite API or equivalent wrapper configuration rather than issuing a raw pragma.

The purpose is to tolerate brief writer contention without indefinite blocking.

A busy timeout is not a substitute for good transaction design.

Transactions must remain short and must not hold write locks while performing slow filesystem, network, OCR, parsing, or hashing work.

---

# 7. Connection Model

The Level-A application uses one local process as the database owner.

The first implementation should use:

- one serialized writer path,
- a small bounded number of read connections where useful,
- no assumption of multiple independent desktop processes writing the same database concurrently.

Long-running indexing workers send bounded persistence batches to the database layer.

They must not each open uncontrolled writers and compete independently.

SQLite's single-writer nature is treated as an architectural constraint, not an error condition to hide.

---

# 8. Transaction Boundaries

Authoritative state changes must be grouped into transactions according to one logical consistency unit.

Examples:

## Index batch

```text
BEGIN

persist filesystem observations
update generation/health state
append path/content changes
insert search_index_outbox operations

COMMIT
```

## Controlled evidence finalization

The metadata portion of a completed evidence capture is committed only after the controlled object has been durably created and verified according to ADR-0007.

## Sign-off

Creating a sign-off and any state transitions required for that exact workpaper revision must commit atomically.

External slow work must happen before or outside the write transaction wherever possible.

---

# 9. Search Outbox Consistency

ADR-0006 defines the search-index outbox.

The rule is reaffirmed here:

```text
SQLite authoritative mutation
+
search-index intent

must commit in the SAME SQLite transaction.
```

Tantivy is updated only after the SQLite transaction commits.

If the process crashes after SQLite commit but before Tantivy update, the pending outbox operation is replayed.

Do not implement cross-engine pseudo-transactions that try to commit Tantivy first.

---

# 10. Schema Migration System

Schema changes must use ordered, immutable migrations from the first production schema onward.

Conceptual layout:

```text
migrations/
  0001_initial.sql
  0002_add_x.sql
  0003_add_y.sql
```

Once released/applied, a migration file is immutable.

Corrections are introduced through a new migration.

Do not edit historical migration files after users may have applied them.

Each migration must be applied inside a transaction whenever SQLite permits the required operation transactionally.

---

# 11. Migration History

The database will contain a dedicated migration history table.

Conceptual fields:

```text
schema_migrations
  version
  name
  checksum
  applied_at
  application_version
```

The migration runner must:

1. read applied versions,
2. verify the checksum of known applied migrations,
3. reject unknown/future incompatible schema versions,
4. apply pending migrations in deterministic order,
5. record each successful migration only after completion.

`PRAGMA user_version` may mirror the latest integer schema version for diagnostics/compatibility checks, but the migration-history table is the richer authoritative migration record.

---

# 12. Migration Failure

Before a destructive or materially risky migration, the application must create or require a recoverable database backup.

If a migration fails:

- roll back the active migration transaction where possible,
- do not continue starting the normal application against a partially migrated schema,
- preserve diagnostic logs,
- offer recovery/retry from the last known valid database state.

A migration must never silently skip a failure and mark itself successful.

---

# 13. Application Compatibility

Startup must distinguish:

```text
database older than application
  -> apply known forward migrations

database same version
  -> continue

database newer than application
  -> refuse writable startup
     and explain that a newer application/schema created it
```

An older binary must not guess how to write to a newer schema.

---

# 14. Backup

A live SQLite database must not be backed up by naively copying only `metadata.sqlite` while the application is using WAL mode.

Professional DocX will use an SQLite-consistent backup mechanism such as:

- SQLite Online Backup API, or
- `VACUUM INTO` where its semantics fit the operation.

A backup is treated as an application-created consistent snapshot.

The implementation must ensure the destination backup is complete before presenting it as valid.

---

# 15. What Must Be Backed Up

Backup categories are:

## Authoritative / required

```text
metadata.sqlite snapshot
managed files
controlled evidence
audit-log authoritative data
application configuration required to interpret managed storage
```

## Rebuildable / optional

```text
Tantivy search index
preview cache
OCR cache
thumbnails
temporary extraction files
```

A restore must not depend on preservation of the search index.

After restore, derived state may be rebuilt.

---

# 16. Controlled Evidence Backup

Controlled evidence bytes are not stored solely inside SQLite.

A complete Professional DocX backup therefore eventually includes:

```text
consistent SQLite metadata snapshot
+
managed-file store
+
controlled-evidence store
+
manifest describing backup contents/version
```

A database-only backup is not presented as a complete engagement/evidence backup once managed/controlled storage exists.

---

# 17. Backup Verification

Backups should be verified before being considered usable.

Minimum metadata-backup verification includes:

- backup file exists,
- backup completed successfully,
- SQLite can open the snapshot,
- `PRAGMA quick_check` returns `ok`,
- `PRAGMA foreign_key_check` returns no violations.

Higher-assurance/manual diagnostics may run `PRAGMA integrity_check`.

Controlled evidence verification later includes validating stored SHA-256 values against preserved evidence bytes.

---

# 18. Startup Validation

Normal startup should perform lightweight validation sufficient to detect obvious database problems without making every launch unnecessarily expensive.

Initial approach:

1. open database,
2. apply/verify connection settings,
3. inspect schema compatibility,
4. detect interrupted migrations/jobs,
5. run lightweight integrity validation when policy indicates it is due or after unclean shutdown,
6. process recovery state before normal work.

A full `PRAGMA integrity_check` need not run on every normal startup for a large database.

`PRAGMA quick_check` may be used for scheduled or recovery-oriented validation.

Foreign-key validation is separate because `integrity_check` does not replace `foreign_key_check`.

---

# 19. Corruption Handling

If SQLite reports corruption or validation fails:

```text
DO NOT
  continue normal writes
  rebuild SQLite from Tantivy
  silently create a fresh empty database over the old one
```

Instead:

```text
1. stop normal writable operation
2. preserve the suspect database files
3. record diagnostic information
4. offer restore from verified backup
5. if necessary, use explicit recovery tooling as a separate operation
6. rebuild Tantivy only after authoritative metadata is recovered
```

The search index must never be treated as an authoritative recovery source.

---

# 20. Interrupted Jobs

On startup, durable job records left in transient states such as `RUNNING` are reconciled according to ADR-0006.

For example:

```text
RUNNING scan after process crash
       |
       v
mark INTERRUPTED
       |
       v
do not infer missing-file deletions
```

Database recovery and indexing recovery are related but separate concerns.

---

# 21. Database File Security

The database may contain confidential metadata such as:

- client names,
- filenames,
- document metadata,
- workpaper text,
- review information,
- extracted/searchable text references.

The application must therefore:

- store it in the user's protected application-data directory,
- rely on appropriate OS account/file permissions,
- avoid world-readable locations,
- avoid logging database secrets/content unnecessarily.

SQLite itself does not provide transparent at-rest encryption by default.

Application-level/database encryption is a separate explicit security decision and must not be claimed as implemented until a supported encryption approach is selected and tested.

Deployment policy may require operating-system disk encryption.

---

# 22. Database and Source Files Are Separate

Deleting or recreating derived Professional DocX metadata must never delete linked client source files.

Conversely, deleting a linked source file must not implicitly delete authoritative Professional DocX history.

Storage lifecycle rules from ADR-0001 and ADR-0005 continue to apply.

---

# 23. Test Database Strategy

Automated persistence tests use isolated temporary database files.

Tests must not reuse a developer's normal Professional DocX database.

Migration tests must include:

- fresh database -> latest schema,
- each supported prior schema -> latest schema,
- failed migration rollback,
- newer/unknown schema rejection,
- foreign-key enforcement,
- outbox transactional consistency,
- interrupted-job recovery,
- backup + restore validation.

---

# 24. CI Requirements

Once SQLite persistence code is introduced, CI must test database behavior on at least:

- Linux,
- Windows.

Tests should run with the same migration set embedded/shipped with the application.

CI must fail if:

- migrations cannot apply to a fresh database,
- migration checksums unexpectedly change,
- foreign-key tests fail,
- Rust persistence tests fail,
- locked dependencies drift.

---

# 25. Initial Connection Profile

The initial authoritative connection profile is conceptually:

```sql
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
```

plus:

```text
busy timeout = 5 seconds
bounded writer access
local application-data filesystem only
```

The implementation must verify critical settings rather than merely issuing them and assuming success.

---

# 26. Deferred Decisions

This ADR does not yet select:

- the exact Rust SQLite crate,
- the exact migration crate/library,
- application-level SQLite encryption technology,
- the final backup scheduler/retention UI,
- PostgreSQL schema for shared deployment.

Those choices may be made without changing this persistence contract.

---

# 27. Acceptance Tests

Before Level-A SQLite persistence is considered stable:

1. Database is created only in the local Professional DocX application-data location.
2. WAL mode is enabled and verified.
3. `synchronous=FULL` is configured for authoritative connections.
4. Foreign keys are enabled and verified on every connection.
5. Brief write contention respects a bounded busy timeout.
6. A failed transaction does not leave partial authoritative state.
7. Search outbox writes commit atomically with metadata mutations.
8. A pending outbox entry survives process restart.
9. Fresh database migrates from version 0 to latest.
10. Applied migration checksums are verified.
11. A failed migration does not mark itself successful.
12. An older application refuses to write a newer database schema.
13. A live backup produces a consistent openable snapshot.
14. Backup validation includes quick/integrity and foreign-key checks as appropriate.
15. Tantivy can be deleted and rebuilt from SQLite after restore.
16. Corrupt SQLite is never reconstructed from Tantivy as though Tantivy were authoritative.
17. Interrupted indexing jobs recover according to ADR-0006.
18. SQLite database is never used as a shared multi-host NAS database.
19. Linux and Windows CI both exercise the persistence layer.

## Consequences

### Positive

- Persistence semantics are explicit before schema code exists.
- Committed professional metadata prioritizes durability.
- Schema evolution becomes auditable and repeatable.
- SQLite/Tantivy authority is unambiguous.
- Backup and corruption recovery have defined behavior.
- Future PostgreSQL migration is easier because persistence behavior is hidden behind repository/service boundaries.

### Trade-offs

- `synchronous=FULL` may be slower than `NORMAL`.
- Migration/checksum infrastructure adds implementation work.
- Consistent backups require SQLite-aware APIs rather than raw file copies.
- A single local SQLite writer constrains concurrency by design.
- Full shared/multi-user operation still requires the later firm-service architecture.

## Official References

- SQLite WAL: https://www.sqlite.org/wal.html
- SQLite PRAGMAs: https://www.sqlite.org/pragma.html
- SQLite Backup API: https://www.sqlite.org/backup.html
- SQLite VACUUM INTO: https://www.sqlite.org/lang_vacuum.html
- SQLite security guidance: https://www.sqlite.org/security.html

## Related ADRs

- ADR-0002 — Desktop-First Application Boundary
- ADR-0004 — Initial Implementation Stack
- ADR-0005 — File Identity, Source Instances, Paths, and Content Versions
- ADR-0006 — Indexing Jobs, Scan Generations, Reconciliation, and Search Consistency
- ADR-0007 — Controlled Evidence Capture, Workpaper Revisions, and Sign-Off Binding
