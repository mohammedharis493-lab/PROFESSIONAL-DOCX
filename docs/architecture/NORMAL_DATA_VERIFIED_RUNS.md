# Normal Data — Verified CSV comparison runs (first adapter)

The native Tauri run command accepts only a comparison recipe-version UUID. It resolves both immutable dataset versions, verifies the source bytes against their stored SHA-256 values using existing approved-root native path validation, and applies the deterministic comparison core.

## Explicit source contract

- Both dataset versions must be stable and HASH_VERIFIED; metadata and fingerprints alone never authorize a run.
- UTF-8 CSV only (optional BOM, quoted values, LF or CRLF). Other formats are rejected.
- Every dataset must declare exactly one TEXT BUSINESS_KEY, the selected period column and every chosen DECIMAL NUMERIC_VALUE column.
- Filing and accounting periods are YYYY-MM; the invoice-month basis requires valid YYYY-MM-DD input dates and groups explicitly by invoice month.
- **All numeric CSV cells must already be signed integer minor units.** There is no automatic decimal or currency-scale inference, rounding, or conversion. Both sides must share the same normalized scale; e.g. 10025 means 10,025 minor units, not an assumed major-currency value.
- Business keys match exactly. Duplicate keys are surfaced, not arbitrarily paired.

Each read is limited to 32 MiB and each side to 100,000 rows. Result JSON has an 8 MiB cap. Invalid or tampered inputs fail before writing history.

## Persistence boundary

Successful executions atomically write the immutable recipe-version ID, both verified input SHA-256 values, the deterministic result digest, result JSON, timestamps and a matching audit event. History queries return stored results without silently re-reading changed linked files.

The result digest describes normalized comparison outputs and effective settings; input hashes independently identify the source bytes. Runs are ordinary working-data history, not controlled evidence. Source originals may subsequently change or disappear; a run does not imply retained original bytes.

## Historical run provenance inspection (Slice D1)

`inspect_normal_data_run_provenance` accepts an exact completed run UUID and returns
the frozen recipe version, workspace, dataset/content versions, the two **source**
SHA-256 hashes recorded during execution, and the separate **result** SHA-256
digest. It recomputes the digest from stored normalized result values and checks
the stored metadata against frozen recipe and dataset versions. It does not
read current linked files, export originals, create a controlled evidence version,
attach to a workpaper or confer any authorization.

Historical inspection remains available after a linked source changes or
disappears, and reports `currentSourceBytesChecked: false` plus
`isControlledEvidence: false` and `specialistPromotionAuthorized: false`.
This is an inspection receipt, not a source-retention or independent audit
attestation. See [ADR_NORMAL_DATA_PROMOTION_BOUNDARY.md](ADR_NORMAL_DATA_PROMOTION_BOUNDARY.md).

## Explicit live source recheck (D2A, not promotion)

`recheck_normal_data_run_sources` accepts only a completed comparison-run UUID.
It first validates the frozen run, then separately reads source A and B through
the existing native approved-root/stable-read verifier (32 MiB maximum each).
Each source must match its immutable content version **and** the exact SHA-256
recorded in that historical run. Success returns two separately timestamped
read observations; either source's change, missing file, source-version
mismatch, or failed verification returns an error and no success receipt.

The observations are sequential, not an atomic two-file snapshot. There is no
stored source copy, permission grant, workpaper link, evidence capture, or
promise that bytes will remain the same for a later operation. A future
capture must independently authorize and re-verify the originals. This
command does not modify the database, evidence store, or run history.

## Internal specialist authorization boundary (D2B1)

A native-only `normal_data_promotion_policy` module defines the **future**
authorization preflight; it is intentionally **not** a Tauri command. No
production authenticator can currently construct its verified principal.
Without that principal, or if even one of the three permissions (workspace
read, engagement evidence attachment, exact workpaper revision modification)
is missing, it fails closed. The source run must belong to that exact Normal
Data workspace and the active target revision must belong to the exact
engagement/workpaper pair. Stale, archived, signed, review-stage and
unresolved-review-note targets cannot pass the structural policy.

