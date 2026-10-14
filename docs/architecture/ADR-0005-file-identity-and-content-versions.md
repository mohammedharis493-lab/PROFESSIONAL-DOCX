# ADR-0005: File Identity, Source Instances, Paths, and Content Versions

- **Status:** Accepted
- **Date:** 2026-10-02
- **Project:** Professional DocX
- **Branch:** develop

## Context

Professional DocX indexes existing files without copying them by default.

A file's path cannot be treated as permanent identity because files may be:

- renamed,
- moved,
- overwritten,
- deleted and recreated at the same path,
- copied,
- accessed through different drive letters or UNC aliases,
- stored on local disks, NAS devices, or network shares.

The application also needs to distinguish the logical professional document from the physical filesystem object and from the exact bytes that were observed or preserved for audit evidence.

Without these distinctions, a new file created at an old path could accidentally inherit historical metadata or evidence relationships.

## Decision

Professional DocX will use separate identities for:

```text
Document
FileInstance
FilePathHistory
ContentVersion
ControlledEvidenceVersion
StorageRoot
```

These concepts must never be collapsed into one path-based file record.

The central rule is:

> Path is location. UUID is application identity. Filesystem identity is evidence about continuity. Hash is content identity.

---

# 1. StorageRoot

A `StorageRoot` represents a logical source namespace approved for Professional DocX access.

Examples:

```text
D:\Clients
\\FileServer\Audit\Clients
Z:\Clients
```

Multiple machine-specific aliases may refer to the same logical storage root in future shared deployments.

Conceptual fields:

```text
storage_root_id           UUID
kind                      LOCAL | NETWORK | REMOVABLE | OTHER
configured_locator
canonical_locator         nullable
display_name
approval_actor_id         nullable in early local mode
approved_at
availability_state
created_at
updated_at
```

Future optional continuity fields may include:

```text
volume_serial_number
filesystem_type
network_share_identity
machine/agent mapping
```

The frontend should primarily refer to a storage root by `storage_root_id`, not by sending arbitrary absolute paths to native commands.

---

# 2. Document

A `Document` is the stable Professional DocX logical object.

It is independent of any particular current path.

Conceptual fields:

```text
document_id               UUID
storage_state             LINKED | CONTROLLED_EVIDENCE | MANAGED
display_name
professional_metadata
created_at
created_by
archived_at                nullable
```

A document may currently be represented by one or more source instances or preserved versions.

The document identity must survive legitimate moves or renames.

---

# 3. FileInstance

A `FileInstance` represents one observed physical filesystem object.

Conceptual fields:

```text
file_instance_id          UUID
document_id
storage_root_id

relative_path_native
relative_path_display
relative_path_search

filesystem_identity       nullable
volume_identity           nullable

creation_time             nullable
last_write_time           nullable
size_bytes
file_attributes           nullable
reparse_tag               nullable

first_seen_at
last_seen_at
first_seen_generation
last_seen_generation
availability_state
```

On Windows, where supported, filesystem continuity information may include:

```text
volume_serial_number
file_id_128
```

These values are **strong continuity signals**, not database primary keys and not eternal document identity.

A file ID may change or later be reused, so it cannot replace `file_instance_id`.

---

# 4. FilePathHistory

Path history must be explicit.

Conceptual fields:

```text
file_path_history_id      UUID
file_instance_id
storage_root_id
relative_path_native
relative_path_display
observed_from
observed_until            nullable
change_reason             DISCOVERED | RENAMED | MOVED | RELINKED | OTHER
actor_id                  nullable
scan_generation_id        nullable
```

A rename or same-filesystem move should normally append path history rather than create a new logical document.

---

# 5. Native Path Storage

Professional DocX must not rely only on lossy display strings.

The authoritative source representation must preserve the native path without lossy conversion.

Conceptually store three representations:

```text
relative_path_native
relative_path_display
relative_path_search
```

Meaning:

- `relative_path_native` — authoritative native representation.
- `relative_path_display` — human-readable representation.
- `relative_path_search` — normalized representation used for searching.

Absolute path identity should be derived from:

```text
storage_root_id
+
relative_path_native
```

rather than stored as the sole globally meaningful address.

This is important because the same logical storage root may later appear as:

```text
User A: Z:\Clients
User B: \\FileServer\Audit\Clients
Indexer: \\FileServer\Audit\Clients
```

---

# 6. ContentVersion

A `ContentVersion` represents the contents observed for a file instance at a point in time.

Conceptual fields:

```text
content_version_id        UUID
document_id
file_instance_id
observed_at
size_bytes
last_write_time           nullable
quick_fingerprint         nullable
sha256                    nullable
verification_state
source_stable_during_read
```

A content version does not need a SHA-256 merely because a file was discovered.

Expensive cryptographic hashing should be performed when:

- evidence is formally captured,
- exact verification is required,
- identity is ambiguous,
- policy requires it.

Initial large-scale discovery should not hash every file eagerly.

---

# 7. ControlledEvidenceVersion

A `ControlledEvidenceVersion` represents a formally preserved evidence object.

Conceptual fields:

```text
controlled_evidence_version_id   UUID
document_id
source_content_version_id
controlled_storage_locator
sha256                           REQUIRED
size_bytes
captured_at
captured_by
capture_reason
capture_policy
version_number
retention_state
verification_state
```

