# Professional DocX — Development Phased Plan

**Repository:** `mohammedharis493-lab/Professional-DocX`  
**Purpose:** Build a secure, high-speed professional engagement and audit documentation platform in which auditors, managers, and partners can reach the right document substantially faster than through conventional file-system navigation.

---

## 1. Product Principles

These principles are architectural constraints and should not be weakened silently in later phases.

1. **Accessibility first.** The fastest path to a document should normally be search, recent history, pinned items, contextual relationships, or direct links — not manual folder traversal.
2. **Universal search must work from partial information.** A file named `2024-25 Salamudd ITR.pdf` must be discoverable by searching `salamudd`.
3. **Advanced filters are optional refinements.** Users must not be forced to select client, year, document type, or area before a normal search.
4. **No hard-coded audit-area list.** Service lines, areas, sub-areas, procedures, and checklists must be configurable.
5. **One client can have multiple service lines.** Examples include statutory audit, internal audit, due diligence, GST/tax review, statutory compliance, certifications, and future services.
6. **Original evidence is preserved.** OCR, previews, annotations, derived files, and later versions must never silently overwrite original evidence.
7. **Deterministic accounting logic.** Reconciliations, ledger tests, mappings, balances, and core audit computations must be reproducible and rule-driven. AI may assist with review, explanation, classification, or drafting but must not silently alter accounting results.
8. **Human review remains authoritative.** Important conclusions, deletions, approval actions, rule changes, and final sign-offs require appropriate human authority.
9. **Auditability by design.** Material document and workflow events must be traceable through an append-oriented audit history.
10. **Private/local deployment must remain possible.** Core document storage, search, OCR, workpapers, and review workflows should not require a public SaaS dependency.

---

# Phase 0 — Architecture, Governance, and Test Baseline

## Objective
Freeze the core domain model, development rules, acceptance-test philosophy, and project boundaries before feature implementation.

## Deliverables

- Architecture decision records (ADRs).
- Initial data/domain model for:
  - Firm
  - User
  - Role
  - Client
  - Engagement
  - Service type
  - Area
  - Sub-area
  - Procedure
  - Workpaper
  - Document/evidence
  - Document version
  - Review note
  - Finding
  - Query/PBC request
  - Audit event
- Configurable hierarchy:
  ```text
  Client
    -> Engagement
      -> Service Type
        -> Area
          -> Sub-area
            -> Procedure / Workpaper / Checklist / Evidence
  ```
- Define immutable-original policy.
- Define permission model boundaries.
- Define search-ranking principles.
- Define local/private deployment model.
- Establish automated test framework and CI.
- Establish coding/review rules.

## Gate

Phase 1 cannot begin until:

- Domain model supports arbitrary service types and areas.
- No statutory-audit-specific structure is embedded as a mandatory schema.
- Test harness and CI run successfully.
- Data-loss-sensitive actions have explicit design controls.

---

# Phase 1 — Core Platform Foundation

## Objective
Create the secure application shell and reliable persistence layer on which all later features depend.

## Deliverables

### Backend
- Modular-monolith backend.
- PostgreSQL metadata database.
- Object/file storage abstraction.
- Authentication.
- Role-based access control foundation.
- Client and engagement CRUD.
- Configurable service type / area / sub-area tree.
- API boundaries for documents, search, workpapers, review, and audit events.

### Frontend
- Application shell.
- Persistent global search box placeholder.
- Client/engagement navigation.
- Dynamic area tree.
- Role-aware home screen.
- Responsive desktop-first UI.

### Security
- Secure secret handling.
- Encryption-in-transit requirement.
- File access authorization.
- No client documents committed into Git.
- Backups/export design.

## Gate

- User can create a client.
- User can create multiple engagements for that client.
- User can create custom service types and arbitrary nested areas.
- Permission checks prevent unauthorized engagement access.
- No hard-coded list of Revenue / GST / TDS / etc. is required for the system to function.

