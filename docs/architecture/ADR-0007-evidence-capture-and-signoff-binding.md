# ADR-0007: Controlled Evidence Capture, Workpaper Revisions, and Sign-Off Binding

- **Status:** Accepted
- **Date:** 2026-10-02
- **Project:** Professional DocX
- **Branch:** develop

## Context

Professional DocX supports linked working files that may continue to change during an engagement.

The core review-integrity question is:

> When a reviewer signs off a workpaper, exactly which workpaper content and exact evidence versions did they review?

A sign-off against a mutable workpaper row, mutable linked path, or "latest file" is not sufficient.

The system must preserve historical review meaning even after the workpaper or linked evidence later changes.

## Decision

Professional DocX will use explicit immutable revision/version identities for reviewable work.

The required relationship is:

```text
Workpaper
   |
   +-> WorkpaperRevision
            |
            +-> EvidenceLink
                    |
                    +-> ContentVersion
                    or
                    +-> ControlledEvidenceVersion

SignOff
   |
   +-> exact WorkpaperRevision
   +-> exact evidence-version set represented by that revision
```

A reviewer never signs off merely on a current path, current workpaper row, or latest external file.

---

# 1. Workpaper

A `Workpaper` is the stable logical professional object.

Conceptual fields:

```text
workpaper_id              UUID
engagement_id
area_id
reference
title
status
created_at
created_by
archived_at               nullable
```

The stable workpaper identity is separate from its reviewable revisions.

---

# 2. WorkpaperRevision

Every material reviewable state is represented by a `WorkpaperRevision`.

Conceptual fields:

```text
workpaper_revision_id     UUID
workpaper_id
revision_number
created_at
created_by

objective
procedure_performed
population
sample
exceptions
management_explanation
conclusion

revision_reason           nullable
supersedes_revision_id    nullable
content_hash              optional/derived
state
```

Possible states include:

```text
DRAFT
PREPARED
SUBMITTED_FOR_REVIEW
REVIEWED
SUPERSEDED
FINAL
```

The UI may present one continuing workpaper, but formal review binds to one immutable revision identity.

---

# 3. EvidenceLink

An `EvidenceLink` connects a workpaper revision to an exact evidence version.

Conceptual fields:

```text
evidence_link_id
workpaper_revision_id
document_id
content_version_id                  nullable
controlled_evidence_version_id      nullable
relationship_type
description                         nullable
created_at
created_by
```

For evidence contributing to formal review, the relationship must resolve to an exact version.

A mutable source path is not sufficient.

---

# 4. Evidence Capture Policy

A linked file may remain mutable during ordinary preparation.

The minimum integrity rule is:

> Evidence required for formal reviewer reliance must be represented by an exact immutable version no later than submission for formal review.

The default Level-B policy is therefore:

```text
Working linked file
        |
        v
Prepared workpaper
        |
        v
Submission for review
        |
        +-> capture required evidence versions
        |
        v
Immutable ControlledEvidenceVersion
        |
        v
Reviewer reviews exact revision/version set
```

A future engagement policy may permit earlier capture.

Capture timing may be configurable, but it may not be so late that a reviewer signs off a mutable linked path.

---

# 5. Controlled Evidence Capture Atomicity

A successful controlled-evidence capture requires all of the following:

1. Source identity resolved.
2. Source opened.
3. Exact bytes copied to controlled storage.
4. SHA-256 calculated over captured bytes.
5. Source stability checked sufficiently for the operation.
6. Controlled object durably stored.
7. Metadata committed.
8. Capture marked COMPLETE.

If any required stage fails, the evidence version is not COMPLETE and cannot be used as completed formal evidence.

Suggested states:

```text
PENDING
CAPTURING
COMPLETE
FAILED
QUARANTINED
```

A later integrity-verification failure may move an evidence version to `QUARANTINED`; the system must not silently replace it.

---

# 6. Source Stability During Capture

The source may change while being read.

For formal evidence capture, compare relevant source identity/metadata before and after capture, including where available:

```text
file-instance continuity
size
last-write time
filesystem identity
```

If the source changes during capture:

```text
abort capture
do not finalize controlled evidence
retry or require user action
```

The SHA-256 recorded for controlled evidence is calculated from the preserved controlled copy.

---

# 7. ControlledEvidenceVersion

ADR-0005 defines the identity concept.

For review integrity, a completed version includes at least:

```text
controlled_evidence_version_id
document_id
source_content_version_id
controlled_storage_locator
sha256
size_bytes
captured_at
captured_by
capture_reason
capture_policy
version_number
verification_state
```

The preserved bytes are immutable through normal application operations.

A later capture creates a new version rather than replacing the old one.

---

# 8. SignOff

A `SignOff` is an immutable historical professional action.

Conceptual fields:

```text
signoff_id                UUID
workpaper_id
workpaper_revision_id
signoff_type
actor_id
actor_role
signed_at
status
superseded_at             nullable
superseded_reason         nullable
comment                   nullable
```

Typical sign-off types may include:

```text
PREPARED
REVIEWED
FINAL_APPROVAL
```

A sign-off must not be represented only by a mutable boolean such as `reviewed = true`, and historical sign-offs must not be rewritten to pretend they never occurred.

---

# 9. Sign-Off Binding

