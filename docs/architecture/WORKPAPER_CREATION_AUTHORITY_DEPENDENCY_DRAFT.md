# Workpaper creation authority: parent-engagement grant and initialization dependency (DRAFT)

**Prepared:** 2026-10-19. **Status:** source-inspected review input ONLY; not an accepted ADR, selected identity architecture, human security review, production native grant, installed command change or release permission. This draft extends, but does not decide, DEC-02/03/04/06/08 in [Issue #129](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/issues/129) and legacy containment options in [Issue #138](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/issues/138). [Draft PR #137](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/pull/137) must remain open, draft, and unmerged.

## 1. Newly identified authorization dependency

**`CREATE_WORKPAPER_IN_ENGAGEMENT` (DESIGN PLACEHOLDER, NOT AN EXISTING GRANT)** is scoped to a real **parent engagement** rather than an as-yet-nonexistent workpaper. A newly generated `workpaper_id` cannot carry an existing `READ_WORKPAPER` or `REVISE_WORKPAPER` grant that independently authorizes its own creation. This avoids circular first-resource bootstrap. A future creator's right to create workpapers in one engagement must not convey automatic permission to read, revise, clear notes, review or professionally sign *any* newly created workpaper. Workpaper grants, if any, require independently approved issuance, revocation and auditing, with no first-launch admin or self-issued privilege.

**Source-verified current state** at [prior green head `bf0eada45f13dec9a2b7a887df5b058197daf75d`](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/commit/bf0eada45f13dec9a2b7a887df5b058197daf75d):

- [Native `create_workpaper` Tauri handler](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/blob/bf0eada45f13dec9a2b7a887df5b058197daf75d/src-tauri/src/lib.rs#L4222-L4245) is **registered** in [`generate_handler!`](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/blob/bf0eada45f13dec9a2b7a887df5b058197daf75d/src-tauri/src/lib.rs#L5321-L5347). It accepts caller-supplied `engagementId`, optional `engagementAreaId/procedureId` and `workflowState`; omission chooses `NOT_STARTED`. It has ID-shape checks, not an issuer-bound person or engagement creation grant.
- [Actual native persistence](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/blob/bf0eada45f13dec9a2b7a887df5b058197daf75d/src-tauri/src/persistence.rs#L14094-L14218) opens an `IMMEDIATE` transaction, verifies that the engagement exists and is active and that optional area and procedure belong to that engagement, then writes a workpaper and `WORKPAPER_CREATED` audit event. It normalizes `workflow_state` as nonempty text **without requiring that a client-provided `REVIEWED` value corresponds to a verified review**.
- Existing review/sign-off rows are not created automatically by this command. Consequently a stored creation-time `REVIEWED` label is an *unverified workflow-state string*, not independent professional approval. The new isolated native test records precisely that current behavior; it does NOT recommend keeping or authorizing it.
- `create_engagement_area`, `create_procedure`, and `create_engagement` are adjacent registered parent-resource mutators. Their trusted person/grant scopes have **not been independently audited/approved** and remain outside this nine-command deferred denial matrix. A hierarchy-wide policy must account for them; do not call nine routes exhaustive.

## 2. Conditional native creation contract (NOT IMPLEMENTED)

Only **after** accountable human decisions and independent security approval:

1. Obtain a verified native session from the approved issuer; neither the Tauri webview payload, local SQLite `actorId`, OS username nor a self-declared role can be the authority.
2. Enter the final SQLite `BEGIN IMMEDIATE` transaction; resolve current engagement ownership, archive/disable state and relevant branch; check current live **engagement-scoped create permission**, expiry, revocation and a trusted issuer state. If the issuer/session cannot be revalidated with an approved coherent consistency model, fail closed.
3. Independently validate optional area/procedure lineage in the same transaction. Resolve the new workpaper ID natively and assign its **initial workflow state from server policy**, never a caller's purported review/approval. Record `NOT_STARTED` or an explicitly unverified draft state only if a human has selected it; neither is chosen here.
4. Write the workpaper and authenticated audit atomically; on denial, preserve workpaper counts, hierarchical relationships, workpaper state, and audit exact rows unchanged. No implicit creator grant, `REVISE_WORKPAPER` grant, reviewer role, sign-off, controlled evidence version, or retroactive authentication of old state labels.
5. To give the creator subsequent access, require a **separate** DEC-04-approved grant-administration workflow, potentially staged within an explicitly reviewed coherent issuance protocol. Grant issuance cannot be justified by the newly created workpaper row alone. Revoke/expiry/switch must be enforced at each later operation.
6. Keep read/export of old workpapers and unverified historical labels compatible with a human-approved legacy containment option (L1–L4). No default policy, destructive migration, first-user auto-admin or silent reinterpretation of old records.

**Design questions for DEC-03/04/08 (all OPEN):** trusted person verification; enrollment/admin bootstrap and separation; whether any creator gets default access and by whose *independent* authority; hierarchy-grant revocation on engagement archive/move; exact workflow initial state; who can transition to `REVIEWED`; whether and how offline grants work; audit storage trust and backup/rollback; how to preserve legacy local-draft UX without mislabeling professional sign-off.

## 3. Security threats and conditional tests

| Scenario | Required acceptance evidence, AFTER decision | Current result |
| --- | --- | --- |
| Missing verified session on `create_workpaper` | Registered native handler must refuse before mutation without leaking a foreign engagement's existence | **NOT IMPLEMENTED / NOT RUN** |
| Valid user without creator grant on chosen engagement | Deny both foreign-engagement attempts and same-OS-user UI payload impersonation | **NOT IMPLEMENTED / NOT RUN** |
| Expired or revoked creation grant | Deny under actual final writer lock; prove revocation/issuer outage semantics and concurrent writers | **NOT IMPLEMENTED / NOT RUN** |
| Caller-provided `workflowState: REVIEWED` or `FINAL_APPROVAL` | Deny or server-override according to separately approved policy; no false audit, historical approval or auto-grant | **NOT IMPLEMENTED / NOT RUN** |
| Foreign area/procedure selected under another engagement | Existing real native lineage check must preserve all data on denial; repeat against approved direct Tauri handler | **Existing persistence characterization tested; new trusted-handler denial NOT RUN** |
| Historical workpaper list/read/export across rollout | Preserve every old revision and original caller-asserted workflow/sign-off provenance; test backups, rollback and offline installs | **NOT IMPLEMENTED / NOT RUN** |

The [deferred direct-Tauri matrix](WORKPAPER_DIRECT_TAURI_DENIAL_ACCEPTANCE_MATRIX_DRAFT.json) now includes **nine known workpaper mutation commands** and **34 still-unexecuted registered-handler denial scenarios** (four new creation-time vectors). Its grant-category strings are design placeholders. The existing isolated deny-all **test-only SQLite probe** now enumerates a ninth *intent*, not a ninth authorized production command. New native real-migration tests merely characterize caller-selected initial state and rejection of foreign procedure scope. No direct registered handler was called by those tests.

**Product invariant maintained:** an independently authenticated editor should eventually be able to change even one character in an already-reviewed workpaper without reviewer unlock, saving an *unapproved* immutable successor while the signed predecessor remains historical. This creation-time dependency must never be used to disable legitimate revision editing or auto-approve the successor.

## 4. Human handoff and release stop

A human product owner and security owner must explicitly select DEC-02/03/04/08 outcomes; DEC-06 requires a named **independent** qualified reviewer, unavailable here. DEC-01/05/07 additionally govern any controlled-evidence bytes or retention promise. Review must settle engagement creation authority, grants and revocation, initial workflow labels, offline failures, rollout/backups and hostile-webview threat assumptions. Record chosen alternative, named owner, platform scope, evidence, residual risks and rollback before converting these proposals into native migration/handlers.

**Do not** install a grant table/issuer, create a first admin, disable existing commands, merge PR #137, close Issues #129/#138, or mark the 34 unrun denial tests as passing on the strength of this source inventory, isolated fixture tests, or CI.