---

# Phase 2 — Universal Document Access & Search MVP

> **This is the highest-priority product phase.**
>
> Broader audit functionality should not be allowed to hide weak document access.

## Objective
Make finding and opening a document materially faster than Windows File Explorer for normal audit work.

## Deliverables

### Universal Search
A single search entry point covering:

- Filename/title
- Client
- Engagement
- Financial year / period
- Service type
- Area / sub-area
- Document type
- Tags
- Metadata
- Workpaper title/reference
- Ledger/reference fields when available
- Extracted PDF text
- Word text
- Excel searchable text/cells
- OCR text
- Comments/notes where appropriate

### Search Behaviour

- Case-insensitive.
- Punctuation-insensitive where reasonable.
- Prefix/partial-word matching.
- Typo/fuzzy tolerance.
- Search-as-you-type.
- Exact matches rank above fuzzy matches.
- Filename/title matches rank above internal-content matches.
- Current engagement context may boost ranking without hiding global results.
- Advanced filters remain optional.

### Required Acceptance Examples

Given:

```text
2024-25 Salamudd ITR.pdf
```

all of the following should reasonably retrieve it:

```text
salamudd
Salamudd
SALAMUDD
salamud
salammudd
salamudd itr
2024 salamudd
```

### Ranking Baseline

Indicative deterministic weighting:

```text
Exact filename/title match              Highest
Filename contains all query terms
Filename contains some query terms
Client / party / primary reference
Workpaper / object title
Structured metadata / tags
Extracted document content
Comments / secondary text
Fuzzy-only match                        Lowest
```

### Access Features

- `Ctrl+K` universal search.
- Keyboard-first result navigation.
- Recent documents.
- Recent clients/engagements.
- Recent searches.
- Pinned/favourite documents.
- Pinned engagements.
- Pinned workpapers.
- Breadcrumb navigation.
- Back/forward navigation history.
- Open result directly into in-app preview.

### Performance Targets

Initial engineering targets, to be validated under realistic datasets:

- First useful search results: target **<300 ms** for indexed normal queries where feasible.
- Previously indexed ordinary document preview: target first usable rendering **<1 second** where feasible.
- Recently accessed item: target **1–2 interactions**.
- Normal search path:
  ```text
  Ctrl+K -> type meaningful fragment -> Enter
  ```

### Comparative Usability Benchmark

Build a repeatable benchmark comparing Professional DocX against File Explorer for tasks such as:

- Find `2024-25 Salamudd ITR.pdf`.
- Find a specific month's GST/RCM working.
- Find the final trial balance.
- Find a subcontractor ageing.
- Find prior-year signed financial statements.

Track:

- elapsed time,
- clicks/keystrokes,
- wrong-file openings,
- user success rate.

## Gate

Phase 3 should not be accepted until:

- Partial filename search works reliably.
- Typo-tolerant search works without outranking exact results.
- Search is measurably faster than manual folder traversal on the benchmark set.
- Users can open results without navigating through folder trees.
- Advanced filters are not required for common retrieval.

---

# Phase 3 — Evidence Store, Viewer, Versioning & Relationship Graph

## Objective
Turn stored files into controlled audit evidence rather than ordinary attachments.

## Deliverables

### Evidence Model

```text
Original Evidence
  -> immutable original
  -> preview representation
  -> OCR/text representation
  -> metadata
  -> annotations
  -> subsequent versions
  -> linked workpapers/findings/queries
```

### File Support

Initial target:

- PDF
- Excel
- Word
- CSV
- TXT
- Images
- XML

### Viewer

- In-app PDF viewer.
- Sheet-aware Excel viewer.
- Word preview.
- CSV/table preview.
- Text/XML preview.
- Page/sheet navigation.
- In-document search.
- Open externally when required.

### Excel-Specific Preservation

Where technically possible, retain visibility of:

- worksheet names,
- values,
- formulas,
- merged ranges,
- comments,
- hidden rows/columns indicators,
- hyperlinks.

### Versioning

- Document version history.
- Original preserved.
- New upload can create a new version rather than silent replacement.
- Hash important evidence (e.g. SHA-256).
- Display uploader/date/source/version.

### Relationships

Support explicit links such as:

```text
TB -> Ledger
Ledger -> Workpaper
Workpaper -> Invoice
Invoice -> Bank Receipt
Workpaper -> Query
Finding -> Evidence
Finding -> Management Response
```

### Context Panel

When a document is open, show relevant related items without requiring a new search.

### Go to Source

Where a derived result stores provenance, allow navigation back to:

- source document,
- worksheet,
- cell/range,
- page,
- voucher/transaction,
- import batch.

## Gate

- Original evidence cannot be silently overwritten.
- Version history is reliable.
- Related evidence can be reached directly.
- User can move through evidence relationships and back again without losing context.

---

# Phase 4 — Engagement Workpapers, Review & PBC Workflow

## Objective
Implement professional engagement documentation and review processes.

## Deliverables

### Workpapers

Configurable workpaper fields including:

- Reference
- Title
- Objective
- Procedure
- Population
- Sample
- Evidence
- Exceptions
- Conclusion
- Prepared by/date
- Reviewed by/date

### Workflow

Typical configurable flow:

```text
Not Started
-> In Progress
-> Prepared
-> Submitted for Review
-> Review Point Raised
-> Response Submitted
-> Cleared
-> Finalised
```

### Review Notes

- Raise review note against:
  - workpaper,
  - document,
  - page,
  - worksheet,
  - cell/range where supported.
- Assign owner.
- Due date.
- Response.
- Reviewer clearance.
- Complete history.

### PBC / Client Requests

- Request number.
- Area.
- Description.
- Requested-from party.
- Due date.
- Status.
- Received evidence.
- Auditor assessment.
- Internal-only notes separated from client-visible content.

### Sign-off

Role-aware preparer/reviewer/final approval structure.

## Gate

- Complete prepare-review-clear workflow works without external spreadsheets/email trackers.
- Review notes retain history.
- Evidence remains linked to the exact procedure/workpaper.

---

# Phase 5 — Firm Methodology & Reusable Templates

## Objective
Prevent good audit methodology from being trapped inside individual client files.

## Deliverables

Firm-level reusable libraries for:

- Engagement templates
- Service types
- Areas/sub-areas
- Procedures
- Checklists
- Audit queries
- Risk templates
- Control templates
- Ledger-scrutiny tests
- Report templates
- Management-letter points
- Statutory compliance requirements

### Template Behaviour

```text
Firm Methodology
      |
      +-> Engagement copy/reference
              |
              +-> client-specific additions
              +-> irrelevant items disabled/removed where permitted
              +-> controlled methodology updates
```

## Gate

- New engagements can be created from templates.
- Engagements remain customizable.
- Changes to firm methodology do not silently rewrite previously completed audit evidence.

---

# Phase 6 — Statutory Audit, Ledger Scrutiny & Compliance Modules

## Objective
Add deterministic audit-specific modules on top of the stable document/workpaper foundation.

## 6A. Ledger Scrutiny

Potential deterministic tests:

- high-value transactions,
- unusual journals,
- year-end journals,
- weekend entries,
- round amounts,
- duplicate vouchers,
- duplicate amounts,
- negative balances,
- rare ledger combinations,
- manual journal entries,
- unusual narrations,
- related-party transactions,
- long-standing balances,
- transactions near defined thresholds,
- auditor-selected transactions.

Each exception must retain provenance back to the original imported transaction.

## 6B. Trial Balance / Financial Statement Linkage

- TB import.
- Ledger mapping.
- Schedule mapping.
- FS reference linkage.
- Opening/closing comparisons.
- Direct navigation from FS/TB/workpaper to supporting evidence where possible.

