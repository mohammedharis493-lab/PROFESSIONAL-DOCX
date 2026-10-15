# Draft: Per-person identity and shared-workspace collaboration boundary

- **Status:** PROPOSED / REVIEW INPUT ONLY — NOT APPROVED, NOT IMPLEMENTED
- **Date:** 2026-10-14
- **Base:** protected develop at 170e2e498d472a3780f308772a5910f6e56686f3
- **User preference:** two or more people collaborate on shared data using **separate individual user logins**, not a shared password and not a device as a person
- **Scope:** DEC-02, DEC-03, DEC-04 and DEC-08 design input for both proposed Windows and Unix/Linux platforms
- **Decision record:** [D2-P1 human security gates, Issue #129](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/issues/129)
- **Review inputs:** [draft threat model, PR #128](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/pull/128), [user preference recorded on Issue #129](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/issues/129#issuecomment-6172418719), [D2 exit criteria](NORMAL_DATA_D2_EXIT_CRITERIA_AND_ROADMAP.md), [proposed promotion boundary](ADR_NORMAL_DATA_PROMOTION_BOUNDARY.md), [accepted sign-off ADR-0007](ADR-0007-evidence-capture-and-signoff-binding.md)

> **Non-authorizing boundary:** This draft supplies requirements, explicit design alternatives and negative-test proposals only. It is not a security decision or governing ADR amendment. It installs no identity issuer, enrollment/bootstrap authority, account, role, grant, evidence custody, controlled capture, specialist linkage, sign-off or UI. Do not merge documentation as a substitute for independent approval.

## 1. Problem and proposed product outcome

Two or more colleagues should be able to work in the same *logical collaboration workspace* without sharing credentials. Each human should be independently identified at the time of a material action so the system can enforce individual permissions and explain who prepared, reviewed, approved or changed the exact revision.

**Terminology matters.** A logical collaboration workspace is a **proposed product tenancy/access concept**, not automatically equivalent to the existing `normal_data_workspaces` table, an engagement, a client or a filesystem directory. No cross-resource mapping or membership schema has been approved. A device/session may provide context but is **not** a human identity, a grant administrator or an independent reviewer.

The user's agreement is a product-direction proposal pending named, authenticated product-owner confirmation; it does not decide whether same-OS-user malware, a local administrator or a joint SQLite/storage owner is trusted or adversarial (DEC-02).

### Target user-facing outcomes (requirements to validate, not shipped behavior)

1. User A and user B can have separate logins and be given individually revocable access to the same authorized collaboration workspace.
2. Each person's action is attributed to that person and, where appropriate, to a bounded session/device context. One shared password is not needed.
3. Merely joining a workspace never authorizes formal controlled-evidence capture, grant issuance, specialist attachment, review or sign-off.
4. Access can be individually withdrawn without rewriting other users' identities, historical workpaper revisions, evidence versions or signed review events.
5. The product must declare how concurrent changes and multi-device/offline usage are handled; an authenticated action cannot silently overwrite newer review-sensitive state.
6. Windows and Unix/Linux controlled custody are separate proposed platform requirements, each with independent security proof and approval (DEC-01); neither is certified here.

## 2. Identity, collaboration and security objects — conceptual only

| Concept | Proposed meaning | Must NOT be treated as |
| --- | --- | --- |
| Human user | Independently authenticated named person with stable subject identity | A frontend-provided UUID, string, role label or display name |
| Authentication session | Short-lived, revocable binding of verified user, authentication event and current context | Permanent proof, reusable capture token or a grant |
| Device | An optional registered client/device context tied to a session | The human user, reviewer, signed approver or authority |
| Collaboration workspace | Logical group of shared product data and members; precise mapping to existing resources **unresolved** | An automatic Normal Data, engagement or evidence grant |
| Workspace membership | Authorization to perform explicitly specified collaboration actions, subject to policy | All resource permissions or a specialist sign-off |
| Normal Data workspace | Existing exact `normal_data_workspace_id` resource used by a frozen comparison run | New tenant/workspace membership automatically inferred from naming |
| Engagement / workpaper revision | Existing exact professional target resources, with client/engagement/revision membership and lifecycle constraints | Mutable `latest` paths, editable signed revision or implicit upload target |
| Scoped permission grant | Existing local registry's exact subject + permission + resource grant semantics, dependent on a trusted issuer | Independently authenticated identity or custody approval |
| Audit attribution | Historical event bound to verified principal and exact affected resource/revision/version | Proof against an administrator able to rewrite both event store and evidence |

**Existing implementation status:** `normal_data_promotion_policy::VerifiedPrincipal` and `normal_data_permission_admin::VerifiedGrantAdministrator` can be constructed in test builds only. There is no production identity issuer, login/bootstrap authority, Tauri identity/grant API or production formal promotion. `DenyAllPromotionPermissions` defaults to denial; `SqlitePromotionPermissions` does not itself authenticate a user. A UUID is correlation data, not identity. Existing permission registry uses three narrow scopes:

**Existing manual indexed-file capture versus new Normal Data D2 (do not conflate).** The shipped Rust/Tauri `capture_controlled_evidence` command in `src-tauri/src/lib.rs` invokes `evidence::capture_controlled_evidence` in `src-tauri/src/evidence.rs` for an indexed file instance. This **existing, separately scoped manual evidence feature** is not this proposed Normal Data three-artifact promotion workflow and has not been independently assessed or authorized by this draft. The internal `normal_data_capture_preflight`, `normal_data_promotion_policy`, and `normal_data_permission_admin` modules have no new production command exposure or constructible production verified human/admin issuer. Do not infer that the manual command supplies the future separate per-human login, three exact-resource grants, result-artifact retention, three-version atomic capture, workpaper-revision linkage, sign-off binding, independent platform custody proof, or acceptance of DEC-01–DEC-08. **“Normal Data D2 formal capture/promotion remains blocked” does not mean all existing manual evidence-capture commands are disabled.** Any proposed reuse of manual retention in D2 must be reviewed against D2-01…D2-08 and accepted ADR-0007 without silently inheriting permissions or custody approval.

- `READ_NORMAL_DATA_WORKSPACE` on one exact Normal Data workspace ID.
- `ATTACH_EVIDENCE_TO_ENGAGEMENT` on one exact engagement ID.
- `MODIFY_WORKPAPER_REVISION` on one exact workpaper-revision ID.

The future product collaboration membership design must **not silently enroll subjects, issue any of these grants, or bypass their current exact-resource checks**. The current internal transaction gate also rechecks historical run provenance and eligible target revision under a SQLite `IMMEDIATE` transaction; this is not a substitute for native authenticated identity or protected filesystem custody.

## 3. Alternative architectures requiring independent selection

**A — Separate named human logins, shared collaboration workspace (user-endorsed product preference).** An independently verified user subject can join one or more workspaces under a separately enforced membership policy. Strong audit and individual revocation are possible only if principal issuance, sessions and event logging are trustworthy. This remains a **proposal**, not an implemented sign-in system.

**B — Shared application login/credential.** Simpler to explain to casual users but cannot reliably attribute actions to separate humans and complicates revocation and professional sign-off. **Not the endorsed default**; do not implement via a back door or assume shared passwords are authorized.

**C — Account-per-device.** A device identity may help bound a session or manage endpoints; it does not reliably identify which person created a review or sign-off. **Not the endorsed default**. Support for multiple people or devices using shared data still requires an identity/collaboration decision.

The security team must independently decide whether an OS-local authority, an external identity provider, managed organization login or another independently authenticated principal issuer is acceptable on the selected platforms. This draft chooses **none** of those technical issuers, protocols, trust anchors, recovery procedures or key stores.

## 4. Proposed authorization and privilege-separation requirements

**Default deny across all operations.** A future production identity adapter may deliver a verified, fresh, non-revoked subject binding only through a trusted native/session boundary. Untrusted Tauri/IPC data, caller-supplied role strings, display names, an `actor_id`, a shared password or a device UUID must never create `VerifiedPrincipal` or `VerifiedGrantAdministrator`.

| Proposed action class | Evidence required before a future implementation could allow it | Status |
| --- | --- | --- |
| Open collaboration workspace/data | Verified user + current authenticated session + precisely scoped active membership/permission; prevent cross-workspace access | Design review only |
| Invite or remove a collaborator | Verified delegated workspace administration authority; independent bootstrap/revocation rules; attributable audit and no implicit specialist grants | DEC-03/04 open |
| Read a frozen Normal Data run | Existing exact run/workspace provenance and separately verified `READ_NORMAL_DATA_WORKSPACE` grant for protected operations | No production issuer |
| Request future formal capture/linkage | Deliberate exact run + recipe/dataset/artifact identities; per-subject exact active grants for read workspace, attach engagement and modify workpaper revision; final in-transaction rechecks; completed custody independent of working-data stage | D2/D3 blocked |
| Edit draft professional work | Verified subject; exact engagement/workpaper/revision membership and eligibility; optimistic/concurrency control per accepted future policy | DEC-03/04/08 open |
| Review or sign a workpaper | Independently authorized named human, exact revision and immutable evidence-version set, all required review-state and separation-of-duties checks | DEC-08 and ADR-0007 govern; not enabled here |
| Manage trusted principals or grant administrators | Separate, audited, independently authenticated administration authority; bootstrap and recovery must not be available to any workspace member by default | DEC-03/04 open |

**Separation-of-duties decision still needed:** product/security reviewers must define whether preparer, reviewer, approver, grant administrator and workspace administrator can be the same person; which exact actions require distinct humans; and emergency override approval + audit rules. Do not equate a role name with independent permission. No role matrix, hierarchy, permission escalation or initial administrator is implicitly approved in this document.

**Revocation:** A future denial model must account for disabled person, deleted membership, expired/revoked session, revoked exact resource grant, changed target eligibility and concurrent administrative action. Prior read-time preflight is **never** a permission token: fresh checks must be bound to the final state-changing transaction, with external file handling separately secured. A revoked person cannot gain access merely through a second device or stale cached membership.

## 5. Cross-device, concurrency and offline boundaries (not designed yet)

- **Multiple devices:** one person may own multiple sessions/devices; log out or revoke each by a reviewed rule. Device identity supplements the user subject; OS account sharing on one device does not provide trusted per-person attribution.
- **Concurrent editing:** use explicit revision numbers/expected-version conditions or a reviewed conflict protocol. A stale client must not silently overwrite a newer revision, review note, grant or sign-off.
- **Offline and synchronization:** whether offline workspace sharing is supported, what server/coordinator is trusted, how SQLite-backed local state synchronizes, what keys/transport are used and how stale/offline revocation propagates are **entirely open**. Do not claim the current local SQLite implementation supplies network multi-user consistency.
- **Signing and review:** evidence signatures and historical sign-off must remain bound to an exact immutable workpaper revision and precise evidence-version set (accepted ADR-0007), not a shared mutable workspace state or a device alias. A new material change creates a new revision and re-review.
- **Recovery:** lost device or account recovery must require independently verified authority and not regenerate a privileged user silently. No invitation/recovery link may automatically confer controlled capture or administrator status.

## 6. Audit model requirements (no new ledger authorized)

Proposed minimum audit event families: user/session authentication and revocation, workspace membership invite/accept/remove, delegated privilege issuance/revocation, exact-resource permission changes, draft/revision creation, material edit/conflict rejection, review-note transitions, formal capture attempts and outcomes (once authorized), evidence-version linkage, review/sign-off/supersession, recovery and denied privileged actions.

For every authorized state-changing event, identify **verified human subject**, event ID, exact target resource ID(s), effective permission/scope and policy version, UTC time with source/ordering model, outcome, and session/device context when trustworthy. For formal evidence or sign-off also bind immutable run/revision/content-version references and retained-byte/digest meaning where appropriate; never rely on a mutable `latest` path. Exact audit schema, retention, confidentiality, deletion, tamper resistance and cross-store durability are DEC-05/07 matters.

A SQLite audit trigger or a stored SHA-256 in the same database is **not independent proof against an attacker permitted to edit that database and the artifact storage**. DEC-02 must explicitly classify same-OS-user malicious code, another local user, administrators and joint database/storage writers. Collaboration between legitimate users **does not** imply any one of these adversary policies is accepted.

## 7. Proposed review/test matrix — future evidence, not current test coverage

| ID | Proposed negative or invariant demonstration | Expected security outcome to specify/review |
| --- | --- | --- |
| MU-01 | Unauthenticated or forged user UUID/display name/role creates a request | No principal, member access, grant, audit attribution or file access |
| MU-02 | Two distinct valid users share one authorized workspace | Individually attributable actions; only explicitly authorized resources visible |
| MU-03 | User of workspace A guesses B's workspace/engagement/revision UUID | Fail closed without cross-tenant metadata/bytes leakage |
| MU-04 | Membership alone requests all three Normal Data promotion grants | Deny; membership is not an exact-resource grant |
| MU-05 | Expired/revoked/disabled human or revoked device session attempts an operation | Deny on final authority check, including from another device/stale cache |
| MU-06 | Non-admin member requests invitation, subject enrollment, grant issue/revoke or admin recovery | Deny; no accidental bootstrap; no partial mutation/audit mismatch |
| MU-07 | User has workspace read but lacks exact engagement attach or revision modify grant | Deny formal capture/linkage with zero evidence links |
| MU-08 | Two users update the same review-sensitive revision using stale expected version | Deterministic conflict/denial, not last-write-wins over a review/signed state |
| MU-09 | Preparer attempts reviewer/approver action or same-user independent sign-off | Deny or allow strictly per separately approved separation-of-duties policy; no implied acceptance |
| MU-10 | Signed revision receives attachment or material edit | Preserve historical signed revision/version set; new revision and re-review |
| MU-11 | User/device and privilege are revoked between preflight and final mutation | Final re-evaluation denies; no stale grant transfer |
| MU-12 | Audit append fails during membership/grant/revision mutation | Atomic rollback or specified recoverable fail-closed state; never silently successful unaudited action |
| MU-13 | Shared OS login is mistaken for individual human authentication | Deny privileged actions requiring individual attribution |
| MU-14 | Same-OS-user process or privileged actor rewrites local DB plus artifact | Only claim resistance/detection if separately trusted proof survives that attacker; otherwise classify as accepted risk with signed approval |
| MU-15 | Cross-device offline copy reconnects after permission or sign-off change | Quarantine/reconcile or deny according to a reviewed protocol; never resurrect revoked rights or rewrite signed history |
| MU-16 | Windows reparse/ACL or Unix symlink/mount/root substitution during future custody | Fail closed until platform-specific handle-bound, effective-rights and durability evidence accepted |

Tests MU-01 through MU-16 are **proposed acceptance scenarios**, not assertions that corresponding software or tests exist. The document grants no authority to implement positive authentication, provision users, enable custody or create user-facing permissions pending DEC-01–DEC-08 and independently reviewed design. Negative-only tests against existing default-deny behavior may be considered as separate limited PRs.

## 8. Decision register / acceptance prerequisites — all unresolved

| Blocker | Required explicit human decision/evidence |
| --- | --- |
| **DEC-01** | Confirm dual Windows + Unix/Linux proposal with precise supported versions/filesystems, excluded mount/share classes, independent platform approval and rollout priority |
| **DEC-02** | Define resist vs detect/recover vs trusted boundary for same-OS-user malware, other users, admins, DB+storage owners; mitigation, accepted risk and per-platform proof |
| **DEC-03** | Select trusted per-person principal issuer, authentication/session binding, rotation/expiry, revocation, multi-device handling, lost account/device recovery |
| **DEC-04** | Select independently trusted grant/membership administrator bootstrap, scoped delegation and revocation/rollback, recovery and auditable issuance |
| **DEC-05** | Select durable protected storage and cross-SQLite/filesystem crash model separately on Windows and Unix/Linux |
| **DEC-06** | Name an independent human security/architecture reviewer and specify separately signed design/proof/custody acceptance artifacts |
| **DEC-07** | Decide privacy, legal hold, retention, audit/export, backup/restore, deletion and incident workflows for user and evidence records |
| **DEC-08** | Decide actual eligible specialist types and revisions, engagement lifecycle status policy, named preparation/review/sign-off authority and separation of duties |

Each decision needs an identified authenticated owner, independent approver where required, date, alternatives, explicit trust/adversary assumptions, negative test evidence and residual risk acceptance, followed by an independently reviewed governing ADR change. A product preference and this draft are not approval.

## 9. Explicit stop conditions and delivery gates

- Do **not** introduce production login/account creation, public Tauri command, real principal/admin construction, seeded/granted privileges, evidence storage grants, formal capture, specialist attachment, evidence promotion or sign-off UI from this PR.
- Do **not** repurpose a Normal Data workspace as a collaboration tenant or assume current SQLite permits safe device-to-device sharing without a separate design.
- Preserve the **unconditional Windows protected-root denial**: `inspect_retention_root_posture` must keep returning the Windows DACL/durable-storage-unverified error. Unix mode/owner observations do **not** certify custody. Keep D2/D3/D4 blocked.
- Do **not** change the accepted ADR-0007 historical exact-revision/evidence-version binding. Do **not** convert transient staged/candidate working data into formal evidence.
- Independently review this document's scope; run all six required **completed successful exact-head** CI/Security jobs for the proposed PR, resolve reviews and obtain **fresh explicit PR-specific human merge authorization** before any merge.
- A merged draft would still not decide DEC-01–DEC-08, approve any threat model or unlock formal capture. Maintain these gates until independently accepted decision evidence and governing ADR amendments exist.
