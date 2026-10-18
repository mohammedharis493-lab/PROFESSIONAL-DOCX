# Workpaper authority and legacy containment — decision packet (DRAFT)

**Prepared:** 2026-10-18  
**Status:** undecided review material; NOT an accepted ADR, selected identity architecture, implementation permission, professional sign-off, or D2 exit.  
**Source:** [Issue #129](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/issues/129) (eight unapproved decisions); [Issue #138](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/issues/138) (callable legacy paths); draft [PR #136](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/pull/136) (revision policy); draft [PR #137](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/pull/137) (lossless editor and isolated contracts); accepted ADR-0007/0008 (evidence/sign-off binding and SQLite durability).

## Product invariant before selecting a security model

A genuinely authenticated editor with exact-workpaper revision rights should eventually be able to open an already-reviewed workpaper, change **one character**, and **Save an immutable successor immediately** without a prior reviewer unlocking it. All seven canonical fields must survive byte-for-byte except deliberately changed fields; exact evidence versions may be carried forward **only with separately approved custody and scope rules**. The predecessor's signed revision, evidence snapshots and sign-offs remain historical. **The successor is unapproved** until a different authorized review/sign-off operation, subject to the selected segregation-of-duties policy. Editing and approval are independent authorities. No auto-signoff or self-claimed "minor" exemption.

**Current truth:** The real Tauri app already registers the legacy revision, evidence-link and sign-off commands. The existing three-field revision form passes four additional canonical fields as empty strings; the native legacy revision command lacks expected-base compare-and-swap and exact-link carry-forward; the legacy sign-off form supplies caller-selected actor ID/role. PR #137's seven-field editor and authority prototype remain **unmounted/test-only**. Neither an immutable database row nor a green CI run authenticates a person.

## A. Human decision record — proposals, not approvals

For **every** DEC, an accountable human must complete: owner, approver independent where required, decision date, explicitly selected alternative, supported platform, attacker/trust assumptions, measurable acceptance evidence, residual risk and rollback. **Do not pre-check Issue #129 or use this document as authorization.**

| Decision | Explicit alternatives for the owner | Working candidate for evaluation, NOT selected | Release hold until recorded |
| --- | --- | --- | --- |
| DEC-01 Capture platform | Windows NTFS only; Linux/POSIX only; separately proven both | Stage by independently proven platform, not a blanket both-OS promise | No formal captured-evidence promotion on unproved platform |
| DEC-02 Adversary model | Local same-OS-user and admin trusted; same-user malware adversarial; database/storage owner adversarial | Model webview/renderer as untrusted; explicitly separate OS-user and local-admin threats; do not claim defense against a malicious DB owner without a tested protected trust boundary | No claim of verified person or tamper-proof local custody |
| DEC-03 Person/session issuer | OS-bound account identity; organization OIDC/SSO native broker; organization-issued offline credentials; unverified draft-only | For professional approvals, investigate an independent per-person identity provider + trusted native broker and revocation. If unavailable, choose clearly unverified **draft-only**, not a simulated user | No trusted signer, "reviewed" claim or production trusted Save |
| DEC-04 Bootstrap/grants | Organization provisioned admin; separately verified signed invite; managed enrollment; unattended first-user bootstrap | No first-launch auto-admin and no direct caller/DB-row asserted grants. Separately issued `READ_WORKPAPER` and `REVISE_WORKPAPER` scoped to exact workpaper; reviewer/approve separate; expiry/revocation audited | No prod grant issuance, migration activation or admin bypass |
| DEC-05 Custody/crash | Encrypted app-managed root + native ACL proof; verified OS-specific protected store; fail-closed no formal capture | Fail closed until actual platform custody, DACL/ownership, SQLite/filesystem crash and recovery tests pass. Existing Windows DACL posture still unverified | No formal controlled-source carry-forward or capture |
| DEC-06 Independent review | Internal independent security architect; qualified external reviewer; postpone professional signing | Postpone sign-off/security approval while no independent reviewer exists. Automated CI, mock personas and assistant inspection may develop evidence, not replace a human | No D2 exit, merger/release or professional authority |
| DEC-07 Retention/recovery | Explicit legal-hold retention policy + verified backups; no formal capture | Keep formal-capture/cleanup disabled until a compliance owner specifies holds, backup/restore, deletions and incident response | No unsupported retention/durability promise |
| DEC-08 Editing, review, no-op | Mandatory independent review; narrowly authenticated expedited editorial acknowledgement; no editorial fast path | Prefer author **Save without reviewer unlock** while always producing unapproved successor on substantive change; decide review tier, segregation, true full-state no-op, and exact-version copy independently | No automatic approval, no content-only no-op, no unreviewed carry-forward |

The Normal Data permission `MODIFY_WORKPAPER_REVISION` is for **one exact immutable revision**. It is **not** a reusable `REVISE_WORKPAPER` permission and must not be broadened via a rename.

## B. Identity and grant choice — exact interfaces an approver must select

**Trust boundary:** TypeScript/webview fields (including actor, role, signoff type, "minor", session ID, time, grant ID, evidence IDs) are all **untrusted claims**. Only a reviewed native issuer may bind an actual person to a revocable session. A `#[cfg(test)]` fixture-constructed `VerifiedNativeWorkpaperSession` is not a credential. Source-level deny-all or a hypothetically positive preflight is not final transactional authorization.

| Candidate | Genuine human binding / revocation | Offline case | Non-negotiable weakness / validation |
| --- | --- | --- | --- |
| OS-account binding | Requires verified mapping from OS principal to named person; OS session switch and logout invalidate native session | Potentially possible | An OS account alone may be shared; malicious same-user process may spoof app calls; must explicitly decide DEC-02 |
| Organization identity provider + native broker | Individual account + fresh verified native proof; short-lived audience-bound tokens and session revocation | Must define loss-of-network policy | Token replay, cached offline verification, native secure storage and subject switching need independent adversarial tests |
| Locally provisioned offline identity | Human enrollment and independently protected private key/certificate, native challenge, separate admin custody | Possible when carefully designed | Recovery, revoked signer, clock rollback, copied key and malicious DB-owner handling are hard; no "first user is admin" |
| Explicit unverified local draft | No verified person; no professional signature, no privileged authoring claim | Yes | Must label as unverified and block reliance on PREPARED/REVIEWED/FINAL signoff labels |

**Proposed native write validation order** *(contract, not implementation)*: obtain issuer-bound person **outside** UI payload; begin SQLite `IMMEDIATE` transaction; revalidate person session and revoked/expired state against trusted issuer; resolve exact workpaper/engagement ownership; recheck current `READ_WORKPAPER` **and** `REVISE_WORKPAPER` rows; validate expected latest revision ID + full ancestor hashes; validate server-resolved exact version links, retention/custody and historical signoff snapshots; apply DEC-08 full-state no-op policy; write canonical seven-field hashed successor, separately-approved new evidence link IDs and unapproved status; historical signoff supersession and authenticated audit in same transaction; commit or rollback all. A standalone `Ok(())` preflight is never a write permit.

**Administrator bootstrap question:** name the first independent verifying authority, its possession/recovery proof, independent logging/audit custody, how the first administrator can be revoked, and how a lost admin is recovered **without** automatically conferring privilege on an OS username, caller string or the first database record. Until accepted, issue **zero** production workpaper grants.

## C. Legacy command containment — choose before any rollout

These commands are currently registered. **Do not claim they are disabled**. Severity and actual exploitability depend on the chosen desktop and threat model; a local webview command is not proof of internet exposure.

| Possible interim disposition | Benefit | Compatibility cost / risk | Preconditions |
| --- | --- | --- | --- |
| L1 — Keep as **explicitly unverified local draft**, with provenance/limitations made visible | Minimizes disruption to existing unsiged local workflows | Caller-asserted sign-off labels still risk being misunderstood; three-field form can silently exclude previous field values | Owner accepts risk in writing; surface visible unverified status **without claiming authentication**, backup/read-path verification and negative UX tests |
| L2 — Feature-gate legacy revision/sign-off **only after rollout review** | Stops new unsupported reliance where gated | Existing installations and offline users may lose create/sign-off workflows; feature flag itself is not identity | Inventory deployments, data export/read-only fallback, migration/rollback and documented affected operations |
| L3 — Disable legacy write/sign-off entirely after compatibility review | Strong stop to creating new unverified signed rows | Highest disruption to current users | Explicit product/security sign-off, read-only historical access, backup/restore and migration proof |
| L4 — Replace with reviewed lossless, authorized native successor + authenticated signoff | Long-term intended experience | Requires DEC-03/04/05/06/08 decisions and implementation | Independent code review and all native negative acceptance tests |

**Interim recommended boundary (proposal):** Do **not** connect the new editor to the legacy command or imply the legacy form/sign-off is verified. Preserve access to historical data; do not rewrite, auto-relabel or delete immutable sign-offs. Product/security owner chooses L1/L2/L3 with actual deployment and rollback evidence. Merely changing the four hidden fields from blank to old values would address some text loss but **would not** solve stale revisions, evidence omissions, forged sign-off identity or review-state confusion. It is therefore a separate scoped compatibility fix requiring review, not an authorized corrective Save.

### Legacy migration/rollback acceptance checklist (not completed)

- [ ] Inventory existing saved data and integrations; classify actual use of unverified PREPARED/REVIEWED/FINAL labels without claiming verified historical authors.
- [ ] Validate read-only access to all immutable old revisions/signoff/evidence snapshots and exports before/after a proposed gate.
- [ ] Confirm a backup/restore and rollback plan on every supported filesystem; no historical rewrite, fabricated re-attestation or signoff identity backfill.
- [ ] Test a workpaper where the four currently hidden fields contain non-empty Unicode/whitespace; no compatibility path silently blanks them.
- [ ] Test two edits from one old base and evidence-only change; no authorizing behavior is inferred from SQLite write serialization.
- [ ] Verify UI language distinguishes **unverified legacy record**, **new unapproved successor**, and **independently verified sign-off**.
- [ ] Obtain named product/security owner decision for L1/L2/L3/L4 and separate independent security review before enabling authoritative writes.

## D. DEC-08 specific product scenarios needing signatures, not more mock-only tests

| Scenario | Candidate desired UX | Mandatory decision |
| --- | --- | --- |
| One-character correction after reviewed revision | Permitted editor saves a new immutable **unapproved** revision without unlock; other six text fields exact; old review remains historical | Who may review the successor, segregation rule, carry-forward eligibility |
| Completely unchanged seven fields **and** unchanged exact evidence/review state | Potential explicit **non-mutating no-op**, not a sign-off supersession | Determine whether an intentional checkpoint warrants a separate audit event |
| Same text, different exact evidence version or review-relevant state | Do not assume no-op or auto-approve | Distinguish independent evidence-update authority from authoring |
| Old reviewer unavailable | Authoring remains possible **only if authenticated authoring was already approved**; successor stays unapproved | No provisional self-approval; product display/reliance rules |
| Minor editorial acknowledgement | Optional later verified, scope-limited independent acknowledgment on **new exact revision + exact evidence set** | Human methodology owner defines eligible classes and reviewer independence |
| Repeated or concurrent stale Save | Preserve local draft and show exact three-way conflicts, never silently rebase | Bounded lock retry and full-state evidence conflict policy |

## E. Acceptance evidence to attach to an eventual decision, without claiming completion

1. Named issuer/administrator/reviewer humans, identity-verification steps, approved supported platforms, and threat assumptions; session expiry/revocation, user switch, hostile webview and privilege escalation negative tests.
2. Separate reviewed exact-workpaper grant migration, no seeded admin, audited issuance/revocation, engagement/archival scope checks **inside the final transaction**, and cross-workpaper negative tests.
3. True end-to-end native Tauri command tests (not only fixture SQL) for signer spoof denial, revoked/expired person, no grant, stale CAS, seven-field losslessness, exact linked versions, full-state no-op and rollback.
4. Real protected-root/Windows DACL or alternative platform custody evidence, filesystem power-loss and backup/restore/retention proof where professional evidence is involved.
5. Owner-approved legacy disposition and deployment compatibility, signed acceptance/rollback record, human DEC-06 independent security review and separately authorized PR merge/release.

**Unapproved actions:** approving/closing Issue #129 or #138, merging draft PRs, enabling a Tauri corrective Save, promoting a `#[cfg(test)]` session into production, seeding admin/grants, declaring historical legacy actor labels verified, or presenting unsigned successors as approved. None is authorized by this packet.
