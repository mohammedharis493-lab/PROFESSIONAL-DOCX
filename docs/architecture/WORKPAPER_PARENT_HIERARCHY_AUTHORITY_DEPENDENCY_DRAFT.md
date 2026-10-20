# Parent-hierarchy mutation authority and scope dependency (DRAFT)

**Prepared:** 2026-10-20. **Status:** source inspection and disposable **test-only** native regressions; NOT an approved ADR, independent security review, authorized identity/grant, deployed native policy, penetration test or release gate clearance. All DEC-01…08 in [#129](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/issues/129) remain unresolved; L1–L4 in [#138](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/issues/138) remain unselected. [PR #137](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/pull/137) remains draft/unmerged.

## Existing registered hierarchy creation (source verified at `062828c8d109d9532d6c1e34c26804b9cc6e5f2e`)

The three `#[tauri::command]` functions in [`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs) are in `tauri::generate_handler!`; the frontend is NOT their only potential webview caller. Their existing handlers validate UUID **shape** (and optional ID shape), forward the caller-provided fields and database path, and do **not** take a trusted session, issuer assertion or grant. Tauri exposure is desktop/webview command availability, **not** proof of remote Internet exploitability; DEC-02 must determine the threat model.

| Existing command / source | Parent and user-controlled fields | Native persistence checks inside an `IMMEDIATE` transaction | Limitations / unresolved authority |
| --- | --- | --- | --- |
| [`create_engagement`](../../src-tauri/src/persistence.rs) (`lib.rs` ~2716; `persistence.rs` ~5758) | `clientId`, `serviceTypeId`, `name`, optional `periodStart`/`periodEnd`, `status` default `ACTIVE` | Requires live (not `archived_at_ms`) client and service-type rows; normalizes name/status and optional period text; generates native ID and inserts engagement atomically | No independently verified creator, client/service authorization or linked permission bootstrap. Optional period text is length-normalized, not checked here for parseable chronological date ordering. No `audit_events` insert in this implementation |
| [`create_engagement_area`](../../src-tauri/src/persistence.rs) (`lib.rs` ~4150; `persistence.rs` ~13837) | `engagementId`, optional `parentAreaId`, `name`, `code`, `displayOrder`, `status` default `ACTIVE` | Requires live engagement; optional live parent area must have **the same engagement ID**; normalizes name/status and optional code; generates ID and inserts area atomically | No trusted engagement-scoped creator. No inheritance of verified authority from parent, no validation of the caller's permission to create nested areas, and no `audit_events` insert |
| [`create_procedure`](../../src-tauri/src/persistence.rs) (`lib.rs` ~4186; `persistence.rs` ~13963) | `engagementId`, optional `engagementAreaId`, `reference`, `title`, `description`, `status` default `ACTIVE` | Requires live engagement; optional live area must have **the same engagement ID**; normalizes title/status/reference/description; generates ID and inserts procedure atomically | No issuer-bound engagement permission or individual procedure grant, and no `audit_events` insert |

**Important distinctions.** An `IMMEDIATE` write lock protects atomicity and ancestry checks against ordinary concurrent SQL writers; it does **not** establish professional authority, issuer revocation semantics, audit custody or global tamper resistance. For these three commands, `status` is accepted as a normalized nonempty label (length bound), not as independently authenticated approval. `archived_at_ms IS NULL` is the native live-row test; it must not be described as a broader permission/status policy. Existing direct caller input may identify other existing parents, but parent existence is not a user grant. The lack of domain audit inserts is a **source observation**, not an authorization to invent or backfill professional audit identities.

**Other adjacent mutators are not comprehensively audited here.** In particular, template-derived engagement creation / template publishing, archive/update operations, any client/service-type creation and hierarchy reassignment require separate scope inventory and sign-off. The **nine** workpaper writers documented in the [unchanged deferred direct-Tauri matrix](WORKPAPER_DIRECT_TAURI_DENIAL_ACCEPTANCE_MATRIX_DRAFT.json) are not a complete parent-hierarchy inventory.

## Proposed, UNSELECTED grant hierarchy — no implementation

- A future `CREATE_ENGAGEMENT_FOR_CLIENT_AND_SERVICE`-like authority (placeholder only) would have to reference an approved client/service context and actual verified subject **before** a new engagement ID exists. Product/security must resolve service-type applicability, tenant/client separation, provisioning and grant bootstrap (DEC-03/04).
- A future `CREATE_AREA_IN_ENGAGEMENT` / `CREATE_PROCEDURE_IN_ENGAGEMENT`-like authority (placeholders only) would require current issuer-bound engagement scope under the final writer lock, not merely access to an optional parent-area or the ability to provide a UUID. Nested area policy, parent archival/scope changes and service/engagement revocation are OPEN.
- Engagement creation must not auto-bootstrap global administrator, creator `READ_WORKPAPER`/`REVISE_WORKPAPER`, professional reviewer, signer, workpaper creation, evidence-custody or downstream edit rights. Separate DEC-04-controlled grants and an explicit DEC-08 initial status/workflow policy would be required.
- Any approved implementation needs denial before mutation with uniform information disclosure for unauthorized foreign parents, atomic authorization+lineage rechecking, session expiry/revocation behavior, audit policy, migration/backups, legacy compatibility, rollback and read/export preservation. No current DEC-03 issuer, DEC-04 admin bootstrap, or DEC-06 independent review exists.

## Tests added versus security tests still NOT RUN

The `#[cfg(test)]` Rust module [`src-tauri/src/workpaper_parent_hierarchy_native_tests.rs`](../../src-tauri/src/workpaper_parent_hierarchy_native_tests.rs) calls **production persistence functions** against a disposable database initialized with **actual production migrations**. It characterizes:

1. Missing client/service rejection and a legacy engagement create requiring no authenticated person argument, with row/audit count checks.
2. Cross-engagement parent-area rejection and same-engagement child-area success, checking unchanged rows on denial.
3. Cross-engagement procedure-area rejection and same-engagement procedure success, checking unchanged rows on denial.
4. Archived parent area/engagement rejection for area/procedure creation, checking unchanged row counts.

These are **existing lineage/integrity characterization tests**, not an approved security deny-all harness. They do **not** invoke the registered Tauri handlers, exercise a real issuer, test revoked rights or malicious-webview principal spoofing, establish independent audit review, or demonstrate production containment.

**Deferred direct-handler acceptance vectors — ALL NOT IMPLEMENTED / NOT RUN (separate from the existing 34):**

| Parent command | Required future registered-handler checks | Status |
| --- | --- | --- |
| `create_engagement` | Missing verified session; foreign client/service context; expired/revoked grant; caller status/period spoofing; concurrent authorization drift; unauthorized error disclosure and zero writes | **NOT IMPLEMENTED / NOT RUN** |
| `create_engagement_area` | Missing session; foreign engagement/parent area; revoked create privilege; archived-parent concurrency; status spoofing; issuer offline; zero rows/audit changes on denial | **NOT IMPLEMENTED / NOT RUN** |
| `create_procedure` | Missing session; foreign engagement/area; revoked privilege; archived-parent concurrency; status spoofing; issuer offline; zero rows/audit changes on denial | **NOT IMPLEMENTED / NOT RUN** |

The original workpaper matrix remains **nine registered writers / 34 unimplemented and unrun direct-Tauri denial cases**. Do not count this new real-migration suite as running any of those 34 or as covering the new parent-handler acceptance vectors. CI success is not independent human security approval.

## Release hold

Leave the three existing parent commands and all nine inventoried workpaper writers callable, preserve immutable revision/evidence/sign-off history and the detached seven-field editor, do not implement identity or professional Save/signing, do not authorize DEC-01…08 or select L1–L4, and do not merge PR #137 or close #129/#138. Product/security must resolve parent/child hierarchy authorization and compatibility before a separately reviewed production change.
