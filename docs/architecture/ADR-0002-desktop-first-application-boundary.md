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
- filesystem enumeration,
- file metadata collection,
- file-change watching,
- opening original files with the operating system,
- secure access to explicitly approved local/network paths,
- communication with local indexing/search services where required.

The UI must not receive unrestricted filesystem access.

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

### File Opening

Users can:

- preview supported documents inside Professional DocX,
- open the original source file in its normal installed desktop application,
- reveal the source location where permitted.

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
- https://v2.tauri.app/learn/sidecar-nodejs/

## Consequences

### Positive

- Fits the existing-file/reference-first architecture.
- Avoids browser filesystem limitations.
- Can feel like a native Windows productivity application.
- Supports keyboard-first navigation.
- Keeps future shared-server deployment possible.
- Does not require copying all source documents into application storage.

### Trade-offs

- Desktop packaging and updating must be maintained.
- Native permissions require careful security design.
- Network path behaviour must be tested thoroughly.
- Multi-user/shared deployment requires a service boundary in addition to the desktop client.
- Some document-processing capabilities may later require sidecars or server workers.

## Acceptance Tests

1. User can explicitly add an existing test folder as an indexed location.
2. Adding the folder does not duplicate the full files.
3. Professional DocX can enumerate files below the approved folder.
4. Professional DocX can detect a created/renamed/removed test file.
5. Professional DocX can open a selected original file using its default Windows application.
6. Professional DocX cannot access paths outside configured permissions through the UI.
7. UI search APIs do not depend on a specific search-engine implementation.
