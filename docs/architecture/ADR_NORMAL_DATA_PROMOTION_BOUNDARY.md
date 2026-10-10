# ADR — Normal Data run-to-specialist promotion boundary

**Status:** Proposed (implementation gates; not an authorization grant)  
**Date:** 2026-10-10  
**Applies to:** Slice D of [ADR_NORMAL_DATA_MODE.md](ADR_NORMAL_DATA_MODE.md)

## Decision and existing invariants

The Normal Data comparison domain (schema v25–v27) is **working data**, not controlled audit evidence. Its completed run stores a recipe-version UUID, both SHA-256 digests for source *bytes read at execution*, a result digest, and immutable result JSON. It does **not** retain both source files or establish an authenticated approver. Historical run viewing must remain possible after a linked source moves, changes or disappears; historical viewing must not assert that source bytes remain available.

The accepted [ADR-0007](ADR-0007-evidence-capture-and-signoff-binding.md) requires formal review to bind exact workpaper revisions and captured controlled-evidence versions. A Normal Data run cannot satisfy that requirement by a link to a filename, current document, "latest" dataset, result digest alone, or a Tauri caller's asserted actor name.

## Trust/authorization constraints

1. Keep **working-data reference**, **promotion request**, **controlled capture**, **specialist attachment** and **review/sign-off** distinct states; labels and progress metadata must not imply authorization.
2. Every requested promotion must be an explicit user action identifying a **specific completed comparison-run UUID**, exact **recipe version**, source dataset versions and hashes, and an exact target **workpaper revision** or specialist target supported by the relevant domain. No implicit promotion on save, run, search, view or export.
3. Validate the stored immutable run against the frozen recipe, dataset A/B identities, immutable source hashes and persisted result digest. Do not reinterpret a changed linked source as historical input. Source hashes and result digest have different meanings.
4. Before declaring preserved source evidence, resolve approved-root source identities through the existing native canonical path boundary; read stably and verify exact size and SHA-256 against the *historical run* for both sources; capture each original via the existing controlled-evidence workflow into retained immutable storage. If either is missing, changed, unreadable or capture fails, **do not finalize**.
5. Preserve the stored run result as its own immutable, versioned artifact if reviewers must rely on that computation (a stored JSON record alone does not prove retained original source bytes). Any representation must bind the exact frozen run and result digest, not a new computation on latest inputs. Do not silently treat cached previews or computed output as captured originals.
6. Do not append a new evidence link to an existing signed/reviewed workpaper revision and imply that the prior sign-off covers it. Material new attachments require a new revision and subsequent review under existing specialist lifecycle rules. Do not modify historical sign-offs.
7. **No authenticated role authorization is currently established by the Normal Data feature.** Never infer it from `actor_id`, role strings, client association, button visibility or an asserted Tauri argument. A formal cross-domain promotion command must fail closed until its caller identity, permission to read the source workspace, permission to modify the exact specialist target, and review-state policy are actually enforced and tested.
8. Do not expose arbitrary file paths, source bytes, or uncontrolled document identifiers to the frontend. No new filesystem grants, external upload service, or auto-promotion facility is warranted.

## Safe implementation sequence

**D1 — native inspection only.** Add a bounded, read-only provenance/readiness query that takes only a completed run UUID. Resolve frozen recipe/dataset identities from the database, return source A/B SHA-256 and result SHA-256 together with their distinct meanings, and indicate integrity preconditions. The query must not capture, attach, mutate, or claim authorisation. A historical receipt is available even when linked source bytes have changed; any live verification status is a new, separately labelled observation.

**D2A — live source-integrity preflight (implemented separately from authorization).** An explicitly invoked native command re-resolves exact dataset versions and re-reads both linked sources through the approved-root, stable-read, bounded SHA-256 verifier. Return two timestamped observations only if each matches the historical run's exact source hash. Reject changed, unavailable and invalid sources without any capture, attachment, state mutation or authorization grant. These reads are sequential and do not produce an atomic two-file snapshot or a durable preservation guarantee.

**D2B — capture policy and authenticated authorization (not implemented).** Establish a trusted caller identity and enforced permissions for both the Normal Data workspace and specialist target; define required roles, target revision state, capture lifecycle and failure cleanup. Add cross-workspace/cross-engagement negative tests and avoid trusting client-supplied identity. Keep this separate from the read-only receipt and the read-time source observations.

**D3 — controlled, atomic linkage.** After D2, use an explicit request to capture (or refer to rigorously verified retained copies of) each required original and the result artifact, then transactionally bind the exact versions to a newly reviewable specialist revision with append-only audit history. Refuse wrong versions, mismatched hashes, concurrent target changes, archived targets, missing permission, incomplete capture and signed-revision mutation.

**D4 — UI.** A human-confirmed promotion flow may be added only after D2/D3, displaying the selected immutable run/source IDs, target revision, capture policy, permissions and verification status. Avoid a misleading "Promoted" badge for a working-data reference.

## Minimum negative regression matrix

- No client, engagement or workpaper requirement for ordinary workspace creation, dataset declaration or comparison execution
- Unknown/malformed run UUID and forged source/recipe/version identifiers are rejected
- Same-size changed linked bytes and moved/unavailable source cannot be considered verified historical sources
- Fingerprinted-only source cannot become controlled evidence via an asserted hash
- A stored result digest cannot be substituted for either source digest
- Historical run remains inspectable after linked-source changes, without a false current-verification claim
- No attached evidence or signed-off workpaper may be mutated indirectly by a read-only operation
- Wrong workspace, wrong engagement, unauthorized caller, finalized/signed revision, and capture failure block future promotion
- Existing ledger/TB/FS specialists and controlled evidence capture stay unchanged

## Scope of D1

D1 supplies provenance and policy clarity, **not promotion**. A valid UUID and database record are not proof of permission to attach it to any specialist target. Subsequent code reviews must verify current code and migration behavior against this ADR, not infer these capabilities from wording.