## 6C. Reconciliation Framework

Reusable deterministic engine for future modules such as:

- bank reconciliation,
- GST,
- TDS/26AS,
- EPF/ESI,
- debtor/creditor ageing,
- journal testing,
- other statutory/ledger reconciliations.

## 6D. Statutory Compliance

Configurable model:

```text
Statute / Requirement
-> Applicability
-> Due date
-> Actual compliance date
-> Evidence
-> Status
-> Exception
-> Conclusion
```

Must support adding future laws/requirements without source-code changes.

## Gate

- Accounting outputs are reproducible.
- No AI-only path can change balances or clear deterministic exceptions.
- Every exception can be traced to source data.

---

# Phase 7 — Internal Audit Module

## Objective
Support process/risk/control-based internal audits without forcing statutory-audit structure onto them.

## Deliverables

### Process Structure

Examples:

- Procure-to-Pay
- Order-to-Cash
- Payroll
- Treasury
- Inventory
- Fixed Assets
- Contract Management
- HR
- IT Controls
- Regulatory Compliance

All remain configurable.

### Risk-Control-Test Model

```text
Objective
-> Risk
-> Control
-> Test
-> Evidence
-> Finding
-> Management Response
-> Action Owner
-> Target Date
-> Follow-up
```

### Findings

Finding fields can include:

- Condition
- Criteria
- Cause
- Risk/effect
- Recommendation
- Management response
- Action owner
- Due date
- Status
- Follow-up
- Supporting evidence
- Linked control/risk/workpaper

### Cross-Engagement Reporting

Examples:

- unresolved findings,
- overdue actions,
- repeated findings,
- findings by process,
- findings by risk classification.

## Gate

- Internal audit can be run independently of statutory-audit terminology.
- Finding follow-up survives across periods/engagements.

---

# Phase 8 — Due Diligence / Data Room

## Objective
Provide a due-diligence workflow with secure external collaboration while keeping internal conclusions private.

## Deliverables

### DD Workspace

- Data room
- Request list
- Q&A
- Financial DD
- Tax DD
- Compliance/legal review
- Findings
- Deal issues
- Report builder

### External Access

Separate client/seller/investor-facing permissions for:

- upload,
- view,
- respond to Q&A,
- request-list interaction.

Prevent access to internal:

- draft findings,
- auditor notes,
- internal risk assessments,
- conclusions,
- review notes.

### Security Enhancements

Potential features:

- watermarking,
- download restriction,
- view-only access,
- expiry,
- granular document permissions,
- detailed access history.

## Gate

- External collaboration cannot expose internal workpapers by default.
- Access events remain auditable.

---

# Phase 9 — Integrations, Intake & Automation

## Objective
Reduce manual document handling without sacrificing evidence control.

## Deliverables

### Evidence Inbox

Sources may include:

- manual upload,
- drag/drop,
- email,
- scanner,
- client portal,
- cloud storage,
- accounting systems,
- API imports.

### Suggested Classification

System may suggest:

- client,
- engagement,
- area,
- document type,
- period,
- correspondent/source.

Human confirmation rules should apply where confidence is insufficient.

### Integrations

Potential later integrations:

- Google Drive
- OneDrive/SharePoint
- Dropbox/Box
- Gmail/Outlook
- Tally/accounting exports
- SAP/ERP exports
- client portals

## Gate

- Intake automation never silently destroys or misroutes original evidence.
- All imports record source/provenance.

---

# Phase 10 — AI Assistance

## Objective
Add AI only after deterministic storage, retrieval, workflow, and accounting foundations are reliable.

## Permitted Assistance Examples

- Summarize documents.
- Suggest metadata.
- Suggest likely audit area.
- Explain reconciliation differences.
- Suggest possible audit procedures.
- Draft queries.
- Draft findings.
- Review workpaper completeness.
- Natural-language search over authorized content.
- Identify potentially relevant evidence.

