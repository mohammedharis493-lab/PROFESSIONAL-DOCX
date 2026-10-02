# ADR-0004: Initial Implementation Stack

- **Status:** Accepted
- **Date:** 2026-10-02
- **Project:** Professional DocX
- **Branch:** develop

## Context

The first implementation must prove the core value proposition quickly:

> Find and open the right existing client document materially faster than File Explorer.

The prototype must run locally on Windows without requiring the user to install and administer multiple server products.

At the same time, the architecture must not prevent later shared/multi-user firm deployment.

## Decision

The initial implementation stack will be:

```text
Desktop shell:
  Tauri 2

User interface:
  React + TypeScript
  Vite-based frontend build

Native application core:
  Rust

Local metadata:
  SQLite

Embedded search:
  Tantivy

File storage:
  Existing files remain linked in place by default
  Professional DocX app-data stores metadata/index/cache
  Controlled evidence stored only when explicitly captured

Future shared deployment:
  Separate service/API boundary
  PostgreSQL and/or dedicated shared search may replace local implementations
```

## Why This Stack

### Tauri 2

Required for:

- explicit filesystem access,
- folder selection,
- native file opening,
- file watching,
- Windows desktop packaging,
- permission-scoped local access.

### React + TypeScript

Used for the user-facing productivity interface:

- universal search,
- keyboard navigation,
- result lists,
- recent/pinned items,
- client/engagement navigation,
- document context,
- future workpaper/review screens.

The UI should remain thin and must not contain filesystem/security logic.

### Rust Core

Rust will initially handle:

- filesystem walking,
- metadata collection,
- file watching,
- path normalization,
- hashing/fingerprinting,
- Tantivy indexing/search,
- SQLite persistence integration,
- native open/reveal operations,
- Tauri commands.

This keeps the first prototype self-contained and avoids requiring a separate runtime.

### SQLite

SQLite will hold structured local application metadata for the first prototype.

Examples:

- indexed storage roots,
- file references,
- client metadata,
- engagement metadata,
- pins,
- recents,
- indexing status.

SQLite is not the full-text search engine.

The persistence layer must be abstracted so shared deployment can later use PostgreSQL.

### Tantivy

Tantivy will provide the local search index.

Initial indexed/searchable fields should include:

- normalized filename,
- display filename,
- extension,
- source path components,
- client name,
- engagement name,
- area names,
- tags/metadata,
- extracted document text when available.

Search composition will support:

- exact/term matches,
- prefix matches,
- fuzzy matches,
- field boosting,
- Boolean composition,
- ranked results.

Filename/title fields receive stronger ranking than extracted document contents.

## Search Ranking Principle

Indicative ordering:

```text
Exact filename/title
  >
Filename terms/prefix
  >
Client / engagement / area metadata
  >
Structured tags/metadata
  >
Extracted document contents
  >
Fuzzy-only matches
```

The implementation may use multiple Tantivy subqueries and explicit boosts rather than relying on one generic query parser.

## Application Data Locations

Professional DocX application data may include:

```text
app-data/
  metadata.sqlite
  search-index/
  preview-cache/
  logs/
```

Client source documents are not placed here merely because they are indexed.

Controlled evidence storage will be introduced separately under the evidence-capture phase.

## Service Interfaces

The codebase must define boundaries for:

- FileRepository
- MetadataRepository
- SearchIndex
- FileSystemGateway
- DocumentExtractor

The UI must call application commands/services rather than SQLite or Tantivy directly.

This allows later implementations such as:

```text
MetadataRepository
  LocalSQLiteRepository
  PostgresRepository

SearchIndex
  TantivySearchIndex
  FutureSharedSearchIndex
```

## Document Extraction

Phase 2 search begins with filename/path/metadata indexing.

Content extraction is incremental.

Likely later processors include:

- PDF text,
- DOCX text,
- XLSX worksheets/cells,
- CSV/text,
- OCR for scanned documents.

Heavy format-specific processing may be implemented as Rust modules or isolated worker/sidecar processes without changing the search API.

## Explicit Non-Decisions

This ADR does not yet choose:

- final shared-server framework,
- final PostgreSQL ORM/library,
- final OCR engine,
- final PDF rendering engine,
- final Excel preview renderer,
- cloud deployment provider.

Those choices are deferred until needed.

## Acceptance Tests

1. Desktop application starts without requiring a separate search server.
2. User can add an allowed folder.
3. Rust core enumerates file metadata without copying source files.
4. Metadata persists across application restart.
5. Tantivy index persists across restart.
6. Search for `salamudd` returns `2024-25 Salamudd ITR.pdf`.
7. Exact filename matches rank above fuzzy content-only matches.
8. User can open the original selected file through Windows.
9. Search and persistence are accessed through interfaces rather than directly from UI components.

## References

- Tauri filesystem: https://v2.tauri.app/plugin/file-system/
- Tauri opener: https://v2.tauri.app/plugin/opener/
- Tantivy: https://docs.rs/tantivy/
- Tantivy fuzzy search example: https://github.com/quickwit-oss/tantivy/blob/main/examples/fuzzy_search.rs
