# Professional DocX — Architect Handoff — 2026-10-09

**Repository:** `mohammedharis493-lab/PROFESSIONAL-DOCX`  
**Handoff baseline:** `develop` at `17f1ecddd7ea6a3790b84c457dbccbb7b7de7088`  
**Baseline commit:** `Add immutable due diligence report builder foundation (#63)`  
**Current schema at baseline:** SQLite schema version **24**  
**Prepared for:** the next architect/model taking over implementation

---

## 1. Read this first

Professional DocX has progressed well beyond the early search/viewer prototype. The common platform, audit workflow foundation, deterministic accounting modules, internal audit foundation, and due-diligence foundation are now materially implemented.

The most important architectural direction for the next version is:

> **Professional DocX must support a first-class Normal Data Mode in addition to audit-related modes.**

Do **not** continue treating the application as though every useful workflow must belong to a statutory audit, internal audit, due-diligence engagement, compliance assessment, workpaper, or controlled-evidence process.

The correct product model is:

```text
COMMON PROFESSIONAL DOCX PLATFORM
  |
  +-- Universal documents/search/viewer
  +-- Linked/managed/controlled storage model
  +-- Generic data workspaces and deterministic data operations
  +-- Provenance/version/run history
  |
  +-- Normal Data Mode
  |
  +-- Statutory Audit Mode
  +-- Internal Audit Mode
  +-- Due Diligence Mode
  +-- Compliance / Reconciliation workflows
  +-- Future professional-service modes
```

Specialist modes are **overlays on the common platform**, not the definition of the platform itself.

---

## 2. Repository and governance rules

Continue using the protected `develop` workflow.

Do not bypass branch protection, CI, Security, review gates, or exact-head merge controls.

Before every logical unit:

1. fetch/verify current `develop`;
2. create or adopt a feature branch from the exact protected base;
3. keep the diff narrow;
4. open a PR;
5. require CI + Security success;
6. confirm mergeability;
7. confirm no unresolved review threads;
8. confirm `develop` has not moved incompatibly;
9. merge using the exact expected head;
10. verify the new protected `develop` head.

If a fix is pushed, restart the bounded validation from the replacement head.

Do not force-push or rewrite shared history merely to make the branch look clean.

Do not suppress Rust/Clippy/security findings just to obtain green checks.

### Filesystem trust boundary

The frontend must not be allowed to provide arbitrary sensitive filesystem paths to native commands.

Continue the established model:

```text
Frontend stable ID / structured input
        |
        v
Rust / Tauri command boundary
        |
        v
Persistence lookup
        |
        v
Approved storage root / managed store / controlled evidence store
        |
        v
Canonical internally-resolved path
```

Never weaken approved-root and canonical-path controls for convenience.

### Evidence integrity

Keep these distinctions explicit:

- **LINKED FILE** — ordinary existing source remains in its current location.
- **MANAGED FILE** — explicitly imported into Professional DocX-controlled storage.
- **CONTROLLED EVIDENCE** — immutable captured snapshot used where evidence preservation is required.

Controlled evidence is immutable. A later linked-file change must not alter an earlier controlled version.

Derived representations, indexes, previews, normalized rows, comparison outputs, or AI summaries are not substitutes for the authoritative source.

---

## 3. Current product state at this handoff

The repository has merged the following major capability groups.

### Common platform

Implemented foundations include:

- Tauri 2 + Rust native boundary;
- React + TypeScript frontend;
- SQLite authoritative metadata;
- Tantivy-oriented local search architecture;
- approved storage roots;
- linked file indexing without compulsory duplication;
- safe document opening and preview routes;
- text, PDF, Excel, Word and raster-image preview support;
- search/navigation/history/pins;
- document version history;
- linked-file reconciliation and relinking;
- quick-fingerprint/change detection;
- document relationships/context;
- controlled evidence capture and exact-version provenance.

### Engagement / workpaper platform

Implemented through the Phase 4/5 work includes:

- clients;
- configurable service types;
- engagements;
- arbitrary engagement areas;
- procedures;
- workpapers;
- immutable workpaper revisions;
- exact evidence-version linking;
- review workflow/history;
- review-note exact location anchors;
- PBC/request workflow;
- revision-bound sign-offs;
- reusable engagement templates;
- versioned firm methodology;
- reusable firm-library categories.

Do not reintroduce a hard-coded statutory-audit area hierarchy.

### Statutory audit / deterministic accounting

