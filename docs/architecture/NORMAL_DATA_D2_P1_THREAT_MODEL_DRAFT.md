# D2-P1 — Normal Data controlled-custody threat model and platform decisions

**Status: DRAFT — security decision required; no platform or capture mode approved.**  
**Inspection baseline:** protected `develop` at `6cc12b7a0f90f0a1fddfc853120fdea96387adb4` (Stage AK).  
**Authority:** [ADR — Normal Data promotion boundary](ADR_NORMAL_DATA_PROMOTION_BOUNDARY.md) (proposed implementation gates) and [accepted ADR-0007](ADR-0007-evidence-capture-and-signoff-binding.md).  
**Related planning proposal:** [PR #127](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/pull/127), not yet merged when this draft was started. This threat model does not depend on a merge of PR #127.

This is the **first security-design input**, not implementation authorization, a D2 approval, a proof of custody, or a substitute for independent platform review. Findings describe observed source-code constraints at the baseline; open items remain unverified.

## 1. Goal, scope and prohibited shortcuts

Protect an explicitly requested transfer from a **specific immutable Normal Data comparison run** into separately retained, controlled versions of **source A bytes**, **source B bytes**, and the **exact frozen result artifact bytes**. The three roles have distinct identities and hashes. Only after trusted capture and a new eligible specialist revision may a later D3 transaction link exact controlled version IDs; D4 may show a deliberate confirmation UI after D2/D3. Viewing runs, verifying working sources or writing a `.ready`/v31 `.candidate` package is **not capture**.

**Out of scope for this P1 documentation change:** enabling actual authentication, granting filesystem access, making an approval decision from Windows ACL shape, certifying filesystem or SQLite durability, creating evidence records, creating or exposing Tauri commands, attaching workpapers, and producing a UI `Promoted` state. There are no approved exceptions for Windows.

## 2. Trusted observations versus missing trust anchors

| Component / trust boundary | Inspected fact at baseline | Not established |
| --- | --- | --- |
| `normal_data_promotion_policy.rs` | Native Rust policy expects `VerifiedPrincipal`; only a `#[cfg(test)]` constructor exists. Default policy denies and separate exact-resource grant types exist. | A production authenticated user/session or reliable revocation when a session changes. |
| `normal_data_permission_admin.rs` | `VerifiedGrantAdministrator` also only has a test fixture constructor; native admin event primitives exist. | A trusted production grant issuer, approved administrator bootstrap or hardware/OS identity binding. |
| `normal_data_capture_preflight.rs` | Checks exact transaction-scoped candidate material using an unconstructible-in-production principal; material remains working-data bytes. | A transferable approval token, formal capture, version issuance or final link. |
| `normal_data_staging.rs` and `normal_data_retention_store.rs` | Owned three-artifact staging/candidate bytes and digests, create-new file writes, file sync and recovery are implemented as **non-controlled** infrastructure. | Storage isolation from hostile writers, immutable controlled versions, cross-system atomicity and power-loss-safe directory publication. Windows `sync_dir` intentionally does not perform reliable directory fsync. |
| `normal_data_retention_root_posture.rs` | Native-only observational root/path checks. The Windows branch unconditionally returns `Windows DACL permissions and durable protected storage are not verified`. | Windows effective ACL/access policy, durable custody, race freedom or authority to enable capture. Unix owner/mode checks also do not prove all ACL/mount or hostile-same-user conditions. |
| `normal_data_windows_dacl_probe.rs` | Earlier Stages AH–AK added read-only native descriptor/SID/DACL structural checks. | Trustworthy effective ACL decisions, full trustee semantics, grant evaluation or controlled custody. |
| ADR-0007 | Controlled capture requires stable source identity, copied exact bytes, per-copy SHA-256, durable storage, committed metadata, and COMPLETE only when all required operations succeed. | Automatic permission to treat a candidate as a completed controlled version. |

**Boundary rule:** SQLite foreign keys, immutable rows, transactions and append-only tables establish *application/database consistency under their assumptions*. They are not authentication or tamper-resistant against the owner of the local database or storage. A passed `IsValidAcl`, `EqualSid`, or copied security-descriptor match is not an effective-permissions proof.

## 3. Assets and security properties to preserve

1. **Working provenance:** run UUID, frozen recipe version, exact dataset/document/content-version identities, historical source A/B SHA-256, semantic result digest, serialized result byte SHA-256 and immutable result JSON. Historical records must remain inspectable even if live links later change.
2. **Authorization context:** trustworthy caller identity and administrator identity; exact workspace-read, engagement-attachment and revision-write grants, status/expiry/revocation, target engagement and unsigned latest DRAFT revision.
3. **Capture material:** three distinct verified byte buffers, lengths and hashes; no swapped source-role, substitution of result semantic digest for artifact bytes, or mutable `latest` file.
4. **Custody:** private handle-bound storage roots, immutable controlled versions, on-disk durability, safe recovery/quarantine, prevention of unauthorized replacement/read/deletion, and an attributable append-only audit history.
5. **Reviewer integrity:** sign-off refers to a specific specialist revision and the exact controlled evidence version set; material new evidence cannot inherit sign-off on an earlier revision.

Security objectives: confidentiality of source/captured content; authenticity of principal and grant administration; integrity of each exact retained byte stream, metadata and audit event; availability and recoverability following interruption; and non-repudiation only to the degree the selected trusted identity/audit platform actually supports it. **Do not claim cryptographic non-repudiation from a local SQLite row.**

## 4. Adversaries and explicit trust assumptions

**Adversaries to test:** malicious or compromised frontend/caller (forged UUID, actor or role); unauthenticated local process; another local OS user; a same-OS-user process; an administrator or service with elevated filesystem/DB privileges; hostile share/mount/ACL inherited policy; an external writer racing source reads or staged files; and crash/power-loss at each filesystem/SQLite boundary. Treat symlinks, reparse points, mount swaps and directory replacement as adversarial inputs. Do not assume a process-owned root is isolated from another process running as the same identity.

**Unresolved policy choice:** which adversaries must be prevented versus merely detected? In particular, a host administrator, compromised process user, and owner of both SQLite and artifact directory cannot be declared out of scope without a **documented product security decision and consequence**. Encryption at rest, OS-protected keys or isolation may be required, but their implementation is **not chosen by this draft**.

**Trusted components are proposed, not certified:** native OS session source, policy authority, protected file/handle operations and a durable store with independently reviewed failure semantics. Untrusted inputs include all frontend parameters, UUID strings, path values, package manifests, working-data stage/candidate records, mutable source files, and timestamps/role strings.

## 5. Threat/abuse matrix and required negative demonstrations

| ID | Crossing / attack | Required fail-closed property and test |
| --- | --- | --- |
| TM-01 | Frontend sends someone else's `actor_id`, workspace UUID or admin role | No `VerifiedPrincipal` or `VerifiedGrantAdministrator` may be built from payload; audit native session binding and deny without trust anchor. |
| TM-02 | Stolen/expired/disabled session or revoked grant used after preflight | At final write, revalidate caller and all three **exact scoped** active grants in one SQLite transaction; stale preflight cannot authorize anything. |
| TM-03 | Grant issuer attempts scope widening, impersonation or silent reactivation | Trusted administrator authentication, explicit allowlist, append-only issuance/revocation event, rollback on failed event; never reactivate a revoked grant by changing DB state. |
| TM-04 | Source A/B swapped, same-size modified, unlinked/moved, or hash of old bytes asserted for new bytes | Reopen through approved native root, bind exact IDs and byte hashes; reject mismatch and preserve historical receipt without implying current integrity. |
| TM-05 | Result JSON reserialized or recomputed from changed sources | Preserve exact frozen serialized bytes and separate artifact SHA-256 from semantic digest; reject rewritten, recomputed or substituted results. |
| TM-06 | Symlink/reparse/ancestor/mount/path swap during file creation or read | Operate through verified, no-follow, identity-bound handles with rechecks. Snapshot/path inspection alone cannot approve root access. |
| TM-07 | World/other-user, inherited ACL, network-share or same-user writer changes retained objects | Demonstrate isolation/authorization of every supported writer and supported filesystem; effective ACL/access review, negative hostile-writer tests, or explicit unsupported-platform denial. |
| TM-08 | Power loss after file sync, rename, directory metadata change or DB commit | Define commit/recovery order, durability guarantees per platform; replay all crash points and never report COMPLETE when required data or metadata is missing. |
| TM-09 | Two files retained but third missing, corrupt or associated with a different run | Require all three independently immutable, versioned artifacts before completion/link; quarantine partial/mismatched sets without promoting candidates. |
| TM-10 | Another SQLite writer revokes access, reviews or signs a revision during capture/link | Transactionally recheck grants, latest unsigned revision, engagement state and frozen identities; deny on conflict without modifying historical signed content. |
| TM-11 | Local DB/storage owner edits grant, audit row or captured bytes | Explicitly decide assumed adversarial privilege and mitigation; acknowledge that SQLite immutability triggers and SHA-256 against attacker-editable metadata do not defeat this writer. |
| TM-12 | Crash/restart leaves provisional artifacts, orphan evidence rows or stale status | Audited idempotent recovery: no partial as COMPLETE, no orphan formal link, deterministic QUARANTINED/FAILED handling with safe cleanup and no silent replacement. |
| TM-13 | New evidence attached to signed/reviewed historical revision | Require new eligible revision and later review/sign-off; old sign-off remains bound to its exact former evidence set. |
| TM-14 | Read-only Windows descriptor probe returns valid copied DACL shape | Keep Windows `inspect_retention_root_posture` denial unchanged; no structural-only test can satisfy protection or durability gate. |

A passing unit test on a synthetic ACL or a CI runner is evidence of that specific code behavior only. It is not independent confirmation of hostile-user resistance or crash durability.

## 6. Supported-platform decision register — NOT APPROVED

| Platform | Current position | Evidence required before an affirmative security decision | P1 decision |
| --- | --- | --- | --- |
| Windows local NTFS | Observational path, owner SID and DACL structure only; mandatory protected-root denial remains. | Review token, share/inherited ACL/effective rights, handle-bound no-follow root and file operations, host/same-user adversaries, reliable directory durability, crash/recovery behavior and repeatable hostile-writer tests. | **OPEN — default DENY** |
| Unix/POSIX local filesystem | Owner/mode checks and some no-follow ancestor observation are implemented, but those checks are not custody certification. | Filesystem/mount ACL and same-user threat policy, trusted openat-style/handle-bound operations, fsync of file plus namespace mutations, replay after crash and hostile-writer tests. | **OPEN — default NO FORMAL CAPTURE** |
| Removable/network/cloud-synced shares | No supported protected-retention claim documented in examined sources. | Separate independent mount/share/cloud consistency and trust threat assessment. | **NOT APPROVED; exclude pending decision** |
| Unknown/unsupported OS | Fail-closed platform-specific posture already provided. | A separately documented adapter and security assessment. | **NOT APPROVED** |

Do not resolve an OPEN cell with “CI green” or a speculative risk acceptance. A platform-specific decision must cite tested environmental prerequisites, operating-system/filesystem versions, storage configuration, threat scope, and a named human security reviewer. If supported environments differ, gate each explicitly; do not silently generalize.

## 7. Open decisions for product owner and independent security reviewer

- **DEC-01:** Which platforms and filesystems must support *formal controlled-evidence capture at initial release*? Is Unix-only acceptable if Windows proof remains unavailable?
- **DEC-02:** Must custody resist *same-OS-user malware*, *local administrators* and *database owners*, or are any treated as trusted with a documented operating/security model?
- **DEC-03:** What audited native identity mechanism binds session and admin to enrolled grant subjects, and how do logout, process restart, impersonation and expiry work?
- **DEC-04:** Who can bootstrap the **first** trusted administrator and how are recovery, revocation and loss-of-access handled without granting through frontend arguments?
- **DEC-05:** What storage backend and crash-durability contracts will be supported, and what independent power-loss tests/evidence establish them?
- **DEC-06:** Who is authorized to approve the custody design and what artifacts/criteria are necessary to close D2 (threat-model version, platform test matrix, security review and residual-risk acceptance)?
- **DEC-07:** Which retention/deletion, legal hold, backup/restore and quarantine policies must govern controlled versions, including a compromised storage root?
- **DEC-08:** Which D3 specialist revision types are in scope for first release? No signed/reviewed revision may be mutated, regardless of UI request.

Each decision must be recorded with alternatives considered, owner/approver, date, rationale, accepted constraints and specific negative verification tests. **None is approved by this document.**

## 8. D2-P1 completion gate and next-package dependency

- [x] Draft an explicit baseline inventory with links to the live module/ADR paths above.
- [x] Enumerate assets, actors, trust boundaries and concrete fail-closed abuse tests.
- [x] Record Windows vs Unix vs external-storage positions and **unresolved** decisions.
- [ ] Independent security reviewer checks local OS/store threat assumptions and the abuse matrix.
- [ ] Product owner defines intended platforms, hostile-writer tolerance and release boundaries (DEC-01, DEC-02, DEC-08).
- [ ] Security/architecture approvers decide identity issuer/admin bootstrap, protected storage and crash durability contracts (DEC-03 through DEC-07), including signed risk decisions and test evidence.
- [ ] Update the governing ADR/roadmap with the accepted platform decision **without** weakening the unconditional Windows denial except after a separately scoped, independently validated and expressly authorized implementation.
- [ ] Confirm P2/P3/P4 dependency and scope after sign-off; all mandatory exact-head CI/Security checks pass on each PR.

**P1 is NOT complete while a decision is OPEN.** This PR may be reviewed/merged as a *draft threat model only*, never as an authorization to launch D2 capture or change Windows storage acceptance. Its safe success condition is better-defined questions and tests, not new runtime capability.