Once created, the controlled evidence version is independent of later changes to the linked source file.

Its bytes and identifying hash must not be silently replaced.

A later evidence capture creates a new controlled-evidence version.

---

# 8. Identity Rules

## Rename

If strong continuity signals indicate the same physical source object at a new path:

```text
same FileInstance
+
new FilePathHistory record
```

The Document remains unchanged.

## Move on the same filesystem

Treat the same as rename when continuity is sufficiently strong.

## Move when filesystem identity is unavailable

Metadata, fingerprint, or hash may be used to suggest a relink.

Ambiguous cases require user confirmation.

Do not silently decide that two files are the same document merely because their metadata is similar.

## Edit / overwrite while physical object continuity remains

If the same physical source object remains but contents change:

```text
same Document
same FileInstance
new ContentVersion
```

The previous content version remains historical metadata where required.

## Delete and recreate at the same path

If continuity signals indicate a different physical source:

```text
new FileInstance
```

The new file must not inherit historical file-instance identity merely because the path is identical.

Depending on professional context, it may either:

- become a new Document, or
- be explicitly associated with an existing Document as a new source instance through controlled reconciliation.

That association must never be inferred from path alone.

## Copy

A copied file is:

```text
new FileInstance
```

even if the bytes and SHA-256 are identical.

Identical content does not imply identical provenance.

## Two identical files

Both are retained as distinct file instances.

They may share the same content hash.

## Network remount / drive-letter change

The logical `storage_root_id` remains stable.

Machine-specific aliases may change.

A drive letter must not become global document identity.

## Controlled evidence

Controlled evidence is identified by:

```text
controlled_evidence_version_id
+
preserved object
+
required SHA-256
```

It does not follow later linked-source changes.

---

# 9. Continuity Confidence

Professional DocX may internally classify continuity evidence.

Example:

```text
CONFIRMED
  same logical root
  strong filesystem identity continuity

STRONG
  strong metadata/fingerprint continuity

SUGGESTED
  probable match but requires user confirmation

UNRELATED
  continuity evidence conflicts
```

This is not an audit conclusion.

It is a filesystem reconciliation aid.

Ambiguous continuity must not silently alter document identity.

---

# 10. Evidence and Workpaper Implication

Evidence links used for professional review must eventually point to an exact version, not merely a mutable linked path.

The required relationship is:

```text
WorkpaperRevision
      |
      v
EvidenceLink
      |
      v
ContentVersion
or
ControlledEvidenceVersion
```

Formal sign-off binding is defined separately in ADR-0007.

---

# 11. Persistence Consequences

The first SQLite schema must preserve these boundaries.

Do not create one generic table such as:

```text
files(
  id,
  path,
  filename,
  size,
  modified
)
```

and expect to retrofit identity semantics later.

At minimum, the initial migration should reserve explicit concepts for:

```text
storage_roots
documents
file_instances
file_path_history
content_versions
```

Controlled-evidence tables may be introduced later when that feature is implemented, but their identity contract is established now.

---

# 12. Search Consequences

Search results must identify a stable Professional DocX object using `document_id`.

Search-index records may also include:

```text
file_instance_id
content_version_id
storage_root_id
```

as appropriate.

A path is searchable/displayable metadata, not the primary search result identity.

---

# 13. Failure-Safety Rules

1. A path being reused must never silently transfer historical identity.
2. A different physical file appearing at an old path must be detectable.
3. A controlled-evidence version must remain unchanged if its source later changes.
4. Two different filesystem instances with identical bytes must remain distinguishable.
5. A rename must not unnecessarily create duplicate documents when continuity is reliable.
6. Ambiguous relinking requires explicit reconciliation rather than silent guessing.
7. Search and metadata relationships use UUID identities rather than path strings.

---

# 14. Acceptance Tests

Before file persistence is considered stable, automated tests must eventually cover:

1. Rename a file and preserve the same Document/FileInstance when continuity is confirmed.
2. Move a file on the same volume and preserve continuity where supported.
3. Delete a file and recreate a different file at the same path; do not reuse the old FileInstance.
4. Modify a file in place; create a new ContentVersion.
5. Copy a file; create a separate FileInstance even when hashes match.
6. Maintain two identical files as distinct source instances.
7. Change a mapped drive alias without changing the logical StorageRoot.
8. Preserve path history after legitimate relinking.
9. Ensure search result identity remains stable after rename.
10. Ensure a ControlledEvidenceVersion remains unchanged after the source file changes.

---

## Consequences

### Positive

- File identity no longer depends on mutable paths.
- Rename/move behavior becomes recoverable and auditable.
- The model supports NAS aliases and future shared indexing agents.
- Exact evidence versions can later be bound to workpaper revisions and sign-offs.
- SQLite schema design can proceed without embedding path-based identity mistakes.

### Trade-offs

- More entities are required than a basic file catalogue.
- Reconciliation logic must combine multiple continuity signals.
- Windows filesystem identity requires platform-specific implementation.
- Ambiguous move/relink cases sometimes require human confirmation.
- Native path persistence needs careful cross-platform representation.

## References

This ADR implements the file-identity recommendations from the independent adversarial foundation review dated 2026-10-02.