Phase 6 has advanced materially beyond the older development-plan wording.

Merged capabilities include:

- controlled-evidence ledger import;
- deterministic high-value ledger scrutiny;
- immutable/narrow ledger provenance;
- test-run history;
- controlled Trial Balance import;
- deterministic opening/closing movement comparison;
- immutable ledger-to-TB mapping;
- TB schedule mapping;
- financial-statement reference linkage;
- controlled PDF supporting-evidence navigation;
- deterministic reconciliation framework;
- reconciliation review UI;
- versioned statutory-compliance foundation;
- statutory-compliance assessment UI.

Important merged PR sequence:

- #35 deterministic controlled-evidence ledger scrutiny foundation
- #37 narrow persisted ledger provenance
- #38 controlled ledger high-value scrutiny UI
- #39 ledger test-run review history
- #40 controlled Trial Balance import foundation
- #41 controlled Trial Balance workflow UI
- #42 deterministic TB opening/closing comparison
- #43 immutable ledger-to-TB mapping foundation
- #44 ledger-to-TB mapping UI
- #45 immutable TB schedule mapping foundation
- #46 TB schedule mapping UI
- #47 immutable financial-statement reference linkage
- #48 FS reference linkage UI
- #49 managed controlled-PDF preview route
- #50 controlled PDF supporting-evidence navigation UI
- #51 deterministic reconciliation framework foundation
- #52 reconciliation review UI
- #53 versioned statutory-compliance foundation
- #54 statutory-compliance assessment workspace

### Internal audit

Merged capabilities include:

- configurable process/risk/control/test foundation;
- internal-audit workspace UI;
- immutable findings/follow-up event model;
- findings/follow-up workspace.

Relevant PRs:

- #55 internal audit process/risk/control foundation
- #56 internal audit process/risk/control workspace
- #57 immutable internal-audit findings and follow-up
- #58 internal-audit findings/follow-up workspace

### Due diligence

Merged through the handoff baseline:

- internal DD request foundation;
- internal DD request workspace;
- append-only due-diligence issue workflow;
- internal DD findings workspace;
- immutable due-diligence report builder foundation.

Relevant PRs:

- #59 internal DD request foundation
- #60 internal DD request workspace UI
- #61 append-only DD issue workflow
- #62 internal DD findings workspace UI
- #63 immutable DD report builder foundation

At the time this handoff was prepared:

- **PR #64 — “Add immutable due diligence report builder UI”**
- head: `8f69343f61c3857fcbd4c175af56c80e02575fe7`
- base: `17f1ecddd7ea6a3790b84c457dbccbb7b7de7088`
- mergeable: true
- CI: success
- Security: success
- review threads: none
- reviews: none

The next architect must re-check live state before merging. Do not assume this PR is still open or unchanged.

### Stale / legacy PRs

Do not accidentally revive old stale branches/PRs such as #17 or #9 merely because they remain visible. Their concepts have already been superseded by later merged work.

The long-lived draft PR #1 is also not the unit of work to merge into `develop`.

---

## 4. Architectural correction: add Normal Data Mode

### 4.1 Why this is required

The platform began from an audit/professional-engagement roadmap, and many later features naturally use audit vocabulary:

- engagement;
- workpaper;
- controlled evidence;
- review note;
- sign-off;
- audit finding;
- statutory compliance;
- due-diligence issue.

Those concepts are valid specialist workflows, but they must **not** become mandatory wrappers around ordinary professional document or data work.

Users also need Professional DocX for normal work such as:

- opening/searching ordinary files;
- comparing two spreadsheets;
- cleaning a dataset;
- joining two tables;
- grouping or pivoting data;
- comparing period summaries;
- finding additions/removals/differences;
- identifying duplicates;
- building a reusable calculation or comparison;
- saving analysis results;
- preparing a normal management/working-data output;
- performing tax/GST/accounting data analysis that is not yet formal audit evidence;
- exploring client data before deciding whether it belongs to an engagement.

Therefore add a first-class **Normal Data Mode** (a better UI name may later be chosen, e.g. **Data Workspace**, **General Data**, or **Analysis Workspace**).

### 4.2 Normal Data Mode is not “Audit Mode with controls hidden”

Do not implement Normal Data Mode merely by setting `audit = false` and reusing audit screens.

It needs its own clean professional semantics.

Normal Data Mode should not require:

- an engagement;
- an audit area;
- a workpaper;
- a preparer/reviewer;
- a PBC request;
- a finding;
- a conclusion;
- a controlled-evidence capture;
- an audit sign-off.