At sign-off time, the system must be able to answer:

```text
Which workpaper revision was signed?
Which evidence versions were linked to that revision?
Which user signed it?
Which role/authority did they exercise?
When did they sign?
Has that sign-off since been superseded?
```

Historical review views must resolve through exact stored version IDs, never through "latest version" semantics.

---

# 10. Material Changes After Sign-Off

A material edit after sign-off creates a new `WorkpaperRevision`.

The old sign-off remains historically valid for the old revision but is no longer the active sign-off for the changed work.

Required behavior:

```text
Revision 3
  reviewed by Manager A
        |
        v
material change
        |
        v
Revision 4 created

Revision 3 sign-off:
  retained historically
  marked superseded for active workflow

Revision 4:
  requires appropriate re-preparation/re-review
```

The prior sign-off must never be deleted or silently migrated to the new revision.

---

# 11. Material Change Policy

The architecture must support sign-off invalidation for changes such as:

- objective/procedure changes,
- conclusion changes,
- exception changes,
- evidence added/removed/replaced,
- evidence version changed,
- material population/sample changes,
- management explanation changes,
- review-sensitive adjustments.

Cosmetic changes may later be treated differently by policy.

The reason for a new revision must be recorded.

---

# 12. Evidence Set Changes

Adding, removing, or changing review-relevant evidence after sign-off creates a new revision or otherwise invalidates the active review state.

Example:

```text
Revision 5
  Evidence A v1
  Evidence B v2
  Manager reviewed

Auditor replaces Evidence B with B v3

Result:
  Revision 5 remains historical
  Revision 6 references B v3
  prior manager sign-off is not active for Revision 6
```

This applies even when filename and path are unchanged.

---

# 13. Review Notes

A review note references the workpaper revision against which it was raised.

Where relevant it may additionally reference:

- evidence version,
- page,
- worksheet,
- cell/range,
- other stable location metadata.

Clearing or reopening a review note records a new state transition and preserves history.

---

# 14. Finalisation

Workpaper/engagement finalisation policy may later require:

- all required evidence captured,
- all required sign-offs current,
- no unresolved review notes,
- no invalidated review state,
- required evidence integrity verification passing.

Finalisation must never merely freeze references to the latest mutable external paths.

---

# 15. Evidence Deletion and Retention

Controlled evidence is authoritative data, not cache.

It may not be removed through ordinary cache cleanup.

A later retention policy will define:

- authority,
- retention period,
- legal/engagement hold,
- audit event,
- secure deletion,
- export/archive implications.

Until then, controlled evidence is treated conservatively as retained authoritative data.

---

# 16. Audit Trail Requirements

Material events include:

```text
workpaper revision created
workpaper submitted for review
evidence capture started
evidence capture completed
evidence capture failed
evidence link added
evidence link removed
sign-off created
sign-off superseded
review note raised
review note cleared
review note reopened
workpaper finalised
```

These events must be append-oriented and must not be silently overwritten.

---

# 17. UI Consequences

The UI should make revision state clear.

Examples:

```text
Prepared — Revision 4
Reviewed — Revision 4
```

After a material change:

```text
Revision 5
Previous review superseded — re-review required
```

Historical views should clearly show the reviewed revision and exact evidence versions used.

---

# 18. Persistence Consequences

The future SQLite schema must reserve explicit structures for:

```text
workpapers
workpaper_revisions
evidence_links
signoffs
review_notes
```

and eventually:

```text
controlled_evidence_versions
evidence_capture_jobs
```

Do not model sign-off only as mutable columns on `workpapers`.

---

# 19. Acceptance Tests

Before formal review/sign-off is production-ready, tests must cover:

1. Reviewer sign-off references exactly one WorkpaperRevision.
2. Signed revision resolves exact evidence-version IDs.
3. Historical sign-off never resolves evidence using latest-file semantics.
4. Material workpaper edit after review creates a new revision.
5. Prior sign-off remains historically visible after supersession.
6. New revision requires re-review.
7. Replacing evidence with a new version invalidates/supersedes active review state.
8. External linked-file change does not alter a previously captured ControlledEvidenceVersion.
9. Controlled evidence SHA-256 verifies against stored bytes.
10. Source changing during evidence capture prevents successful finalization.
11. Failed evidence capture cannot be used as completed formal evidence.
12. Review-note history survives clear and reopen operations.

## Consequences

### Positive

- Reviewers sign exact, reproducible professional work states.
- External file changes cannot silently change evidence underlying historical sign-off.
- Historical sign-off remains auditable after later edits.
- Evidence/version semantics are defined before persistence makes them expensive to change.
- The application can later prove what was reviewed.

### Trade-offs

- Workpapers require revision records rather than one mutable row.
- Evidence capture adds storage for selected formal evidence.
- Editing reviewed work requires explicit re-review workflow.
- The UI must communicate current versus historical review state clearly.
- Capture policy must balance audit integrity with unnecessary duplication.

## Related ADRs

- ADR-0001 — Core Product Architecture Principles
- ADR-0003 — Core Domain Model and Configurable Engagement Hierarchy
- ADR-0005 — File Identity, Source Instances, Paths, and Content Versions
- ADR-0006 — Indexing Jobs, Scan Generations, Reconciliation, and Search Consistency