## Guardrails

AI must not silently:

- alter balances,
- rewrite accounting rules,
- delete evidence,
- clear exceptions,
- approve workpapers,
- sign off engagements,
- change permissions.

All AI output should retain clear provenance/context where it influences audit work.

## Gate

- Core application remains usable without AI.
- AI failure cannot prevent ordinary search/retrieval of documents.
- Deterministic source records remain the authority for accounting computations.

---

# Phase 11 — Hardening, Scale, Deployment & Release

## Objective
Prepare the platform for real professional use.

## Deliverables

### Security
- MFA support.
- Session controls.
- Encryption-at-rest deployment options.
- Malware scanning.
- Secure backups.
- Retention policies.
- Legal/engagement hold.
- Disaster recovery procedures.
- Permission and privilege review tooling.

### Deployment Modes

Target architecture should support, where practical:

```text
Cloud
Private firm server
Office LAN server
Local/self-hosted deployment
```

### Performance

Stress-test:

- large client libraries,
- many years,
- many engagements,
- large PDFs,
- large Excel files,
- high document counts,
- concurrent users,
- search indexing/re-indexing.

### Auditability

- Complete append-oriented event trail for material actions.
- Exportable engagement archive.
- Data integrity verification.

### Usability

Repeat accessibility benchmark from Phase 2 at production scale.

The release is not considered successful if richer functionality causes document retrieval to regress materially.

---

# Cross-Phase Non-Regression Tests

These tests should remain active through the life of the project.

## Search

1. `2024-25 Salamudd ITR.pdf` is found by `salamudd`.
2. Case differences do not prevent retrieval.
3. Minor reasonable spelling mistakes can produce fuzzy matches.
4. Exact filename matches rank above fuzzy content matches.
5. Advanced filters are not required for normal search.
6. Search authorization never returns documents the user cannot access.

## Evidence Integrity

1. Original upload remains retrievable after OCR.
2. Original upload remains retrievable after annotation.
3. New versions do not destroy prior versions.
4. Hash/provenance information is retained where required.

## Flexibility

1. A user can create an engagement area unknown to the source code.
2. A new service line can be introduced without a database redesign.
3. Internal audit and DD can use different workflows on the same common platform.

## Audit Workflow

1. Preparer cannot impersonate reviewer sign-off.
2. Review-note history cannot be silently rewritten.
3. Critical deletion/change actions respect authority controls.

## Accounting

1. Deterministic calculations are reproducible from source data.
2. AI cannot silently alter deterministic outcomes.
3. Exceptions retain source-level traceability.

---

# Suggested Initial Technical Direction

This section is provisional and may be changed through explicit ADRs.

```text
Frontend:
  React / Next.js or equivalent modern web framework

Backend:
  Python FastAPI / Django-family modular monolith

Database:
  PostgreSQL

File Storage:
  Abstracted object/filesystem storage

Search:
  Dedicated search service with typo tolerance,
  partial/prefix matching, field weighting, and filters

Workers:
  Background task queue for indexing, OCR, previews,
  document parsing, and large imports

Document Processing:
  Format-specific parsers + OCR where needed
```

The application should expose its own search abstraction so that the underlying search engine can be changed later without rewriting the audit domain.

---

# First Implementation Sequence

The recommended immediate sequence is:

```text
Phase 0
  -> Phase 1
    -> Phase 2 Universal Access/Search
      -> Phase 3 Evidence Viewer/Versioning
        -> Phase 4 Workpapers/Review
          -> Phase 5 Firm Methodology
            -> Phase 6+ specialist modules
```

**Do not rush into AI, analytics dashboards, or specialist modules before Phase 2 document accessibility is demonstrably excellent.**

The principal product benchmark is simple:

> A user who knows approximately what document they want should normally be able to reach it in seconds, without remembering its folder path.