It may optionally link to a client, engagement, workpaper, or controlled-evidence version later.

### 4.3 Normal Data Mode must remain compatible with evidence promotion

The mode should have a clear lifecycle:

```text
Ordinary linked/managed data
        |
        v
Normal Data Workspace
        |
        +-- preview / filter / compare / transform / reconcile
        +-- save deterministic recipe
        +-- save exact run history
        +-- export working result
        |
        +-- optional later promotion/link
                |
                +-- exact run/input bound to engagement
                +-- controlled evidence captured if policy requires
                +-- audit/compliance workpaper can reference exact version/run
```

A normal working dataset is **not automatically controlled audit evidence**.

Promotion into an audit/compliance context must be explicit and must bind the exact source/run/version used.

---

## 5. Recommended product-mode architecture

Avoid a finite schema-level enum that assumes only the currently known modes will ever exist.

Prefer a capability/profile architecture.

Conceptually:

```text
Common platform capabilities
  SEARCH
  VIEW_DOCUMENTS
  LINK_FILES
  MANAGED_FILES
  DATASETS
  DATA_RECIPES
  DATA_COMPARISON
  EXPORT

Optional specialist capabilities
  CONTROLLED_EVIDENCE
  WORKPAPERS
  REVIEW_NOTES
  SIGNOFF
  LEDGER_SCRUTINY
  TB_LINKAGE
  COMPLIANCE
  INTERNAL_AUDIT_FINDINGS
  DUE_DILIGENCE
```

A workspace/profile selects a suitable capability set.

Possible initial profiles:

```text
NORMAL_DATA
STATUTORY_AUDIT
INTERNAL_AUDIT
DUE_DILIGENCE
COMPLIANCE
```

But do not encode those as the only possible future profiles.

### Migration strategy

Do **not** perform a risky full rewrite of all existing engagement tables just to obtain a common `workspace` superclass.

A safer progression is:

1. introduce a Normal Data workspace/domain slice alongside existing engagement domains;
2. keep shared document/storage/search identities reusable;
3. factor truly common data-processing components underneath both normal and specialist modes;
4. only introduce a broader shared workspace abstraction later if real duplication proves it valuable.

---

## 6. Normal Data Mode — minimum functional scope

### 6.1 Data workspaces

A user should be able to create a normal data workspace with:

- name;
- description;
- optional client link;
- optional period;
- optional tags;
- created/updated timestamps;
- no mandatory engagement.

Examples:

- “October GST comparison”
- “Vendor master cleanup”
- “FY 2025-26 sales analysis”
- “Bank transaction review”
- “2A filing-month comparison”
- “Management MIS working”

### 6.2 Dataset sources

Normal Data Mode should support datasets originating from:

- linked XLSX/XLS/XLSM/XLSB/ODS where supported;
- CSV;
- text-delimited sources;
- managed imported files;
- exact controlled-evidence files when intentionally selected;
- later: database/API/accounting-system connectors.

Never require source-file duplication merely to analyze linked data.

A dataset version/run must retain enough provenance to identify exactly what source content was used.

### 6.3 Explicit semantic columns

Do not silently assume that a date field has one universal meaning.

Datasets may contain:

- invoice date;
- posting date;
- filing date;
- return period;
- accounting period;
- receipt date;
- due date;
- transaction date;
- document date.

The user or an explicit import profile must be able to assign semantic roles.

This matters especially for comparisons.

### 6.4 Period-basis rule — critical acceptance requirement

When a source summary is filing-period based, comparisons must be filing-period based.

Do not substitute invoice month merely because an invoice date is present.

Example:

```text
Summary basis: August filing period

Invoice dated: July
Reported/filed in: August

Correct comparison bucket: August
Invoice month remains transaction metadata.
```

The comparison engine must preserve both concepts.

For a GSTR-2A-style comparison, support an explicit basis such as:

```text
Comparison period = Filing Month
Secondary attribute = Invoice Month
```

A result should be capable of explaining:

- present in both datasets in the same filing period;
- present only in dataset A;
- present only in dataset B;
- same invoice moved between filing periods;
- same key with taxable-value difference;
- same key with tax-component difference;
- amended/revised values;
- duplicates;
- unmatched records.

This principle is generic: **the user-selected business period determines comparison grouping, not an inferred date field.**

### 6.5 Deterministic operations

The first useful data engine should support deterministic operations such as:

