# ADR-0003: Core Domain Model and Configurable Engagement Hierarchy

- **Status:** Accepted
- **Date:** 2026-10-02
- **Project:** Professional DocX
- **Branch:** develop

## Context

Professional DocX must support multiple professional services for the same client, including statutory audit, internal audit, due diligence, statutory compliance, ledger scrutiny, tax/GST reviews, and future services not yet known.

The system must therefore avoid embedding a fixed list of audit areas or a statutory-audit-only schema.

## Decision

The core domain will use a generic engagement hierarchy with reusable objects.

## Core Entities

### Firm

Represents the professional firm/organization using Professional DocX.

Initial relationships:

```text
Firm
  -> Users
  -> Clients
  -> Methodology Library
  -> Engagement Templates
```

### User

Represents an authenticated person.

Examples of role assignments may include:

- auditor/assistant,
- senior,
- manager,
- partner,
- administrator.

Roles and permissions are separate concepts from engagement content.

### Client

Represents the continuing client entity.

A client may have many engagements across periods and service types.

```text
Client
  -> Client metadata
  -> Engagements
  -> Documents/evidence relationships
```

### Engagement

Represents a specific professional assignment.

Examples:

```text
ABC Ltd - Statutory Audit 2025-26
ABC Ltd - Internal Audit Q2 2026
ABC Ltd - Financial Due Diligence 2026
ABC Ltd - GST Review 2025-26
```

Minimum conceptual fields:

- id,
- firm_id,
- client_id,
- name,
- service_type_id,
- period_start,
- period_end,
- status,
- created_at,
- created_by.

### Service Type

Configurable classification of an engagement.

Examples:

- Statutory Audit
- Internal Audit
- Due Diligence
- GST Review
- Tax Review
- Statutory Compliance
- Certification
- Other

Service types are data, not hard-coded application branches.

### Area

A configurable node inside an engagement.

Examples:

- Revenue
- Purchases
- Ledger Scrutiny
- Statutory Compliance
- Payroll
- Procurement
- Contract Management
- Working Capital
- Tax Due Diligence

Areas may contain child areas/sub-areas.

### Area Node Model

The hierarchy should use a parent-child structure rather than separate rigid tables for every depth.

Conceptually:

```text
EngagementArea
  id
  engagement_id
  parent_id nullable
  name
  code nullable
  display_order
  status
  owner_user_id nullable
```

This permits:

```text
Statutory Compliance
  -> GST
      -> RCM
      -> GSTR-1
      -> GSTR-3B
  -> TDS
  -> PF/ESI
```

without schema changes.

### Procedure

Represents a defined audit/review/test procedure.

A procedure can belong to an area and may be created from firm methodology or specifically for one engagement.

### Workpaper

Represents documented professional work.

Conceptual relationships:

```text
Workpaper
  -> Engagement
  -> Area
  -> Procedure optional
  -> Evidence links
  -> Review notes
  -> Findings/exceptions
  -> Prepared sign-off
  -> Reviewed sign-off
```

### Document / File Reference

Represents the application record for a file.

It is not necessarily a stored copy.

Conceptual fields include:

- id,
- storage_state,
- source_path_or_uri,
- display_name,
- normalized_name,
- extension,
- size,
- source_modified_at,
- fingerprint/hash nullable,
- availability_status,
- indexed_at,
- captured_at nullable.

Storage state:

```text
LINKED
CONTROLLED_EVIDENCE
MANAGED
```

### Evidence Link

A many-to-many relationship connecting a document/file reference to professional objects.

A single document may support multiple workpapers/procedures.

An evidence link records the exact relationship rather than forcing the file into one folder.

### Review Note

Represents a reviewer query/comment and its resolution history.

Review notes may be linked to:

- workpaper,
- document,
- page,
- worksheet,
- cell/range where supported.

### Finding / Exception

Represents an identified issue.

The base entity must be generic enough to support:

- audit exceptions,
- internal audit findings,
- due diligence findings,
- compliance exceptions.

Service-specific extensions may add fields without changing the common identity/history model.

### Query / PBC Request

Represents an information/evidence request to the client or another responsible party.

### Audit Event

Append-oriented event record for material actions such as:

- evidence capture,
- relinking,
- sign-off,
- permission changes,
- review-note state changes,
- controlled version changes.

## Relationship Overview

```text
Firm
  |
  +-- User
  |
  +-- Client
       |
       +-- Engagement
            |
            +-- Service Type
            |
            +-- Engagement Area
            |      |
            |      +-- child Engagement Area
            |      +-- Procedure
            |      +-- Workpaper
            |
            +-- File Reference / Evidence Link
            +-- Query / PBC
            +-- Finding
            +-- Review Note
            +-- Audit Event
```

## Design Rules

1. No fixed list of engagement areas in source code.
2. No fixed maximum hierarchy depth for areas, subject to practical UI safeguards.
3. Documents are linked to professional objects through relationships, not only folder placement.
4. Client identity is separate from engagement identity.
5. Engagement period is explicit and is not inferred solely from filenames/folders.
6. Service types are configurable.
7. Workpaper references are stable identifiers inside an engagement.
8. Sign-off records must identify actor and time and must not be represented by a simple editable boolean.
9. Audit events are append-oriented.
10. Domain identifiers must not depend on Windows file paths because files can move.

## Acceptance Tests

1. One client can have both a statutory audit and an internal audit engagement.
2. User can add a new service type without changing source code.
3. User can add `Statutory Compliance -> GST -> RCM` without a database migration.
4. User can add an entirely new area not anticipated by developers.
5. The same evidence file can be linked to two different workpapers without duplicating the source file.
6. Moving a linked file does not change the identity of the Professional DocX document record after legitimate relinking.
