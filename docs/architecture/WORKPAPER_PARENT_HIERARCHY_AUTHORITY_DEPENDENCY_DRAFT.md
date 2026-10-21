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


## Alternative creation paths and template-derived hierarchy (source-inspected 2026-10-21; DECISIONS OPEN)

This expands, **without replacing**, the three-command parent hierarchy table above. All five commands listed here are registered in `tauri::generate_handler!`. Each currently takes webview-supplied business inputs and database state; none takes an independently verified native person, grant or professional reviewer. Do **not** equate local-webview accessibility with demonstrated Internet exploitation.

| Registered command | Native facts / transaction | Outstanding authority and custody boundary |
| --- | --- | --- |
| `create_client` | `normalize_domain_label` (nonempty, ≤200 characters), native UUID and direct SQLite `INSERT` through configured connection (single-statement atomicity; **no explicit IMMEDIATE transaction**); no domain audit insert | Anyone able to invoke the existing command under the installed boundary can submit a client name; no trusted client-creation or tenant provisioning grant |
| `create_service_type` | Nonempty ≤120-character normalized label, `normalize_search_text` must not be empty; native UUID and configured-connection direct `INSERT` (no explicit IMMEDIATE transaction), no domain audit insert | No independent firm-wide service taxonomy administrator or provisioning authority |
| `create_engagement_template_from_engagement` | SQLite `IMMEDIATE`; requires live source engagement, serializes active areas/procedures, rejects active procedures whose area is not in the active area set; creates template and immutable-looking version-1 **database rows** atomically | Snapshot operation checks source existence/relationships, not a native `READ_SOURCE_ENGAGEMENT_HIERARCHY` plus `PUBLISH_TEMPLATE` permission; database version rows are not authenticated evidence custody |
| `create_engagement_template_version_from_engagement` | SQLite `IMMEDIATE`; requires live destination template and source engagement; source service type must match latest template version; next sequential version inserted atomically | No issuer-bound template administration or source-read authority, no human review of an updated hierarchy; version rows are historical records, not professionally approved templates |
| `create_engagement_from_template` | SQLite `IMMEDIATE`; requires live destination client, live template parent and template version, live version service type; parses stored template JSON, inserts a new engagement and recreates nested areas/procedures with freshly generated IDs under one commit; rejects an unresolvable area parent or procedure area | **Destinations are caller-selected clients; source template client does not bind destination authority.** A caller-selected engagement `status` is a normalized string, not a verified review state; no principal/grant or creator audit event. No workpapers, seven-field revisions, evidence links or sign-offs are copied by this path |

**Two distinct provenance boundaries:** First, `engagement_template_versions.definition_json` is SQLite-held application content; despite historical version rows, this is not captured, externally authenticated or access-controlled formal evidence. Second, a copied engagement/area/procedure receives new native IDs, so any future grants must be independently issued for the new destination context. Neither source read access nor possession of a template version ID may bootstrap unrelated-client creation, workpaper grants, evidence custody or professional review.

**Additional adjacent functions are NOT covered by this bounded characterization:** `create_firm_library_item` and `publish_firm_library_version` are also registered, use `IMMEDIATE` and write domain audit events, but their caller trust, definition injection and downstream template consumption are not yet fully traced. Other source hierarchy mutation, archival and client/service reassignment endpoints also remain outside this audit. Do not interpret this section as an exhaustive mutable-parent inventory or authorization certification.

### New test-only characterization, not security remediation

The separate `#[cfg(test)]` Rust module [`workpaper_template_hierarchy_native_tests.rs`](../../src-tauri/src/workpaper_template_hierarchy_native_tests.rs) uses disposable, actual-production-migration SQLite and calls production persistence functions. Four tests cover: (1) legacy client/service creation and invalid empty labels with no additional audit counts; (2) existing **cross-client template instantiation** with nested parent IDs correctly remapped, a procedure linked to the remapped child, source workpaper left at source, no cloned workpaper, and unverified caller `REVIEWED` engagement status; (3) malformed stored template area-parent and procedure-area references, each rejected after potential staged inserts with unchanged aggregate engagement/area/procedure/audit counts; (4) different-service source denied when adding a template version, with same-service version increment preserved. Fixture-only tampering of a template JSON payload is **not** a production UI capability or a claim of threat reachability.

The extended [source-only Node tripwire](../../tests/workpaper-parent-hierarchy-boundary.test.mjs) pins registration, existing scope/transaction checks and nonshipping test placement. Neither suite invokes actual registered Tauri handlers nor supplies a verified principal, revocation policy or human signer. Record numerical test outcomes **only after exact-HEAD CI completes**.

### Deferred independent-native acceptance (ALL NOT IMPLEMENTED / NOT RUN)

- `create_client` and `create_service_type`: absent/expired/revoked firm-level provisioning permissions, issuer outage, actor spoofing, name collision and exact audit/refusal invariants.
- Source engagement → template snapshot/version: wrong source-engagement read rights, foreign tenant/client, wrong service, archived source/template, stale source hierarchy, concurrent version publication, malicious definition and rollback without leaked data.
- Template version → engagement: wrong client grant, unrelated-client template reuse, revoked engagement-creation permission, untrusted template definition, wrong service or archived entities, nested area/procedure ancestry spoofing, malicious initial `REVIEWED`/approval label, revalidation under the write lock, zero-write denial and no accidental copying of workpaper/evidence/sign-off histories.
- Real registered-command denial tests must use a DEC-03/04-approved issuer with qualified independent DEC-06 review, not a fixture-supplied role or invented first administrator.

**Count boundary is unchanged:** original workpaper inventory = **nine** callable writers and **34 direct-Tauri cases, all `execution: NOT_IMPLEMENTED`, `result: NOT_RUN`**. The additional eight parent/alternative creator routes are **separate**, and none of their actual authenticated direct-handler denial vectors has been implemented or executed. No DEC-01…08 selection, no L1–L4 decision, no production grant, no user-data migration, no signed professional status, no trusted Save or release authorization is implied.
