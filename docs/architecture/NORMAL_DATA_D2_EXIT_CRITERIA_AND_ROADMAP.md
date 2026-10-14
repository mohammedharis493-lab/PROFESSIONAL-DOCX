# Normal Data D2 exit criteria and delivery roadmap (planning only)

**Status:** Proposed engineering plan; **not** a decision to enable protected storage, controlled evidence, authorization, or promotion.  
**Baseline inspected:** protected `develop` squash commit `6cc12b7a0f90f0a1fddfc853120fdea96387adb4` (Stage AK, 2026-10-14).  
**Governing decisions:** [Normal Data promotion boundary](ADR_NORMAL_DATA_PROMOTION_BOUNDARY.md), [ADR-0007](ADR-0007-evidence-capture-and-signoff-binding.md), and the Normal Data mode ADR. Where there is any conflict, the governing ADR and existing fail-closed code win.

## Purpose and what is already complete

Stages AH (owner SID), AI (group SID), AJ (owned DACL identity) and AK (owned DACL size/count/ACE metadata) are **completed read-only, negative prerequisites**. Earlier D1 and D2A provenance/source-hash preflight, native-only transaction-bound policy, default-empty v28-v29 grant registry/audit skeleton, transient three-buffer verified material, private stage/candidate preparation, and v30-v33 recovery observations are documented as implemented. They are **not** production identity, formal controlled evidence, or a permission to promote.

Current `normal_data_retention_root_posture::inspect_retention_root_posture` still unconditionally refuses **Windows** protected-root approval with `Windows DACL permissions and durable protected storage are not verified`. Unix mode/ownership checks are also observations, not custody certifications. A verified copied DACL shape says nothing about effective access, hostile writers, durable synchronization, or atomicity across disk and SQLite. D2/D3/D4 remain deferred. Do not turn this roadmap into a sequence of additional automatic Windows DACL observation stages.

## D2 hard exit criteria (all must be demonstrably satisfied)

The items below are **proposed exit tests**, not assertions that the existing implementation satisfies them.

- [ ] **D2-01 — Threat model and platform decision.** Record trusted operating principals, administrator authority, local hostile users (including same-user/privileged writers), share/mount/reparse/symlink threats, SQLite tamper assumptions, concurrency, power loss and supported platforms. Obtain an explicit security/architecture decision for each supported custody platform. If credible Windows DACL/effective-rights and crash durability proof is unavailable, keep Windows protected-root and controlled capture **disabled**; no blanket exception.
- [ ] **D2-02 — Real authenticated principal.** Production-only identity is established by an auditable trusted native session, not by `actor_id`, caller UUID, UI role, untrusted Tauri input or a test-only constructor. Verify session expiry, change, impersonation and revocation; ensure unresolved identity is denied.
- [ ] **D2-03 — Trusted grant administration and authorization.** Authenticate and authorize actual administrators; issue/revoke narrowly scoped, time-limited subject grants against exact workspace, engagement, and workpaper-revision resources with an immutable action history. Preserve the currently default-empty registry and fail-closed behavior. Prove a stolen or asserted ID cannot create a principal or grant.
- [ ] **D2-04 — Verified protected storage root and filesystem policy.** Establish security evidence for canonical, handle-bound, no-follow filesystem traversal, ownership, *effective* rights including inherited ACLs, mount/share policy, path-swap adversaries and least-privilege isolation. Bind checks to actual open handles and storage operations. Mere mode bits, owner SID or DACL pointer validation are insufficient.
- [ ] **D2-05 — Durable, immutable three-artifact custody.** Specify and implement formal retained versions separately for original source A, original source B, and the **exact frozen result artifact**; verify bytes, lengths, per-original hashes, serialized result byte hash and distinct result semantic digest. Define durability (including directory metadata and power-loss) and prevent replacement/overwrite, version swaps, unauthorized deletion and unauthorized read.
- [ ] **D2-06 — Atomic/recoverable capture lifecycle.** Specify stages and crash points between filesystem operations and SQLite commits; distinguish provisional files, final retained artifacts, committed evidence records and abandoned/quarantined pieces. Demonstrate idempotent recovery with no false `CAPTURED` state, orphan links or mixed-version triples. Never relabel v31 candidates or `.ready` stages as controlled evidence.
- [ ] **D2-07 — Audit/retention policy enforcement.** Append exactly attributable, immutable capture/retention/cleanup/revocation event history in a transactionally consistent manner; enforce exact retention and cleanup rules, never merely a best-effort log. Document limitations against a compromised local database/storage owner and obtain appropriate security sign-off.
- [ ] **D2-08 — Final authority and independent acceptance.** Within the final SQLite `IMMEDIATE` write boundary, independently recheck authenticated caller, all three active scoped grants, exact immutable run/recipe/dataset versions/hashes, engagement/revision eligibility, sign-off/review state and all three formally retained versions. Demonstrate negative tests for revoked grants, archived or signed targets, swapped/hacked bytes, symlink/path changes, mid-capture crash, rollback and power-loss behavior. Complete independent threat-model/security review and all mandatory exact-head CI/Security jobs.

