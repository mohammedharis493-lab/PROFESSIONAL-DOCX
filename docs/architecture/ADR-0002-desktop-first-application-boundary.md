# ADR-0002: Desktop-First Application Boundary

- **Status:** Accepted
- **Date:** 2026-10-02
- **Project:** Professional DocX
- **Branch:** develop

## Context

Professional DocX must work naturally with existing files stored on Windows PCs, office file servers, and NAS/network paths.

The product must be able to:

- let the user select existing folders,
- recursively enumerate and index files,
- detect file-system changes,
- retain references to original paths,
- open original documents in their normal desktop application,
- provide fast in-app search and navigation,
- later support shared/multi-user firm deployments.

A browser-only application is not an appropriate primary client because arbitrary local/network filesystem access is intentionally restricted by browsers.

## Decision

Professional DocX will use a **desktop-first client architecture**.

### Desktop Shell

The initial desktop shell will use:

```text
Tauri 2
  + React
  + TypeScript
```

Tauri provides the native boundary required for controlled local filesystem access and opening native files while allowing the user interface to be built as a modern web-style application.

### Native Responsibilities

Native/desktop-side code is responsible for capabilities such as:

- folder selection,
- approved-storage-root registration,
- filesystem enumeration,
- file metadata collection,
- file-change watching,
- opening original files with the operating system,
- secure access to explicitly approved local/network paths,
- communication with local indexing/search services where required.

The UI must not receive unrestricted filesystem access.

### Approved Storage Root Boundary

A folder becomes available to Professional DocX only through a native folder-selection and registration flow.

```text
Native folder picker
      |
      v
ApprovedStorageRoot
      |
      +-> storage_root_id
      +-> native/canonical path held by native layer
      |
      v
Frontend receives storage_root_id
```

After registration, ordinary filesystem commands must accept trusted IDs such as:

```text
scan(storage_root_id)
open(document_id)
reveal(document_id)
```

rather than arbitrary absolute path strings supplied by the frontend.

The displayed path may be shown to the user, but it is not an authorization token.

All later file operations must resolve their target through trusted application records and verify that the resolved target remains inside the approved storage boundary.

Approved roots are persisted in SQLite once persistence is introduced. Until then, development builds may hold the registry in native process memory only.

### UI Responsibilities

The React/TypeScript UI is responsible for:

- universal search experience,
- result ranking presentation,
- client/engagement navigation,
- recent items,
- favourites/pins,
- breadcrumbs,
- document context panels,
- workpaper/review interfaces,
- settings and administration.

### Service Boundary

Search, metadata persistence, document extraction, and later audit modules must be accessed through clear application/service interfaces.

The first prototype may run these services locally, but the UI must not assume that all services permanently live in-process.

This permits later deployment modes such as:

```text
Single PC
  Desktop client
  Local metadata/search

Office / Firm
  Desktop clients
       |
       v
  Shared Professional DocX service
       |
       +-> central metadata/search
       +-> file indexing agents near storage
```

### Search and Database Engines Are Replaceable

The application domain must not directly depend on one search/database vendor.

Prototype implementations may use lightweight local storage/search.

Shared deployments may later use PostgreSQL and a dedicated search engine without rewriting the engagement domain or UI search contract.

### Path Permissions

Professional DocX will use explicit allow-scoped access.

A user or administrator chooses the folders/storage roots Professional DocX may index.

The application must not silently scan the entire computer.

Frontend-provided path text is always treated as untrusted input and must not grant filesystem authority.

### File Opening

Users can:

- preview supported documents inside Professional DocX,
- open the original source file in its normal installed desktop application,
- reveal the source location where permitted.

Native open/reveal operations must resolve by trusted document/file identity rather than arbitrary frontend path strings.

## Why Tauri

Tauri 2 provides:

- desktop application packaging,
- scoped filesystem access,
- recursive directory watching,
- native file opening through the operating system,
- a permission/capability model,
- the ability to integrate sidecar processes later if needed.

Official references:

- https://v2.tauri.app/plugin/file-system/
- https://v2.tauri.app/plugin/opener/
- https://v2.tauri.app/plugin/dialog/
- https://v2.tauri.app/learn/sidecar-nodejs/

## Consequences

### Positive

- Fits the existing-file/reference-first architecture.
- Avoids browser filesystem limitations.
- Makes filesystem authority native-owned rather than path-string-driven.
- Can feel like a native Windows productivity application.
- Supports keyboard-first navigation.
- Keeps future shared-server deployment possible.
- Does not require copying all source documents into application storage.

### Trade-offs

- Desktop packaging and updating must be maintained.
- Native permissions require careful security design.
- Approved root registration/persistence becomes a first-class subsystem.
- Network path behaviour must be tested thoroughly.
- Multi-user/shared deployment requires a service boundary in addition to the desktop client.
- Some document-processing capabilities may later require sidecars or server workers.

## Acceptance Tests

1. User can explicitly add an existing test folder through the native folder-selection flow.
2. Frontend receives an opaque `storage_root_id` rather than filesystem authority.
3. Calling a scan operation with an unknown/fabricated root ID fails safely.
4. Adding the folder does not duplicate the full files.
5. Professional DocX can enumerate files below the approved folder.
6. Professional DocX can detect a created/renamed/removed test file.
7. Professional DocX can open a selected original file using its default Windows application.
8. Professional DocX cannot access paths outside configured permissions through the UI.
9. UI search APIs do not depend on a specific search-engine implementation.
