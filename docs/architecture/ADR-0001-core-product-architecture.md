# ADR-0001: Core Product Architecture Principles

- **Status:** Accepted
- **Date:** 2026-10-02
- **Project:** Professional DocX

## Context

Professional DocX is intended to give auditors, managers, and partners substantially faster access to the correct client document than conventional File Explorer navigation, while growing into a professional engagement platform for statutory audit, internal audit, due diligence, compliance, ledger scrutiny, and other services.

The system must not require users to remember physical folder paths in order to find documents. It must also avoid unnecessarily duplicating all existing client files.

## Decision

Professional DocX will follow these architectural principles.

### 1. Accessibility is the primary product constraint

The default route to information is universal search, recent history, pins/favourites, contextual relationships, and direct links.

Manual folder navigation remains available but is not the primary retrieval model.

A document such as:

```text
2024-25 Salamudd ITR.pdf
```

must be discoverable from a simple search such as:

```text
salamudd
```

Advanced filters are optional refinements.

### 2. Engagement structure is configurable

The system must not hard-code a fixed list of audit areas.

The common hierarchy is:

```text
Client
  -> Engagement
    -> Service Type
      -> Area
        -> Sub-area
          -> Procedure / Workpaper / Checklist / Evidence
```

Users can add service types, areas, sub-areas, procedures, and checklists without a source-code change.

A single client can have multiple engagements and service lines.

### 3. Existing files are linked by default

Professional DocX normally indexes and references files where they already exist:

- local Windows storage,
- office file server,
- NAS/network storage,
- other configured storage locations.

Indexing a folder must not require creating a second full copy of every source file.

Professional DocX may store metadata, extracted text, thumbnails, previews, and search indexes required for fast retrieval.

### 4. Three storage states are supported

#### LINKED FILE

The source file remains in its existing location.

Professional DocX stores:

- canonical source path/URI,
- filename,
- size/timestamps,
- metadata,
- search index,
- optional fingerprint/hash,
- optional preview/cache,
- availability status.

#### CONTROLLED EVIDENCE

When an engagement requires formal preservation, a linked file may be captured as immutable controlled evidence.

The captured evidence records:

- source provenance,
- capture date,
- capturing user,
- cryptographic hash,
- engagement/workpaper relationships,
- controlled version history.

Subsequent changes to the external working file must not alter previously captured evidence.

#### MANAGED FILE

A user may explicitly import a file into Professional DocX-managed storage.

This is an intentional action and is not the default behaviour for existing files.

### 5. Source-file changes must be visible

For linked files, the system must detect or surface when the original source is:

- unavailable,
- moved,
- renamed,
- materially changed.

Legitimate relinking must be auditable.

The system must never silently treat a changed external source as the same signed-off controlled evidence.

### 6. Original evidence is preserved

OCR, previews, extracted text, annotations, caches, and derived representations do not replace the original evidence.

Search indexes and previews are rebuildable derived data.

### 7. Accounting logic is deterministic

Reconciliations, ledger tests, mappings, balances, and other accounting computations must be reproducible from defined rules and source data.

AI may assist with explanation, classification, drafting, or review but must not silently alter deterministic accounting outcomes or sign off work.

### 8. Auditability is built in

Material actions involving evidence, permissions, review, sign-off, relinking, and controlled versions must be traceable through an append-oriented audit history.

### 9. Local/private deployment remains possible

Core functions such as:

- document indexing,
- search,
- document opening/preview,
- workpapers,
- evidence capture,
- review workflows,

must not inherently require a public SaaS provider.

## Consequences

### Positive

- Existing client libraries can be indexed without doubling storage.
- Search can become substantially faster than File Explorer navigation.
- Audit evidence can still be preserved when required.
- The architecture can support multiple professional service lines.
- Specialist audit modules can be added without redesigning the document foundation.

### Trade-offs

- Linked-file monitoring is more complex than copying everything into one store.
- Network paths can become temporarily unavailable.
- File move/rename/change detection needs careful engineering.
- Controlled evidence requires additional storage for selected documents.
- Search indexes and previews require rebuild and integrity strategies.

## Non-Negotiable Acceptance Tests

1. `2024-25 Salamudd ITR.pdf` can be found using `salamudd`.
2. Adding an existing folder for indexing does not create a second full copy of every source file.
3. A new engagement area can be created without a code change.
4. A controlled-evidence snapshot does not change when the linked working file later changes.
5. Search never exposes a document the current user is not authorized to access.