- select/remove columns;
- rename columns;
- explicit type conversion;
- filter;
- sort;
- calculated columns;
- grouping;
- aggregation;
- pivot/unpivot;
- join;
- union/append;
- deduplication;
- key-based comparison;
- period-based comparison;
- numeric tolerance;
- reconciliation;
- duplicate detection;
- difference classification.

Do not make AI the calculation engine.

### 6.6 Saved recipes and runs

Separate:

```text
RECIPE
  definition of deterministic operations

RUN
  execution of one recipe
  against exact input dataset versions
```

A saved run should retain:

- recipe version;
- exact input identities;
- source hashes/fingerprints where available;
- parameters;
- row counts;
- deterministic result summary;
- execution timestamp;
- output identity/hash where useful.

If a linked source later changes, an earlier reproducible run must not silently become a different result.

### 6.7 Working outputs versus evidence

Normal-data results may be:

- transient preview;
- saved derived dataset;
- exported CSV/XLSX;
- report/table;
- chart;
- comparison result.

They are ordinary working outputs until intentionally promoted or linked into an engagement/evidence workflow.

---

## 7. Normal Data Mode — UI direction

The UI should make the distinction obvious without fragmenting the application.

A possible home structure:

```text
Home
  |
  +-- Search / Documents
  +-- Normal Data
  +-- Clients & Engagements
       |
       +-- Statutory Audit
       +-- Internal Audit
       +-- Due Diligence
       +-- Compliance
```

Universal search, recent items, pins, viewer, and document relationships remain common.

### Normal Data workspace screen

Minimum useful layout:

```text
Dataset sources
  -> Preview
  -> Columns / semantic roles
  -> Operations / comparison builder
  -> Result table
  -> Difference summary
  -> Saved recipes
  -> Run history
  -> Export / Link to engagement
```

Avoid audit terminology in this mode unless the user explicitly links the workspace to an audit/compliance workflow.

---

## 8. Normal Data Mode — architecture invariants

The next architect should treat these as acceptance constraints.

1. **No engagement required.**
2. **No audit vocabulary required.**
3. **No arbitrary frontend filesystem paths.**
4. **No forced controlled-evidence capture.**
5. **Linked source files remain authoritative working sources unless explicitly imported/captured.**
6. **Deterministic calculations are reproducible.**
7. **Saved runs bind exact input versions/fingerprints.**
8. **Critical period basis is explicit, not silently inferred.**
9. **Invoice month and filing month remain distinct when both exist.**
10. **AI can explain/suggest mappings but must not silently change deterministic results.**
11. **Normal data can later be promoted/linked to a specialist engagement without losing provenance.**
12. **Normal-mode tables/recipes must not be designed specifically for GST, audit, or one service line.**

---

## 9. Recommended next implementation sequence

Before adding more specialist breadth, create the general-data substrate.

### Step 1 — ADR

Add an ADR covering:

- workspace/profile concept;
- Normal Data Mode;
- separation of generic data operations from specialist workflows;
- ordinary working data vs controlled evidence;
- deterministic recipe/run model;
- explicit semantic-period model.

### Step 2 — Development plan update

Update `DEVELOPMENT_PLAN.md` so Normal Data Mode is not hidden inside audit phases.

Recommended wording:

> The application is a common professional document/data platform. Audit, internal audit, due diligence and compliance are specialist workspace profiles layered on common search, document and deterministic-data capabilities. Normal Data Mode is a first-class profile and must remain usable without an engagement.

### Step 3 — Backend foundation

A first logical unit may introduce concepts equivalent to:

- data workspace;
- dataset;
- dataset source/version;
- column schema/semantic roles;
- deterministic recipe;
- recipe version;
- run;
- run input;
- result summary.

Do not overdesign the full spreadsheet engine in the first PR.

### Step 4 — First end-to-end comparison

Implement one broadly useful deterministic comparison slice:

```text
Dataset A
Dataset B
  -> choose comparison key(s)
  -> choose period basis
  -> choose numeric comparison columns
  -> exact deterministic match/difference classification
  -> saved run
  -> reopen run
```

The comparison must support the filing-period versus invoice-period distinction described above.

### Step 5 — UI

Add a visible Normal Data entry and a thin workflow for:

- create workspace;
- attach/select two datasets;
- map columns;
- select period basis;
- run comparison;
- inspect differences;
- reopen exact run.

### Step 6 — Promotion/linking

After the generic workflow is stable, add optional linking:

- normal dataset/run -> engagement;
- normal result -> workpaper;
- exact source/run -> controlled evidence when explicitly required.

---

## 10. Security and authority status

Do not overclaim authorization maturity.

The application contains role/actor/sign-off concepts and audit history, but the next architect must distinguish:

- recorded actor/role metadata;
- workflow-state validation;
- actual authenticated authority enforcement.

Do not describe recorded actor strings as strong identity/authentication unless a real authority boundary exists.

Future shared/firm deployment still requires centralized authentication/RBAC and a service architecture rather than shared SQLite on a NAS.

---

## 11. Deterministic accounting/data principles

Keep one rule across both Normal Data and audit modes:

> Same exact inputs + same exact deterministic recipe/parameters = same accounting/data result.

For specialist accounting:

- AI must not change balances;
- AI must not clear deterministic exceptions;
- mappings and reconciliations remain rule-driven;
- exceptions retain source traceability.

For Normal Data:

- AI may suggest keys, data types, column mappings, period semantics, or explanations;
- the user must be able to see/confirm material mapping assumptions;
- deterministic operations produce the official result;
- saved run parameters remain inspectable.

---

## 12. Current due-diligence handoff detail

PR #63 merged the immutable DD report builder foundation at schema v24.

Its model includes:

- immutable report identity;
- immutable report versions;
- exact links to the latest immutable due-diligence issue event at publication time;
- snapshot hashing;
- historical report versions that do not rewrite when issues later change.

PR #64 provides the corresponding internal UI and was green/open at the time of this document.

Keep external publication/external permission semantics out of scope until explicitly designed. The current DD report content is internal application data.

---

## 13. What not to do next

Do not:

- turn Normal Data Mode into another audit engagement type;
- require workpapers for generic analysis;
- automatically capture every analyzed file as controlled evidence;
- assume invoice month is always the correct period for GST/tax comparison;
- embed one GST form's schema as the generic data engine;
- build comparison logic only in React;
- make AI-generated matching the authoritative reconciliation result;
- accept arbitrary filesystem paths from the frontend;
- rewrite old immutable evidence or run history;
- add a broad dependency/schema expansion when a smaller deterministic slice proves the design.

---

## 14. Suggested acceptance tests for Normal Data Mode

### General workspace

1. User can create a data workspace without a client or engagement.
2. User can optionally link it to a client later.
3. No preparer/reviewer/sign-off fields are required.

### Dataset handling

1. A linked workbook can be selected through existing indexed identity without sending an arbitrary path.
2. The source can remain linked rather than copied.
3. A saved run identifies the exact source content/fingerprint used.
4. A later source change does not silently rewrite the historical run.

### Period semantics

Given:

```text
Invoice: INV-100
Invoice date: 2026-07-29
Filing period in dataset A: 2026-08
Filing period in dataset B: 2026-08
```

the record is compared in the August bucket when the selected period basis is filing period.

Given:

```text
Dataset A filing period: 2026-08
Dataset B filing period: 2026-09
Same invoice key
```

the result should be capable of classifying it as a period movement rather than simply “missing from A / new in B”.

### Numeric differences

For the same configured record key, report deterministic differences in selected numeric fields, including taxable value and tax components where those columns are chosen.

### Audit promotion

A saved normal-data run can later be linked to an engagement without changing the original run definition or result.

---

## 15. Near-term handoff checklist for the next architect

1. Fetch live `develop`; do not rely only on this SHA.
2. Reconcile PR #64:
   - if already merged, verify final commit;
   - if still open, re-check exact head/base/CI/Security/reviews before merging.
3. Read `DEVELOPMENT_PLAN.md`.
4. Add the Normal Data Mode ADR.
5. Update the development plan to recognize Normal Data as a first-class mode.
6. Design the smallest deterministic generic-data workspace foundation.
7. Build a two-dataset comparison workflow with explicit period semantics.
8. Keep all existing evidence/filesystem invariants intact.
9. Keep specialist audit/internal-audit/DD modes as optional overlays on the shared platform.
10. Only after this foundation is proven should the architect resume broader specialist/AI scope.

---

## 16. Product statement to preserve

Professional DocX should evolve toward:

> **A secure professional document and data workspace that is excellent for ordinary work, and becomes a rigorous audit, compliance, internal-audit, or due-diligence platform when those specialist controls are needed.**

The system should be useful even when the user is not conducting an audit.

That is now a core architectural requirement, not a future nice-to-have.
