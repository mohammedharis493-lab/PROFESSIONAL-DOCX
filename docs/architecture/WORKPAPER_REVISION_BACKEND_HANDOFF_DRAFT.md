# Draft: Workpaper revision Save — backend implementation handoff

- **Status:** PROPOSED / NON-AUTHORIZING ENGINEERING HANDOFF — not an accepted ADR, security approval or permission to merge.
- **Prepared:** 2026-10-16.
- **Target:** [Draft PR #137](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/pull/137).
- **Decision gate:** [Issue #129](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/issues/129), especially DEC-03/04/06/08.
- **Related:** [Policy proposal PR #136](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/pull/136); [accepted ADR-0007](ADR-0007-evidence-capture-and-signoff-binding.md) and [accepted ADR-0008](ADR-0008-sqlite-persistence-durability-and-recovery.md).

This is a **design and review checklist**, not a runnable Tauri/SQLite implementation. It does not activate production identity, grants, custody, capture, promotion, Save or sign-off, and does not close any human-decision gate.

## Product intent — easy revision, independent approval

A person with **genuinely authenticated and narrowly workpaper-scoped revision rights** should eventually use **Edit → change one character → Save** even after the old revision was reviewed, **without a former reviewer unlocking editing**. Saving must append an immutable successor, not rewrite a signed historical row. The old sign-off and exact version-resolved evidence snapshot remain attributable to the old revision. Save does **not** approve a successor. If a reviewer is unavailable, a permitted user may work on an **unapproved** successor once the trusted authoring identity/permission system has itself been approved and built.

The still-unmounted PR #137 editor handles all seven canonical fields, stale-base warnings, field-only restore, text-only three-way comparison and detached immutable drafts. None of these UI helpers grants permission or implements a production Save.

## Verified source boundary (why a new route is needed)

| Observed source | Existing behavior | Future requirement |
| --- | --- | --- |
| **src/App.tsx / submitWorkpaperRevision** | Calls legacy revision creation with three UI fields but explicitly supplies empty strings for four other canonical fields | Do NOT reuse for a signed typo correction; preserve all seven fields |
| **src-tauri/src/lib.rs / create_workpaper_revision** | Existing **callable** legacy Tauri command forwards caller content without a trusted workpaper-scoped author grant in this handler | Separate reviewed trusted-native authorization boundary |
| **src-tauri/src/persistence.rs / create_workpaper_revision** | SQLite IMMEDIATE transaction, SHA-256 over seven length-prefixed UTF-8 fields, append revision, supersede sign-offs, audit | No expected latest-base ID / exact predecessor evidence copy in this existing function |
| **src-tauri/migrations/0006_engagement_workpapers.sql** | Immutable revisions and immutable revision-owned evidence links, optional content hash | Create fresh successor link IDs while preserving exact version IDs under specifically approved carry-forward |
| **src-tauri/migrations/0009_workpaper_signoffs.sql** | Immutable sign-off/evidence snapshot, append-only supersession | No sign-off transfer to successor; verify direct revision ancestry and exact snapshot |
| **PR #137 tests/workpaper-revision-successor-contract.test.mjs** | Synthetic oracle using real migration DDL and **mock** grants; Node experimental SQLite | Not a production authority, Rust transaction, custody proof, or independent approval |

**Precise status:** The legacy production revision command already exists and is callable. It must be separately assessed for exposure and any migration/disablement plan. **What is absent is the new lossless, authenticated, workpaper-scoped corrective Save route**. Do not connect the PR #137 preview editor to the unsafe legacy command or claim all production revision writing is disabled.

## Blocking policy questions for accountable humans

| Decision | Must specify | Until decided |
| --- | --- | --- |
| DEC-03 | Native trusted per-person session issuer, expiry, impersonation, switching and revocation | Deny the NEW trusted corrective route |
| DEC-04 | Initial trusted administrator bootstrap, grant issuance/revocation and recovery | No seeded/default grants; reject caller-claimed actor/role |
| DEC-06 | Named independent security reviewer, platform proof, acceptance artifacts and release authority | No security approval, no merge or feature activation |
| DEC-08 | Workpaper eligibility, revise versus approve, segregation of duties, unchanged-evidence carry-forward, no-op and editorial acknowledgment | No inherited sign-off, no shortcut review, no unapproved source-copy policy |
| DEC-01/02/05/07 | Supported custody platforms, adversary assumptions, durable storage and retention | No claim of approved protected storage or formal controlled evidence |

These are independent requirements. The existing exact-revision Normal Data permission **MODIFY_WORKPAPER_REVISION** must never be broadened into a persistent **REVISE_WORKPAPER** grant by renaming it. An authoring privilege alone does not grant discretionary evidence attachment, review/sign-off, retention or export rights. Any workpaper-scope grant requires its own explicitly reviewed migration and admin workflow.

### True no-op vs unchanged text: unresolved DEC-08 semantics

The synthetic oracle tests intentional unchanged-text successor creation. Draft PR #136 instead proposes no successor for a **true no-op** (identical canonical content AND identical verified evidence/review-relevant state). Both can be valid **test scenarios** but cannot be conflated into an approved policy.

**Proposal to decide, not implement automatically:**

1. Unchanged text **plus** unchanged exact evidence and review-relevant state: return a non-mutating no-op result, preserving prior sign-off.
2. One-character change within signed canonical content: append a new immutable unapproved revision, pending applicable independent review.
3. Evidence-only or review-relevant change: handle under separately approved permissions; never skip Save solely because text is unchanged.
4. Explicit user checkpoint with identical state: DEC-08 must decide whether to record a non-review event, append an authorized revision, or return no-op.

A client **content-only** preview must never make this full-state decision. If an expedited editorial acknowledgment is approved, it must be a separate authenticated action on the **exact new revision** and its version set; the author's self-applied minor label is not approval.

## Proposed new native command contract (NOT IMPLEMENTED)

**Untrusted UI input:** workpaper ID, expected immutable latest revision ID, all seven canonical text values verbatim, revision reason and (if approved) a narrow intent that no evidence change is requested.

**Must not be trusted from UI:** actor ID, OS username, role, grant, claimed approval, exact evidence IDs to copy, custody labels, evidence-set digest, latestness flag, successor IDs or self-declared review category.

**Native state:** authenticated session and permitted workpaper read/revise grants, resolved workpaper-to-engagement scope, relevant evidence custody verification and audit attribution. Trusted session identity must be established outside this UI request.

**Result only after committed transaction:** newly created revision ID and number, exact current-revision review state, copied link count and an explicit status such as created, approved-policy no-op, stale-base, stale-evidence, denied, invalid-evidence or integrity-error. These labels are illustrative, not an implemented API.

## Proposed one-transaction algorithm (subject to decision and review)

All mutable authorization and freshness facts must be verified within the **same final bounded SQLite IMMEDIATE write transaction**, or safely rechecked there after preliminary native validation:

1. Independently establish a live authenticated person from the trusted native boundary; never accept a caller-selected user or grant. Validate exact seven-field schema with no trimming/Unicode normalization. Preserve production Rust UTF-8 **byte-length**-prefixed hash framing; test actual Rust SQLite behavior for NUL, as Node's experimental TEXT reader truncates that character.
2. Start a properly configured SQLite IMMEDIATE transaction under ADR-0008. Recheck scoped workpaper READ and REVISE grants, revocation/expiry, active resource ownership, engagement and eligibility. A previous review/signature is not a blanket authoring lock.
3. Read latest workpaper revision from the database; compare to **expected immutable base revision ID**. On mismatch, fail without writes and preserve the unsaved editor buffer; do not auto-rebase.
4. Validate contiguous same-workpaper ancestry, current and historical exact-revision sign-offs/snapshot link sets, and the complete server-resolved predecessor evidence set. Evidence-only races must not be hidden by the frontend text diff.
5. If separately approved, mechanically carry unchanged exact source **version identities and relationship metadata** into freshly generated successor link IDs. Source links are selected by the trusted backend; never permit a user-supplied arbitrary evidence list to piggyback on REVISE permission. Reject quarantined/invalid/unknown custody according to the signed policy.
6. Implement the independently selected **full-state no-op** rule **before any mutation**. Identical text alone is not enough. Do not supersede historical review for a true non-mutating no-op.
7. Append an immutable successor with the correct direct predecessor, revision number and seven-field hash. Preserve all old revision/evidence/sign-off history. For an approved copy path, create new immutable evidence-link rows and auditable provenance; never repoint old evidence links.
8. Add append-only sign-off supersession only for the applicable prior exact revision, never manufacture the predecessor's approval on the successor. Establish the successor's explicit unapproved/pending review status; do not inherit a stale FINALISED/REVIEWED workpaper label. Preserve review-note history and apply the human-approved current reliance rules.
9. Append authenticated audit and any required outbox intent **in the same transaction**, then COMMIT. No success notification until commit. Any failure rolls back successor, copied links, supersessions and audit. Keep the local draft available for a user-friendly retry.

This does not authorize cross-filesystem pseudo-atomicity, uncontrolled copy/delete of formal evidence or a claim that source versions are safely retained. Custody and filesystem crash assumptions remain subject to DEC-01/02/05/07.

## Required real-backend negative tests (NOT satisfied by Node fixture)

- **Identity/scope:** missing/expired/revoked trusted native session, unbootstrapped admin, forged actor/role, wrong engagement/workpaper, read-grant absence and caller-chosen permission all denied without mutation.
- **Everyday UX:** signed original loads all seven text fields exactly; one character edit preserves other six, whitespace, Unicode and deliberate blanks; missing/future canonical fields fail closed; no reviewer unlock for ordinary authorized editing.
- **Concurrency:** two independent database writers cannot succeed from one expected base; stale-base and stale-evidence conflict preserve local draft; bounded locks and crash/rollback.
- **Evidence:** zero/many links; exact content/controlled version IDs, matching document, relationship and description; **fresh IDs** on successor; no optional extra evidence privileges. Invalid/quarantined version denied when policy requires.
- **History:** only direct successor supersedes exact sign-off, subsequent revisions retain old supersession and remain unapproved, malformed chain or snapshot denied.
- **No-op and review:** apply explicitly selected DEC-08 no-op/acknowledgment policy; a signed older revision does not make a newer revision approved; no self-declared cosmetic approval.
- **Integrity/durability:** actual Rust seven-field SHA-256 parity and embedded-NUL round trip, SQLite audit rollback, recovery, legitimate backup, and any required filesystem durability proof.
- **Independent acceptance:** named human reviewer examines native code and adversarial cases, records threat assumptions and decision evidence, with all required exact-head CI/security jobs green. Tests and CodeQL alone do not approve security.

## Delivery sequence / explicit handoff

1. **Now:** retain PR #137 as draft and its preview unmounted; this document is review material. Track the existing callable legacy revision command separately; **do not wire corrective editing to it**.
2. **Human decisions first:** close applicable DEC-03/04/06/08 with actual owners, dates, alternatives, chosen policy and remaining risks; retain separate protected-custody gates for any formal versioned evidence.
3. **Separately reviewed backend PR:** implement trusted native per-person identity and workpaper grants with no default issuances; create inaccessible/disabled native transaction with real Rust/SQLite/Tauri tests. Only after review wire the full-field UI.
4. **Release:** derive approval from current exact-revision sign-off, prove independent authorization and retention semantics, obtain formal review and explicit merge/release action. Do not promote this document or a passing mock fixture into approved architecture.

**Not delivered:** production corrective Save, authenticated person or grant issuer, admin bootstrap, authorized copy/acknowledgment, reviewer approval, protected-root readiness, formal capture/promotion, or permission to merge.