A positive **unit-test fixture** is not proof of production authorization:
no runtime grants, identity provider, specialist capture or append operation
are installed. Recheck policy and source hashes in the eventual write
transaction; a prior preflight is never a reusable permission token.

## Internal durable grant resolution (D2B2A, schema v28)

Schema v28 adds a **default-empty** native permission registry for subject IDs
enrolled by a future trusted authentication authority and exact resource grants
for workspace read, engagement attachment and revision modification. The
`SqlitePromotionPermissions` adapter reads these grants and denies when a
subject is not enrolled, is disabled, the grant is absent, revoked, expired or
for a different resource, or the database cannot be read. Resource/permission
changes and subject reactivation are blocked by SQLite triggers; revoke is a
one-way state transition.

No frontend command creates a subject, grants, authorizes a caller, or performs
promotion. A local subject row is **not authentication**; the production code
cannot construct `VerifiedPrincipal`. SQL content itself is not a signed
authorization credential or protection against a compromised database owner.
D2B2B must establish trusted identity and privileged grant administration and
repeat permission evaluation inside a future atomic write transaction. The
current read-only check returns no approval token and is not safe to use as
the sole authorization at write time.

## Native-only audited administrator lifecycle (D2B2B1, schema v29)

Schema v29 adds immutable administrator event rows for subject enrollment,
grant issuance, grant revocation and permanent subject disabling. A native-only
administrator module performs these mutations and the associated audit insert
inside one SQLite IMMEDIATE transaction. Missing resources, archived targets,
unknown/disabled subjects and expired grants are denied; audit insertion failure
rolls back the permission change. No evidence attachment or controlled capture
can be created through this module.

The administrator capability has only a test-only constructor. There is still
**no trusted authenticated administrator in production**, no frontend/Tauri
admin command, no permission bootstrap path and no permission token issued by
viewing the grant ledger. The local database cannot prove a user is the named
administrator against a malicious database owner. Next work must supply secure
native identity and privilege attestation, enforce it at use time inside the
specialist write transaction and design the controlled-capture recovery plan.

## Atomic native permission/target gate (D2B2B2A)

An internal `with_authorized_transaction` helper provides a transaction-bound
check for future specialist linkage, instead of treating read-only preflight as
a reusable approval. It obtains a SQLite IMMEDIATE reservation; with a
verified principal (unavailable in production), it rechecks all three
exact-resource grants, current enrolment/revocation/expiry, historical run
identity and result digest, and target review/signoff/revision state on **one
SQLite connection**. Only then may a native callback execute, and policy or
callback failures roll back the transaction. A competing SQLite writer cannot
revoke a grant or change review state between the check and callback commit.

This does **not** authenticate a production caller, lock source files, preserve
their bytes, capture a result artifact, create an evidence link, or protect
against an attacker who controls the local database. No Tauri command exposes
the internal gate. The eventual evidence-capture workflow must have its own
retained-artifact verification and staging/recovery logic and must use this
or an equivalent in-transaction check at final commit.

## Exact transient three-artifact material (D2B2B2B1)

The internal-only `prepare_preservation_material` function accepts an immutable
comparison-run UUID and returns two original source byte buffers and the exact
stored comparison result JSON bytes. Source A/B are independently read from
approved roots with the stable, 32 MiB bounded SHA-256 verifier and rechecked
against the historical run's dataset/content-version hashes. The result is
limited to 8 MiB, verified against the original run's canonical semantic
comparison digest, and labeled with an **independent SHA-256 for the serialized
JSON artifact bytes**. A result calculation digest is *not* the artifact-byte
digest, and neither substitutes for a source-file hash.

This structure exists **only in native memory**, with no Tauri API, persistence
write, controlled capture, promotion link, grant or sign-off. Source files are
read one after another and may change again after reading. The adapter does
not provide durable retention, crash recovery, authenticated capture approval
or atomicity across the filesystem and SQLite. Future work must preserve and
verify the exact returned bytes under a crash-recoverable capture policy and
authorize the final specialist link in a fresh transaction.

## Deferred features

XLSX import, configured decimal scales, controlled evidence promotion, paginated history and authenticated reviewer workflow remain separate slices. Workspace, recipe and bounded history UI exist; formal specialist attachment and retention do not.
