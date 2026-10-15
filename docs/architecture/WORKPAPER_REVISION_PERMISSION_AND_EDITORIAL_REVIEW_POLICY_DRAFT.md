# Draft: Revision permission, editorial corrections, and sign-off validity

- **Status:** Proposed / non-authorizing; **not** an accepted ADR or a DEC-08 decision.
- **Date:** 2026-10-15
- **Source of product direction:** Users granted workpaper revision permission should be able to revise freely, including small changes, without requesting an unlock from a former signer.
- **Blocking decisions:** DEC-03 (trusted individual identities), DEC-04 (trusted grant bootstrap), DEC-06 (independent acceptance), DEC-08 (sign-off, revision eligibility, separation of duties); see [Issue #129](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/issues/129).
- **Invariant:** [Accepted ADR-0007](ADR-0007-evidence-capture-and-signoff-binding.md) continues to govern exact immutable revision and evidence-version sign-off binding. Neither this proposal nor merging its documentation authorizes Normal Data D2/D3/D4.

## Product rule: editing is not approval

A person with a trusted **ReviseWorkpaper** permission may open, edit, and save a new revision of a workpaper even when a previous revision has an active historical sign-off. There is **no lock/unlock gate** imposed by sign-off.

**Saving a change does not approve it.** The immutable prior revision and its exact prior sign-off/evidence set remain historically available. A newly saved revision is independently identified and does not inherit the old sign-off, whether the change is minor or material.

Do not use an arbitrary client-provided `actor_id`, `actor_role`, subject UUID, OS username, workspace membership or self-labeled "minor" classification as trusted authority. No currently implemented human session/permission issuer is sufficient to activate this multi-user policy.

## Distinct permissions, scopes, and checks

| Capability (proposed semantic name) | Scope | Allows | Does NOT confer |
| --- | --- | --- | --- |
| `ViewWorkpaper` | workspace/workpaper according to read policy | View permitted work and revision history | Revision or sign-off |
| `ReviseWorkpaper` | **workpaper identity** + engagement/workspace context | Append a successor workpaper revision when workpaper content is separately readable | Read access by itself, discretionary evidence attachment, independent review, sign-off, admin |
| `AttachEvidenceToRevision` | **exact revision** and eligible engagement | Attach permitted exact controlled evidence versions | Revision-writing or sign-off automatically |
| `ReviewWorkpaper` | eligible engagement/revision | Raise/clear notes and inspect exact revisions under policy | Sign-off unless separately granted |
| `ApproveWorkpaperRevision` | eligible exact revision and sign-off type | Record independently authorized review/final sign-off | Rewrite old sign-offs, revise or administer grants |
| `AcknowledgeEditorialCorrection` | eligible successor revision | Perform narrowly scoped, accountable expedited editorial approval if DEC-08 accepts it | Blanket sign-off, material-change approval, self-approval without explicit policy |
| `ManagePermissions` | controlled administrator scope | Enroll/revoke subject grants after trusted bootstrap | Automatically revise/review/sign work |

These are **proposed policy concepts**, not current production APIs or grants. The existing Normal Data test-only `ModifyWorkpaperRevision` boundary is scoped to an **exact existing revision** for D3 operations; it cannot simply be reused as a broad, persistent workpaper editing permission. In particular, granting revision creation must not silently grant evidence-attachment rights on new revisions.

To keep permission assignment simple for real users, the administrative UI may offer an **Editor access profile** that issues separately recorded `ViewWorkpaper` + `ReviseWorkpaper` grants after trusted administrator authorization. A backend must still check read authority before returning workpaper content and check revision authority in the final append transaction. **Revise alone must not be a hidden read-data bypass**; bundling grants for UX is not permission inheritance.

Before every revision append, trusted native code must:
1. Bind the verified person/session to the exact workpaper and engagement.
2. Check the distinct workpaper-scope revise grant and its revocation in the **final SQLite write transaction**; no cached frontend authority.
3. Check archived/eligible engagement and workpaper states under DEC-08, without interpreting prior sign-off as a write lock.
4. Require `expected_latest_revision_id` (or an equivalent monotonic version) and refuse stale writes with a human-readable comparison/merge workflow. Never silently overwrite a concurrent editor's revision.
5. Append the new immutable revision, reason, authenticated actor attribution, exact content hash and audit event atomically in SQLite; preserve old revision/evidence/sign-off records.
6. Create an explicit current review status for the successor; do not copy or infer an old `APPROVED` label.
7. Recheck independent review/approval rights at the sign-off boundary, including revocation and separation-of-duties policy.

If valid revise permission is absent, reject the mutation **without creating the revision**. The ability to revise is not an implicit right to export, delete, modify controlled evidence, or change authorization records.

## Implementation boundary: do not repurpose the existing exact-revision grant

This proposal requires a **separate, reviewed schema and authorization-adapter change**, not a new interpretation of an existing permission string. At the current baseline, the internal Normal Data permission registry (`src-tauri/migrations/0028_normal_data_permission_scopes.sql`) allowlists only `READ_NORMAL_DATA_WORKSPACE`, `ATTACH_EVIDENCE_TO_ENGAGEMENT`, and `MODIFY_WORKPAPER_REVISION`. The last applies to an **exact revision ID**, not a durable workpaper-level authoring privilege. Its existing purpose and default-deny behavior must remain unchanged.

Before implementing workpaper-wide editing, separately review a **versioned migration and grant policy** with these invariants:

1. Introduce a distinct workpaper-scoped revision-authoring capability, with a reviewed database key (for example `REVISE_WORKPAPER`), bound to the **workpaper UUID**, and do not reinterpret existing exact-revision grants.
2. No implicit grants for existing users, workspace owners, engagement members, reviewers, or administrators. Both new and existing accounts default to denial until a genuinely trusted issuer has enrolled and granted each person under DEC-03/04.
3. Define how administrators validate that an exact workpaper belongs to the expected engagement and workspace before issuing the new grant; do not accept a bare resource UUID as proof of scope.
4. At the final write transaction, read both the authenticated person's current grant state and the workpaper's latest revision. Refuse expired/revoked grants and stale `expected_latest_revision_id` values **before** creating an immutable successor or audit event. Return an explicit conflict for a user-friendly compare/retry rather than silently overwriting.
5. A new revision must have its **own exact evidence-link set** and content identity. It cannot inherit approval or independently authorize a discretionary evidence attachment by virtue of its author's workpaper-wide edit grant. Subject to the separately signed carry-forward policy below, the trusted backend may mechanically copy **unchanged exact version references** from the authorized predecessor into **new revision-owned immutable link records** without asking the editor to reattach every item. Additions, replacements, removals and changed version IDs are different operations subject to separately scoped evidence permissions and review.
6. Preserve existing revision rows and sign-offs exactly; derive or recompute the **current revision's** effective review status without rewriting historical approvals. Ensure a new successor cannot be shown as final merely because the workpaper row carries a previous state.
7. Make actor attribution and trusted revocation checks mandatory at the authoritative native/backend boundary; do not rely on a frontend form's identity, role, label, or description of a correction.

These are **future design requirements only**; do not write a permissive migration, provision any grants, or expose a production edit/review endpoint until the relevant human decisions and separate code reviews are complete.

## Easy everyday editing

The workpaper should offer ordinary **Edit → Save** for authorized revisers, including after review. No special "unlock signed workpaper" or mandatory preapproval step. Autosaved in-progress editor buffers may be used for usability, but they must be labeled *working/unapproved* and must not rewrite or masquerade as the immutable signed revision; checkpoint/Save creates a new immutable revision. Avoid one formal revision per keystroke.

On save, show a concise diff and a suggested **editorial** versus **material** classification. The author can provide a correction reason and propose "minor," but that classification is advisory: it cannot itself transfer sign-off, bypass mandatory review, or change custody state. Allow the work to remain saved and visible as an unapproved successor if no reviewer is immediately available.

### Low-friction carry-forward of unchanged evidence (proposed; not implemented)

**Do not make a typo correction require reattaching twenty unchanged files.** A simple Save should propose carrying forward the predecessor's **exact** evidence set if no evidence change was requested. Because `workpaper_evidence_links.evidence_link_id` belongs to a specific revision, the backend must create **new link IDs for the successor revision**, each referencing the **same exact controlled evidence-version/content-version ID, document ID, relationship type and description** as the predecessor. Never repoint the previous link or modify signed history. Preserve provenance from each new link to its predecessor link, and record the carry-forward in an append-oriented audit event.

This copying is a **narrow, system-mediated continuation of already-authorized exact references**, not a user-visible power to choose arbitrary evidence. Its legitimacy and circumstances are an open **DEC-08/DEC-05** policy question. If approved, the backend should:
1. Confirm trusted workpaper read + revise permissions, the predecessor is the expected latest revision, and both identities belong to the same eligible workpaper/engagement.
2. Load the predecessor's complete evidence-link set **inside the final transaction**; copy server-resolved exact version identities, never caller-submitted "unchanged" IDs, filenames, paths, or mutable `latest` pointers.
3. Create the immutable successor revision and its own frozen link records in that transaction, with a recorded source-link mapping, evidence-set digest, and auditable `UNCHANGED_EVIDENCE_CARRIED_FORWARD` reason.
4. Refuse any carry-forward presented as **formally review-eligible** if a source item lacks the required controlled version, is quarantined, not retained, inaccessible, or otherwise fails the independently defined proof; a draft-only carry-forward policy, if needed, must never imply formal evidence reliance.
5. Check for any difference between source and target evidence identities. If items are added, removed or changed, **do not silently use the carry-forward-only privilege**; require separately approved attachment/removal permissions, custody verification and the material-review path.
6. Preserve prior sign-offs and exact predecessor evidence as historical facts; the successor stays **unapproved** until an authorized current-revision acknowledgment or sign-off.
7. Fail the entire protected database append/audit operation atomically on missing links, grant revocation, concurrent base change, or an audit failure. No partial successor with misleading "approved" status.

Neither an editor's `ReviseWorkpaper` grant nor a frontend checkbox may independently grant broad evidence-attachment authority. Do not implement automated copying until accountable reviewers decide whether it is permitted, including the special case of a later-discovered invalid/quarantined evidence version. This proposal should save users repetitive manual steps **without loosening professional evidence custody**.

### Candidate classification (DEC-08 to decide)

| Example | Initial route | Approval meaning |
| --- | --- | --- |
| UI-only formatting/presentation change outside canonical signed content and evidence | No new signed-content revision if exact canonical bytes/evidence IDs truly remain unchanged; audit applicable presentation adjustments under product policy | Existing sign-off continues to bind unchanged signed content |
| Spelling, punctuation, typographical correction **inside** signed workpaper content | New immutable successor; fast editorial acknowledgment may be available | No prior sign-off automatically migrates; the authorized acknowledgment binds **exact successor revision** and exact evidence IDs |
| Correction to non-conclusion description whose materiality is uncertain | Save successor; flag for reviewer classification | Pending review until a reviewer decides |
| Amounts, exceptions, audit findings, risks, conclusion, procedures, population/sample, management explanation, controls or review-relevant statements | New successor and full policy-defined re-review | No active approval of successor until independently signed |
| Evidence add/remove/replace, changed content-version or controlled-evidence-version IDs | New revision with exact new evidence set; full re-review and custody authorization | Old review remains historical only |
| New material unresolved review note | Preserve history; apply decided re-review/suspension policy | Never silently imply active review of unresolved changes |

A "minor change" toggle, AI classifier, string similarity score or short diff is **not** proof of immateriality or professional authority. Until DEC-08 signs a narrow classification and reviewer-acknowledgment policy, default ambiguous text/data edits to **unapproved pending review**. Do not automatically grandfather old approvals onto new content.

### Lightweight editorial acknowledgment (optional, **not enabled**)

If DEC-08 accepts a narrow expedited path, an authenticated person separately authorized for editorial acknowledgment may compare the exact diff, confirm unchanged evidence-version set and absence of material or unresolved review changes, then record a short, auditable acknowledgment **bound to the new immutable revision**. It should be one short action (or a reviewed batch), not a full repeated workflow. The signed event records reviewer, exact predecessor and successor IDs/hashes, classification-policy version, reason, exact evidence IDs, decision time and any required segregation-of-duties check.

Only then may the UI show "Editorial correction acknowledged / current revision accepted under policy"; do **not** falsely present the previous historical reviewer's signature as their approval of text they never reviewed. Define whether this limited acknowledgment qualifies as "approved" in professional methodology before implementing it.

## Approval display and revision history

Separate:
- **Editing status:** draft/saved/unapproved/current revision.
- **Review status:** pending review, reviewer-acknowledged editorial change, formally approved, re-review required.
- **Historical sign-offs:** immutable approvals against exact prior revisions and evidence sets.

The current revision is formally approved only when a valid, **active and authenticated** sign-off for that *exact current revision* satisfies the signed DEC-08 requirements. An earlier sign-off may be historically valid without conferring current-revision approval.

Suggested UI examples:
- `Revision 7 · Saved · Approval pending` with `Revision 6 · Historical review`.
- `Revision 7 · Editorial correction acknowledged by [verified reviewer] under policy vN` (only after valid reviewed event).
- `Revision 7 · Material update · Re-review required`.
- `Revision 8 · Signed by [verified reviewer]` only when actual validated sign-off exists.

An unsigned successor must not be displayed as `REVIEWED`, `FINAL`, `FINALISED`, or `ACTIVE APPROVED` merely because `workpapers.workflow_state` still contains an older label. Compute the authoritative current approval from exact-revision sign-off/review state, or transactionally reset the visible workflow status on new revision creation, under a human-approved policy.

For material review notes opened/reopened **after** sign-off, preserve sign-off history but do not leave a misleading active approval indicator if DEC-08 requires suspension/re-review. Decide the revalidation behavior explicitly, and distinguish a note on a historical revision from one affecting current reliance.

## Existing implementation compatibility (non-authorizing)

The current application has an established legacy workpaper/revision workflow and a **separate**, deliberately fail-closed Normal Data authorization boundary. This draft does not certify the current sign-off, identity, review-note, or lifecycle implementation for authenticated multi-user professional approval.

Implementation reviewers must reconcile this proposal with current revision creation, stored workpaper-level workflow labels, revision-bound sign-off history, and the exact scope of Normal Data grants. Existing behavior must not be treated as proof that the proposed trusted editor or reviewer capability exists.

Security-sensitive findings about current actor attribution, review state, and custody should be handled through the repository's **private reporting procedure** under `SECURITY.md`, not detailed in a public design PR. Preserve accepted ADR-0007's requirement that only exact immutable revisions and evidence-version sets are signed, with later changes requiring appropriate current-revision review validity.

## Acceptance-test plan (proposed, not executed)

1. A verified person with workpaper-scope `ReviseWorkpaper` may save a successor to a signed revision without unlock or reviewer preapproval.
2. The prior immutable revision/sign-off and exact evidence IDs remain available; no old sign-off is silently moved onto the successor.
3. A denied/revoked subject, guessed UUID, stale session, or another workpaper's editor cannot append a revision; denial leaves SQLite unchanged.
4. A reviewer-only grant does not confer revise permission; revise does not confer approval or evidence-attachment permission.
5. Concurrent writers who both start from the same revision cannot silently overwrite; the losing write receives explicit conflict and comparison.
6. Saving a typographical change appends a separate revision without a full review form; it remains unapproved until an *authorized* editorial acknowledgment or new sign-off under DEC-08.
7. Self-declared "minor" on a changed conclusion, amount, procedure or evidence version cannot bypass full review.
8. Optional editorial acknowledgment records authenticated reviewer, exact successor content and evidence-set hash/IDs, policy version and an append-only audit event.
9. Unauthenticated/revoked reviewer cannot acknowledge or sign; preparer/self-review prohibition follows DEC-08.
10. A material successor to a previously `FINALISED` workpaper never shows that old final approval as if it applies to the new revision.
11. Unresolved current-revision review notes visibly block/suspend current approval according to signed DEC-08 policy, without mutating historical sign-off.
12. Audit append failure rolls back protected revision/acknowledgment mutations; no partial current approval.
13. Verification/custody failure of linked evidence fails closed at formal sign-off even when a stored earlier status indicated success, according to DEC-05/08 verification policy.
14. No activation of Normal Data D2/D3/D4 through these editorial features or through seeded permissions.
15. With unchanged, eligible evidence, a trusted workpaper editor with a separately checked read grant may save a typo successor without manual reattachment; new revision-owned link IDs preserve the **identical exact version-ID set** and a predecessor mapping, with no inherited sign-off.
16. Editing-only privileges cannot read otherwise inaccessible workpapers. An administrator's explicit Editor access profile issues separately auditable read and revise grants; revoking either blocks the corresponding operation.
17. A caller changes, removes, invents or replaces even one evidence-version ID while requesting carry-forward: the operation cannot be authorized as unchanged evidence and cannot inherit approval.
18. If the predecessor's required evidence has been quarantined or becomes ineligible, the backend must not carry it into an approved/review-eligible successor on the strength of its old verification flag; simulate denial and audit rollback.
19. A stale-base or concurrent editor's save cannot use a newly changed predecessor evidence set without an explicit retry and new review of the resulting diff.

All tests must use synthetic data and exact intended platform scope; separate a passing test fixture from an authenticated two-human integration test.

## Human decisions and out-of-scope items

**DEC-08 product/methodology authority** must choose permitted editorial correction classes, approval types, whether independent acknowledgment is sufficient, when a previously finalized engagement may be reopened, the exact effect of post-sign-off review notes, and which fields are materially review-sensitive.

**DEC-03/04/06 security/authorization authorities** must choose a trusted production session issuer, initial grant administrator, identity revocation model, segregation of duties, and genuine independent reviewer/merge governance.

This document makes **no** human decision, appoints **no** reviewer, supplies **no** platform proof, and confers **no** permission. It does not change the registered manual capture/sign-off commands or remove the existing Normal Data default-denial/Windows refusal. No implementation, feature flag, seeded grant, merge, branch-protection relaxation, or production entitlement is authorized.

## Implementation sequencing after decisions

1. Accept a product/methodology DEC-08 decision and an independently reviewed ADR amendment defining editorial rules and current-revision approval derivation.
2. Independently establish trusted production actor/revise/admin/reviewer issuers and scoped grants under DEC-03/04/06.
3. Add synthetic negative tests for old approval not transferring, cross-user revision denial, conflicts, and stale status.
4. Implement the workpaper-scoped revision capability and current-revision approval projection behind a closed default-deny gate, review and test exact HEAD.
5. Add optional editorial acknowledgment only after policy and separation-of-duties acceptance.
6. Keep formal Normal Data D2/D3/D4 blocked until their **separate** DEC-01..DEC-08 evidence and approvals are complete.