**Fail-closed rule:** Until every applicable exit criterion has documentary evidence, negative tests, and separate explicit security approval, D2 is **not complete** and no production permission, formal capture or promotion command/UI may be enabled. This checklist cannot waive ADR-0007 or the unconditional Windows denial by itself.

## Proposed finite work packages and dependencies

This is an engineering **estimate**, not a commitment to a fixed number of PRs or an instruction to implement them without separate approval. Each substantial change needs a scoped PR, ADR review, negative regression tests, six successful exact-head jobs, and its **own explicitly authorized merge**.

| Package | Scope, main deliverable | Dependencies | Estimated PRs |
| --- | --- | --- | ---: |
| D2-P1 | Threat model, supported-platform decision, formal custody invariants and independent security sign-off criteria | Existing ADRs | 1 |
| D2-P2 | Trusted production principal/session issuer and fail-closed identity lifetime/revocation tests | P1 | 1–2 |
| D2-P3 | Trusted administrator authentication plus exact grant enrollment/issuance/revocation audit activation | P1, P2 | 1–2 |
| D2-P4 | Platform-specific protected-root design and native handle-bound effective-access/race proof (Windows remains denied absent independently verified solution) | P1 | 2–3 |
| D2-P5 | Protected, durable, immutable storage for three exact independently versioned artifacts | P1, P4 | 2–3 |
| D2-P6 | Recoverable formal capture protocol, fail-closed failure-state transitions and identity/digest-bound evidence records | P2, P3, P5 | 1–2 |
| D2-P7 | Capture/retention event ledger, recovery reconciliation and cleanup/retention policy enforcement | P5, P6 | 1–2 |
| D2-P8 | Final transaction-bound rechecks, adversarial Windows/Unix/crash tests, independent D2 security acceptance | P2–P7 | 1–2 |
| **D2 total** | **Eight proposed packages; estimates may change after P1** | | **10–17** |

## After D2: still separate work

| Boundary | Proposed delivery | Estimated PRs |
| --- | --- | ---: |
| D3 — controlled atomic linkage | Bind the three exact retained version IDs to the selected **new eligible unsigned specialist revision**, enforce current permissions/state under a final transaction and append audit; negative tests forbid signed/reviewed revision mutation and wrong/swapped versions | 2–3 |
| D4 — deliberate user flow | Only after D2 + D3 and separate authorization: human-confirmed exact run/source/target/capture-policy UI, truthful success/failure display, no misleading automatic `Promoted` marker | 1–2 |
| **Estimated remaining total** | **D2 + D3 + D4, not counting this planning-only PR** | **13–22 PRs** |

These PR ranges are **planning estimates**, not verified effort or a promise that Windows custody can be certified. If P1 finds an unresolvable platform, hostile-writer or power-loss guarantee, implementation **stops** at a documented refusal instead of manufacturing a release deadline or relaxing the denial. D3 and D4 must not be started as live authorization/promotional features until D2 passes its exit gate.

## Evidence required to close this plan

For each D2 package, the review must link the exact PR and tested commit, ADR/risk decision, automated **and** adversarial negative test results, and the applicable evidence for the exit criteria above. P8 supplies a final matrix showing every D2-01…D2-08 requirement proven, explicitly deferred by a blocked platform, or still failing. A deferred/blocked requirement cannot be marked complete on that platform.

**Current classification:** Stage AK is merged; **D2 is not complete**, D3 is not implemented as authorized linkage, D4 promotion UI is deferred. Nothing in this document changes those facts.
