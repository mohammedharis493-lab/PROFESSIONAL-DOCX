# Draft — native workpaper authoring authorization boundary

**Status:** proposed and NON-AUTHORIZING. Prepared 2026-10-17. Applies to draft PR #137; this is neither an accepted ADR nor independent security approval. Governance blocks: [Issue #129](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/issues/129) (DEC-02/03/04/06/08 and custody DEC-01/05/07); [Issue #138](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/issues/138) (callable legacy revision and signer risks). See the neighboring WORKPAPER_REVISION_BACKEND_HANDOFF_DRAFT.md for transaction requirements.

## Delivered in this incremental draft

The private native Rust module workpaper_revision_authorization_contract.rs is compiled into the crate but is **not registered as a Tauri command and is not called by production Save, read, evidence-link or sign-off code**. No source-version or immutable history data is modified. The sole shipping policy adapter is **DenyAllWorkpaperAuthoringPolicy**, which cannot grant anything. A verified native session contains private fields and has **no production constructor**. Its only constructor exists inside cfg(test) fixture code. The module has no native credential issuer, session persistence, workpaper-grant table, grant administrator, evidence carry-forward, UI route or sign-off. All positive test cases use isolated test-only fixture grants.

This is a compile-checked interface candidate, **not a genuine verified session, authentication gateway, issuer, authorization grant, or defense against privileged local malware/database owners**. Preflight success using a hypothetical policy does not authorize a write.

## Proposed distinct authorities — human approval required

| Operation | Proposed native scope | Cannot be inferred from |
| --- | --- | --- |
| Read workpaper | READ_WORKPAPER grant scoped to the exact workpaper plus a trusted live human session | Caller-provided ID, OS name or role |
| Append correction successor | READ_WORKPAPER plus REVISE_WORKPAPER, exact same workpaper, checked again within final IMMEDIATE write transaction | Normal Data MODIFY_WORKPAPER_REVISION (an exact immutable revision), reviewer unlock or self-described minor correction |
| Retain unchanged exact evidence | DEC-05/08-approved source-version custody check and native backend-only fresh successor links | Text-only equality, user-supplied evidence list, authoring rights alone |
| Sign new revision | Independent native signer authority and new exact-revision/evidence-set binding | Old sign-off, PREPARED label, authoring grant or caller-asserted actor ID |
| Issue or revoke grants | Independently bootstrapped trusted native admin with audit | First user, local SQLite row, draft doc, tests or temporary fixture |

These proposed READ_WORKPAPER / REVISE_WORKPAPER scope names are **not** existing database-grant values. Migration 0028 applies MODIFY_WORKPAPER_REVISION only to one exact revision, in a separate Normal Data workflow. Do not broaden or rename it to grant future workpaper writes.

## Final native transaction contract still missing

1. DEC-03 supplies an independently authenticated native person/session issuer with expiry, impersonation and revocation semantics. No arbitrary role or principal identifier from the frontend.
2. DEC-04 supplies a separate workpaper-scoped grant registry, administrator bootstrap, revocation, recovery and immutable admin audit, with no seeded grants. Workpaper-to-engagement membership and eligibility must be checked from the database.
3. Inside **the same SQLite IMMEDIATE write transaction**, recheck session activity and both exact-workpaper grants, base revision CAS, immutable seven-field hashes, complete chain, previous signed exact evidence/version snapshot, custody validity, and DEC-08's full-state no-op rules. A standalone preflight is never a Save permit.
4. Produce a new unapproved successor with untouched old rows, server-resolved exact source versions attached under new link IDs only if separately approved, historical sign-off supersession, authenticated audit and rollback. No reviewer unlock for legitimate authoring; no implied reviewer approval.
5. DEC-06 requires an actual named independent security reviewer, reviewed migration/issuer/adapters, failure and platform proof, and explicit security/release decisions. CI or a draft branch cannot satisfy this gate.

## Regression scope and hard stops

The candidate module's five tests prove only fail-closed **contract** behavior: absent session, default deny, read-vs-revise/workpaper mismatch, expired/revoked fixture session, and malformed or role-shaped target IDs. They do not verify real SQLite grants or signers, hostile-local adversaries, formal evidence custody, or in-transaction revocation. Existing native transaction tests in PR #137 test SQLite mechanics separately; the two layers are deliberately **not wired together**.

**Hard stops:** do not merge PR #137, connect prototype Save, register this module in generate_handler!, create a production verified-session constructor, seed the first admin, reuse a caller-provided actor/role, or treat success of any fixture policy as authorization. Human DEC-03/04/06/08 decisions and independent native security review are prerequisites for production integration.
