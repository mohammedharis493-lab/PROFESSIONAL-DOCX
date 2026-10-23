# Optional document matching advisory — implementation handoff (DRAFT)

**2026-10-22. Nonshipping and unmounted.** This is a separately reviewable client-only contract, **not** a replacement for Professional DocX document management, formal book reconciliation, evidence authority, or an approved native persistence workflow. It does not change the user document, captured bytes, immutable workpaper history, statutory assessments, or sign-off.

## Product requirement

1. **Documentation remains usable without matching.** Users open and manage their original documents and any extracted field snapshot without importing books, running comparison, or re-uploading a document.
2. **Matching is auxiliary.** Only an explicit opt-in comparison against a deliberately selected reference record produces a report; a missing book/reference or no active comparison rules results in `no-reference`/`no-rules`, never a document rejection.
3. **Discrepancies are suggestions, not source mutations.** `difference` does not prevent opening, reading, annotating, or managing the original document. Absence/invalid comparability yields `not-compared`, *not* evidence that the source document is defective.
4. **Correct extracted metadata separately from the original.** A future reviewed UI can offer targeted extraction-metadata corrections with provenance; it must never silently rewrite, recreate or require re-upload of the original file. This first increment does **not** implement metadata corrections.
5. **Review flags are separate and untrusted.** An acknowledged or dismissed suggestion is a local display annotation only, not permission, a professional sign-off, audit event, compliance determination, or durable case disposition.
6. **No inferred voucher/reconciliation truth.** The engine does not identify invoice types, infer matching candidates, normalize ambiguous invoice numbers, convert currencies, infer accounting rules, or reconcile ledger records. A later voucher classifier/extractor must have its own verified schema, field provenance, missing-data review and consent behavior.

## Code boundary

[`src/document-match-advisory.ts`](../../src/document-match-advisory.ts) exports:
- `inspectOptionalDocumentMatch(document, reference, rules, requested = false)`, returning an immutable, explicitly `detached-optional-matching-observation-only` result with `blocksDocumentUse: false`, `requiresDocumentChange: false` and `requiresReupload: false` on every outcome.
- `noteOptionalMatchFinding(report, ruleId, disposition)`, creating only an immutable, nonpersistent `local-advisory-review-only` annotation for a differing or unavailable comparison.

All rules require explicit reference and document field keys. String comparisons are exact, and optional amounts are safe integer minor currency units, not floating-point guessed money. Currency equality must be included as a separate explicit rule when it matters. Missing/blank fields, absent source/reference or unsafe amount values are non-comparisons rather than false mismatches or false matches.

[`tests/document-match-advisory.test.mjs`](../../tests/document-match-advisory.test.mjs) characterizes opt-in default, absent books, zero rules, discrepancies vs consistent fields, missing values, numeric type failures, Unicode/whitespace exactness, immutable input/output, scoped review annotations, strict schema validation and no side-effect imports. `npm run test:document-match-advisory` is an independent [CI](../../.github/workflows/ci.yml) frontend step.

## Integration / acceptance remains separate

Before any live UI/backend binding, review the application screens and stored extraction schema. Add a nonblocking **Compare** action in the document context and an optional suggestion panel; display existing source and current extracted fields without re-upload. Keep any corrected extraction field as a new, explicitly reviewed metadata layer with source/extractor provenance, and keep reference-record changes separate. Require opt-in for new comparisons, permission-checked read paths and a reversible user dismissal/acknowledgement UX.

Formal statutory sign-off or review may require independent mandatory compliance checks, but this **optional book/document matching helper must never be repurposed as an authorization or formal evidence gate**. Do not infer a verified person's identity from a local review annotation. The old reconciliation workflows remain available/unchanged.

**Workpaper security hold unaffected:** [Issue #129](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/issues/129) remains open for DEC-01…08; [Issue #138](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/issues/138) remains open for legacy callable routes; draft [PR #137](https://github.com/mohammedharis493-lab/PROFESSIONAL-DOCX/pull/137) remains unmerged with its own test-only editor/custody work. This proposal does not choose L1–L4, fix native issuer/grants, attest captured bytes or authorize merging professional security features.


## Detached read-only suggestion panel (2026-10-22 continuation)

[`src/document-match-advisory-panel.ts`](../../src/document-match-advisory-panel.ts) provides an intentionally **unmounted React presenter** for an already computed optional matching observation. It always states that the original document and extracted record are unchanged, reports `not-requested`, `no-reference`, `no-rules`, `consistent`, `incomplete`, or `suggestions` without calling anything invalid, and shows exactly which extracted/reference fields differed or could not be compared. User-supplied field text is escaped through ordinary React text rendering. It has no upload, edit, Save or submit control.

An optional callback can show two ordinary `type="button"` controls per actionable difference/unavailable field. They emit **only locally scoped acknowledgement/dismissal annotation objects** with document/extraction/reference/rule identifiers, and never write a database, alter extracted fields, or certify a reconciliation. With no callback, the panel has no inert action buttons. Nothing is mounted from `src/App.tsx`, and the existing document and reconciliation routes are unchanged.

[`tests/document-match-advisory-panel.test.mjs`](../../tests/document-match-advisory-panel.test.mjs) adds server-rendered text/safety and direct callback regressions for opt-out, absent reference/rules, missing or incompatible values, mixed consistent/suggested items, HTML escaping, nonmutating local annotations and isolation. The dedicated advisory npm script runs both the pure comparison engine and this separate visual contract in CI. This is **not** an end-to-end integrated user experience; actual document/extraction read permissions and UI association behavior require another reviewed implementation.


## Read-only indexed-file identity adapter (2026-10-22 continuation)

[The detached adapter](../../src/document-match-advisory-adapter.ts) and [regression tests](../../tests/document-match-advisory-adapter.test.mjs) introduce a bounded readiness bridge from the existing indexed document viewer's `documentId` and `fileInstanceId` to a **separately supplied** field snapshot. A caller must provide matching document/file-instance identities, an explicit extraction snapshot ID, and scalar fields before the pure advisory engine can run. The default still yields `not-requested`; the caller must opt in to comparison and choose a reference explicitly. Unavailable, malformed or cross-file extraction input returns a nonblocking `unavailable` result without comparison or source operations. Missing books/rules, incomplete fields, discrepancies, and local dismissals remain nonblocking.

### What the present application actually provides

- `src/App.tsx`'s indexed-file view model currently exposes `documentId`, `fileInstanceId`, name/path/extension/size/modified timestamp/availability. The backend `IndexedFilePreviewDto` in `src-tauri/src/lib.rs` and `IndexedFilePreviewRecord` in `src-tauri/src/persistence.rs` have the same identity/metadata shape, **not an extracted voucher-fields map**.
- The existing `list_indexed_file_preview`, `search_documents`, `preview_text_file_instance`, PDF/image, Word and spreadsheet preview flows are for discovery or display. They are **not** an extraction-snapshot read API and must not be silently converted to confident voucher-field values.
- Existing formal reconciliation and Normal Data operations are independent: this adapter imports neither them nor any Tauri/native permission, data writer, source loader or accounting matcher.

The adapter intentionally does **not** invent a source of `ReadOnlyExtractionFieldSnapshot`, attach a document to books automatically, infer document type, mint an extraction ID from a timestamp, trust a filename as an invoice reference, import a ledger, or use preview text as verified structured extraction. Merely matching `documentId`/`fileInstanceId` **does not prove extraction provenance or freshness** (the current viewer DTO has no pinned extraction/content-version relationship). Accordingly, this code remains unmounted and cannot be used as a formal evidence guarantee.

### Before any live UI binding

1. Independently review and implement or identify a read-only, permission-checked source for extraction fields, with explicit source content/version association, provenance, and a freshness/staleness model. Its schema must be verified rather than assumed.
2. Pass only the currently selected indexed file's identity, a deliberately selected extraction snapshot and an explicitly chosen reference/rule set to the adapter. Never associate records by heuristic name/amount or run comparison in the background.
3. Render an opt-in Compare action and the existing optional panel only in a separate reviewed integration after permissions, stale-data behavior and normal document-navigation regression tests. An unavailable adapter result must never prevent preview, open, search, pinning or normal document operations.
4. Keep original source files, extraction corrections, durable acknowledgements, professional approvals and authenticated security decisions out of this draft scope. PR #137 and open Issues #129/#138 are untouched.

This increment's source-preservation argument is a narrow **static/read-only call-graph guarantee**: the adapter invokes only the pure advisory engine, accepts data objects by reference without modifying them, and returns immutable observations. It does not read or write original bytes. End-to-end runtime integrity, read authorization and extract provenance remain **unverified** because nothing is mounted in the live app.


## Explicit reference selection and volatile Compare workflow (2026-10-22 DRAFT continuation)

New files: [`src/document-match-advisory-flow.ts`](../../src/document-match-advisory-flow.ts),
[`src/document-match-opt-in-control.ts`](../../src/document-match-opt-in-control.ts), and
[`tests/document-match-advisory-flow.test.mjs`](../../tests/document-match-advisory-flow.test.mjs).

This is a **detached, unmounted, nonshipping** opt-in user interaction boundary layered over the already verified read-only indexed-file adapter. No new native command, extraction reader, accounting book association, upload/save form, persistence, signed evidence activity, or change to `App.tsx` is introduced.

- The control must be given one **fresh visit-local key** for each viewer opening. It is an ephemeral UI context token, **not** an identity proof, stable document identifier, source hash, content version, authenticated actor, or evidence attestation. It must not be persisted or reused when the viewer closes/reopens.
- A host may supply a **separately permission-checked, read-only** extraction snapshot and an explicit list of reference records and mapping rules. This infrastructure **does not currently exist for structured voucher fields in the indexed viewer**; do not infer it from the filename or document preview. Without a bound snapshot or references, compare is disabled and ordinary document use remains intact.
- Only an actual dropdown selection creates a scoped `volatile-optional-reference-selection` object, and **only an explicit Compare click** may invoke `requestExplicitOptionalMatch`. Missing or duplicate candidate IDs are not silently picked. Unselected and stale selections cannot initiate a comparison. A missing book is not a document failure.
- The immutable `volatile-optional-comparison-review` holds an ordinary *noncryptographic*, local-only `inputKey` used to hide stale results when the indexed file, viewer visit, extraction snapshot, chosen reference, reference field values, or comparison rules change. This does **not** attest source provenance, validity, freshness or financial correctness; the read-only extraction provider is still responsible for authenticated source-version association.
- A current observation is rendered using the existing passive advisory panel. Optional acknowledge/dismiss actions are **local-only display annotations**, not a professional approval, trusted sign-off, custody event, persistence operation or accounting correction. Clearing a local review does not edit the document. Invalid external props or rules produce a nonblocking unavailable result rather than altering document use.
- Component state is **controlled**: the eventual host must hold only volatile `selection`/`review` and local disposition state and refresh the visit token for every distinct opening. There is no automatic background comparison or automatic proposal of books. Before actual mounting, independently verify read permissions, version provenance, context-token rotation and viewer open/close safety in end-to-end tests.

The dedicated npm advisory script now runs the flow/control regressions alongside the original engine, panel, and indexed adapter suites. No live frontend route or production privilege is changed. Existing PR #137, Issues #129/#138, DEC-01..08 and L1–L4 containment/security decisions remain independent and unresolved.


## Existing content-version history correlation gate (DRAFT, 2026-10-22)

[`src/document-match-source-version-gate.ts`](../../src/document-match-source-version-gate.ts)
provides a **pure, detached, non-authorizing metadata correlation** against the native
`list_document_version_history` DTO already exposed in `src-tauri/src/lib.rs`.
[`tests/document-match-source-version-gate.test.mjs`](../../tests/document-match-source-version-gate.test.mjs)
covers both legitimate metadata correspondence and missing, changed, inconsistent or maliciously
malformed inputs. The existing advisory CI script includes its tests.

The current read-only native history DTO exposes `contentVersionId`,
`fileInstanceId`, `verificationState`, `sourceStableDuringRead` and `sha256Hex`.
The file-instance identity is read directly from the existing persisted `content_versions`
row (as of the incremental draft change described below). The DTO still does **not**
contain `documentId` per returned row: the caller must explicitly pass the document ID
it requested. The future extraction snapshot schema does **not** currently
exist; this module defines **claimed** `sourceContentVersionId` and `sourceSha256Hex`
only as future adapter inputs, never as fields silently taken from filenames or previews.

A correlation is allowed only for an `AVAILABLE` indexed source with the same document and
file-instance identities as the supplied snapshot, a **deliberately named** content-version ID,
an exact 64-character lowercase hexadecimal SHA-256 that equals the corresponding native history
row, and `HASH_VERIFIED` + `sourceStableDuringRead === true`. History must belong to the
explicitly requested document ID. Duplicate rows from the native controlled-evidence JOIN are
accepted only when the version/hash/verification/stability attributes agree; otherwise the
gate fails closed. No available snapshot, history, reference or mapping is inferred. Actual
comparison remains disabled by default; truthy non-Boolean inputs do not imply consent.
Every unavailable outcome explicitly preserves normal document use and requires no document
editing, replacement or re-upload.

**Critical limitations: metadata correspondence is NOT source provenance.** A caller could
fabricate snapshot version/hash claims from a legitimate version history; matching them does
not establish that extracted fields were produced from those bytes, that a read was authorized,
that extracted fields were actually derived from that indexed instance or that the current
filesystem bytes are unchanged. The native history now exposes each version's stored
`fileInstanceId`, which the detached gate checks; this is *identity corroboration*,
not extraction provenance or current source-byte verification.
The check **must not** be used as the sole gate for live UI or an assurance/approval statement.
A separate permission-checked extractor/read-only field provider, binding to the exact
content version and file instance with independently established provenance, remains necessary.
This does not modify any native APIs, migration, production DB, accounting book, document source,
custody, user permissions, workpaper PR #137, or security decisions in Issues #129/#138.


## Exact native content-version to file-instance identity (DRAFT continuation, 2026-10-22)

The pre-existing SQLite `content_versions` table has always contained a nonnullable
`file_instance_id` foreign key, but the existing `list_document_version_history`
read-only response omitted it. This increment **extends that existing response only**:

- `src-tauri/src/persistence.rs`: read the stored `cv.file_instance_id` as part of the
  existing document history SELECT, preserving all previous field indices and query ordering,
  and add it to `DocumentVersionHistoryRecord`.
- `src-tauri/src/lib.rs`: expose the existing field as `fileInstanceId` via
  `DocumentVersionHistoryDto` camelCase serialization. No new Tauri command, new permission
  capability, write endpoint, schema migration or database mutation is introduced.
- `src/App.tsx`: update the existing viewer DTO type for the added read-only field;
  no matching action or new UI is mounted and no existing viewer behavior changes.
- `src/document-match-source-version-gate.ts`: **fail closed** when the chosen history
  content-version row omits `fileInstanceId`, belongs to a *different* instance, or has
  conflicting duplicated instance metadata. The row's original `contentVersionId` and
  stored `fileInstanceId` must agree with the independently selected file and future
  explicitly claimed extraction snapshot **even if two files share identical hashes**.
  All failure states remain nonblocking and cannot request document edits or re-upload.
- `tests/document-match-source-version-gate.test.mjs` and the existing native
  synthetic-copy test in `src-tauri/src/indexer.rs` verify exact-instance binding,
  missing identities, duplicate conflicts and successful independent read-only version
  history for two byte-identical indexed documents.

**Boundary:** The new field is stored metadata, not a proof of extractor input, file-read
authorization, snapshot provenance, current disk stability or financial correctness.
An adversarial caller can still submit fake extraction fields and claim an existing
version/hash; a production extraction provider must independently establish those facts
with approved read permissions and authenticated provenance *before* this UI is mounted.
Nothing is changed in PR #137, professional custody/sign-off, production source files,
accounting books or user-generated documents. The new metadata field is read through an
already-existing Tauri history command; its existing authorization posture is NOT thereby
security-approved. The independent decisions in Issues #129 and #138 remain OPEN.


## Detached UI correlation prerequisite and version-claim invalidation (DRAFT continuation, 2026-10-22)

The detached `DocumentMatchOptInControl` now requires the same **read-only
source-version/file-instance correlation** as the dedicated metadata gate.
Its controlled props include a future `VersionClaimedExtractionSnapshot`,
`indexedFile.availabilityState` and an explicit `ReadOnlyDocumentVersionHistory`
result from an independently authorized read. The UI checks correspondence with
`requested=false` before enabling the reference selector; it checks again
**inside the deliberate Compare click handler with `requested=true`**.
Merely invoking a disabled DOM button handler cannot bypass this check.

No corresponding version history, wrong file instance, missing/changed source,
unverified observation, unstable read or wrong SHA-256 disables the *optional
comparison only* and hides any prior advisory. In addition, the volatile
selection and review invalidation key now bind optional claimed
`sourceContentVersionId` and `sourceSha256Hex` values. A new version/hash
does not carry a previous opt-in reference selection forward—even when the
`extractionSnapshotId` is accidentally reused by a future host.

This is intentionally still an **unmounted, nonshipping and untrusted** UI
boundary. Version/hash correspondence is necessary metadata corroboration but
NOT an extractor receipt, current-byte verification, financial validation,
authorized native read, audit trail or professional approval. No production
structured extraction provider exists for indexed viewer files, and therefore
there is no safe live path from the current `App.tsx` viewer to this UI.
No new Tauri command, mutation, source rewrite, upload, migration, book
reconciliation, sign-off or permissions change was added. Both the
client-side gate and the new controlled props require a future independently
reviewed provider and live direct-Tauri security acceptance prior to mounting.

The existing `tests/document-match-advisory-flow.test.mjs` suite now exercises
missing/foreign/unverified/unstable version histories, direct simulated clicks
despite disabled controls, changed source availability, stale version claims,
and even a newly internally consistent version that must not inherit an old
book choice. These tests establish **frontend fail-closed behavior only**.


## Cached advisory finding integrity (DRAFT continuation, 2026-10-22)

The detached `reviewIsCurrent` guard in `src/document-match-advisory-flow.ts`
now checks more than a cached input key, selected reference and source/version
identity. **Only after an explicit Compare result already exists**, it
recomputes the deterministic pure advisory observation from the *current*
read-only snapshot, explicit selected reference and mapping rules. It requires
the cached report's outcome and **every** finding's rule ID, label, status,
field names, scalar values, reason and original order to match. Missing,
added, reordered, substituted or fabricated findings are hidden instead of
being presented as accounting consistency or discrepancies. This also
rejects a forged all-consistent result produced by changing only a cached
outcome or its finding statuses. Copying a genuinely equivalent report is
permitted because review state is not an identity credential.

The recomputation is a **display-integrity check for a prior deliberate
user action**, not background matching, source-byte verification or a new
authorization decision. No report is calculated or shown in the absence of a
review; the original document remains usable if an observation is rejected.
Neither the input key nor this equality check establishes extraction
provenance: a malicious provider could supply fabricated fields to both
calculations. Native read authorization, independently source-bound
extraction and direct-Tauri acceptance remain mandatory before mounting.
No `App.tsx` integration, new Tauri command, persistent annotation, original
source rewrite, re-upload requirement, ledger reconciliation or sign-off is
introduced by this increment. Regression coverage targets forged statuses,
values, labels, field identities, reason codes, reordered and added findings.


## Visit-local history-read epoch and explicit consent reset (DRAFT continuation, 2026-10-22)

The detached controlled UI now **requires** a `historyReadKey`: a fresh, nonempty,
nonpersistent, caller-generated token identifying the *current response lifecycle*
of an independently authorized document-history read. The future host is
responsible for changing the token on **every history reload, replacement,
loss, and recovery**, even when the returned rows are identical; it must
never reuse a token during one viewer visit. This is a presentation lifecycle
contract, not a cryptographic digest, permission decision or source provenance.

The optional reference selection and the locally cached advisory review now
carry this token. The selector and Compare handler refuse selections bound to
a different read epoch; an old report is hidden after a refresh, even if
`contentVersionId`, `fileInstanceId`, SHA-256, reference and field data
have not changed. A new explicit selection and a new Compare click are
required after an epoch change. Missing or blank epochs disable optional
comparison without affecting original file preview/open or other document
operations. A host that fails to rotate its token is **not protected**
against stale consent revival; an untrusted caller can fabricate any token.
The token must not be interpreted as trusted identity, a grant, an extractor
receipt, a time-based expiry or native read authorization.

For backward compatibility, pure lower-level advisory helpers accept an
optional history-read key; the **detached control** is stricter and always
supplies/requires it. Lower-level helpers can still operate in an isolated
unit-test context and must not be mounted or exposed directly as an
authorized production interface. The existing metadata correlation checks
remain necessary and insufficient; extraction provenance, live direct-Tauri
authorization tests, DEC-01–08 and human acceptance remain open. New
frontend regressions cover same-metadata refresh, stale direct-click
attempts, absent epochs, recovery and renewed explicit user consent. No
Tauri command, new native permission, storage mutation, original document
edit, re-upload, accounting book operation, or reconciliation is added.


## Visit-bound acknowledgement and dismissal (DRAFT continuation, 2026-10-22)

The advisory engine's original `LocalAdvisoryDisposition` intentionally contains
only document, extraction-snapshot, selected-reference and rule IDs. This is
**insufficient as a cache key across viewer visits or changed comparison data**:
the same identifiers can reappear after a source/version or book-field update.
An earlier acknowledgement must never make a new disagreement appear approved.

The still-unmounted `DocumentMatchOptInControl` now emits only a
`VolatileAdvisoryAnnotation` to its future visit-local
`onLocalAnnotation` callback. It is independently stamped from an
already-current, explicitly requested review and binds the annotation to
`visitKey`, `documentId`, `fileInstanceId`, `extractionSnapshotId`,
`referenceId`, `historyReadKey`, and the pure comparison `inputKey`,
as well as the non-consistent rule and `acknowledged`/`dismissed` choice.
The lower-level narrow `LocalAdvisoryDisposition` is still available for
panel-only isolated tests and never represents a professional decision.
The narrow annotation validator now rejects invalid disposition strings.

A future host may retain a stamped annotation **in volatile visit memory
only**, and MUST use `volatileAnnotationIsCurrent` to filter before showing
one. That guard first revalidates the selected source, reference, extraction
snapshot, read epoch, rules, and every report finding; then it checks the
annotation's exact IDs and comparison input. Changes to history epoch, file
instance, document visit, selected book, version/hash claims, mapped fields or
rules invalidate the annotation. A consistent or absent finding cannot be
annotated. Clearing a comparison or closing a viewer must discard annotations.
The bounded UI still has no annotation display and no state persistence.

**Security boundary:** These values are local invalidation hints. They are
not unforgeable, an authentication secret, a signer or reviewer credential, an
audit record, extraction/source-byte provenance, professional approval, or
native permission. A malicious caller controlling the snapshot and client
can fabricate both the review and annotation. Any future durable review or
sign-off requires its *own* separately authorized native transaction and
independent policy review. No original document or accounting book is
revised, replaced, or required to be re-uploaded. No new Tauri command,
storage operation, live `App.tsx` mount, mandatory reconciliation,
evidence capture or sign-off path is introduced.

Frontend regression tests exercise exact-visit delivery, changed values
with reused snapshot IDs, history refresh, wrong-instance and wrong-reference
rejections, forged disposition labels, consistent-field suppression and
absence of callback-driven writes. Prior DEC-01–08, Issues #129/#138,
direct-Tauri authorization, live extractor provenance and independent human
acceptance remain unresolved.


## Event-time defensive rechecks after render (DRAFT continuation, 2026-10-22)

An already-rendered select or annotation handler can be invoked after a future
host has replaced or mutated a supplied history/reference object in place,
without a new React render or a correctly rotated `historyReadKey`.
The controlled, **unmounted** UI therefore no longer relies solely on the
render-time source-version gate at these event boundaries:

- Explicit selection performs a fresh, read-only
  `correlateOptionalExtractionWithDocumentHistory` check and verifies that
  exactly one current reference still has the selected ID. A stale selection
  event clears the local review and returns no selected reference.
- A local acknowledgement/dismissal callback independently repeats the same
  history-correlation check, then checks the full current advisory review and
  exact volatile annotation scope before contacting the future visit-local
  host. Malformed annotation events are dropped without an exception from the
  detached control. Errors thrown by a future host callback are not mistaken
  for source-document validation failures.
- The explicit Compare handler already repeats the source/version correlation
  with `requested=true` before producing a report. These changes do not
  trigger an automatic comparison, storage read, native permission or audit.

New synthetic tests intentionally mutate the same history object *after* UI
render but *before* select/annotation callbacks and inject an ambiguous
reference list or invalid annotation. They verify no annotation/reference
selection is forwarded. This is defense in depth against accidental client
state mutation; it cannot enforce a trustworthy history-token lifecycle or
verify the original bytes, authenticated extraction or native authorization.
All direct-Tauri, provenance, DEC-01–08, Issues #129/#138 and independent
security acceptance hold points remain outstanding. Original files and books
remain available and unchanged irrespective of matching status.


## Exact extraction-field opt-in invalidation (DRAFT continuation, 2026-10-23)

The detached explicit reference selection previously bound only the selected
document/file instance, extraction snapshot ID, source-version/hash claims and
viewer/history-read epochs. A future extraction provider could accidentally
reuse those identifiers while replacing `snapshot.fields`. A cached report
would be hidden by the comparison input key, but the old reference selection
could still authorize a new comparison **without a renewed user choice**.

`chooseExplicitAdvisoryReference` now captures a deterministic
`extractionFieldsKey` from sorted own field names, value types and scalar
values. `selectionIsCurrent` requires exact equality with the present
snapshot's field key; changing extracted values, types, field availability or
membership invalidates optional reference consent, even if the snapshot ID,
claimed source version/hash and `historyReadKey` remain unchanged.
Reordering fields without changing their values is not a change. Missing or
malformed field objects, nested values and throwing field accessors make
selection unavailable instead of enabling a Compare action. The user can
choose a reference again after valid changes; incomplete fields still produce
nonblocking `incomplete` observations after that explicit choice.

The key is **ordinary local UI serialization**, not a cryptographic hash,
a source receipt, a trusted timestamp, a revocation token, an extractor
signature, native authorization or proof of accounting correctness. The
host remains responsible for rotating the history-read epoch as previously
documented. This new check cannot secure a malicious provider that
forges all data consistently. No automatic reference association,
source-file rewrite, audit voucher mutation, required re-upload, native
command, durable annotation or App mount has been introduced. Human
DEC-01–08, Issues #129/#138, L1–L4, source-bound extractor provenance,
direct-Tauri permission/denial tests, and independent acceptance remain
outstanding. Added regression coverage for reused snapshot IDs with changed
fields, canonical reordering, type drift, malformed fields and renewed
explicit opt-in.


## Throwing or revoked read-only metadata input (DRAFT continuation, 2026-10-23)

The detached `correlateOptionalExtractionWithDocumentHistory` gate previously
caught malformed comparison field errors **after** history correlation, but
file/extraction/history preflight read operations could still throw if a future
provider or integration handed it an object with a throwing getter, revoked
Proxy or corrupted history row. Such a failure would be unacceptable as an
original-document viewer error because matching is optional.

The full metadata preflight and correlation now returns a frozen,
`status="unavailable"` / `reason="comparison-input-invalid"`,
nonblocking result if a metadata getter, history filter or proxy operation
throws. Valid source-version, file-instance and SHA-256 correspondence
continues to be checked as before, and an existing valid optional comparison
is not performed without an explicit request. The detached React control's
snapshot-status rendering separately catches thrown identity getters;
direct selected-reference and annotation event paths use the same
nonthrowing pure metadata gate. Tests inject throwing file availability,
document and snapshot identity, version/hash, history request/row fields,
revoked proxies, and metadata that becomes invalid **after UI render**.

A caught exception is **not** proof of malware, authenticity of the other
fields, revocation of a native permission or valid source bytes. The returned
unavailable state disables only the detached optional comparison, never
forces source editing or re-upload, never invalidates the indexed document
and never modifies accounting books. No production extraction provider,
Tauri command, filesystem access, persistence, formal review/sign-off or
`App.tsx` mount is introduced. Future trusted provider design and native
authorization/denial tests remain prerequisites under Issues #129/#138,
DEC-01–08 and the previously recorded acceptance hold points.


## Untrusted reference option list / stale event safety (DRAFT, 2026-10-23)

The detached control previously read `referenceId` properties repeatedly
during render, explicit selection and Compare, and pure review helpers
filtered reference lists outside their exception boundary. A read-only
future host that supplied an invalid reference, throwing getter, revoked
proxy or a list modified after UI render could therefore throw into the
document viewer or leave stale optional consent usable.

The still-UNMOUNTED `DocumentMatchOptInControl` now derives explicit
reference options through a **read-only, whole-list fail-closed**
`distinctReferenceOptions` check. Empty, non-string or duplicate IDs,
malformed entries and throwing/revoked getters yield no selectable options;
there is **no inferred candidate or automatic reference association**.
Option IDs are read and normalized only for safe display; React still
escapes them. Select and Compare recheck the current list at event time,
not only the old render snapshot, and refuse a now-ambiguous, missing or
unreadable option. A broken event clears only volatile opt-in, not the
original indexed document. The pure selection/review/annotation guards
and explicit optional comparison path now also catch throwing host
metadata, returning a local unavailable status or false instead of
propagating a viewer exception.

The defensive checks neither authenticate the reference source nor prove
it is a ledger record, prevent a malicious host from forging consistent
input, establish extraction provenance, authorize a native file-history
read, or constitute professional approval. The control remains detached
from `App.tsx`; no Tauri API, file write, stored annotation, ledger
reconciliation, mandatory matching, source modification or re-upload
requirement was added. Frontend synthetic negative tests cover broken
getters, revoked arrays/proxies, duplicate references added after render,
stale selection/Compare clicks, pure comparison/annotation denial, and
malformed event objects. The independent direct-Tauri and product/security
decision gates remain open (DEC-01–08, Issues #129/#138, L1–L4).


## Explicit reference-field revision consent (DRAFT continuation, 2026-10-23)

The optional reference **ID is not an immutable book snapshot**. Previously, an
explicit selection was bound to the extracted voucher fields, source-version
metadata and visit/history epochs, but could be reused after the *selected
reference's* fields changed under its existing ID. The old review would be
hidden; nevertheless a subsequent Compare click could silently use the
replacement book values without renewed reference selection.

The pure `chooseExplicitAdvisoryReference` now requires an explicitly
provided reference object whose ID matches the chosen ID. It binds
`referenceFieldsKey`, a deterministic serialization of sorted field
names, types and values, alongside the already required
`extractionFieldsKey`. The lower-level explicit comparison and cached
review guards reject changed/missing/malformed reference fields as stale;
the **unmounted** controlled UI also hides stale selections and rejects
a stale Compare event *before* attempting a document/reference observation.
Field-name order alone does not expire a choice; added/removed fields or
changes in type/value do. The selector passes the freshly chosen reference
object, and the Compare handler rechecks the current reference's fields,
including changes occurring after render. An unavailable or malformed
reference still never prevents previewing or using the original document.

This introduces an intentional type-level change to the **detached**
`chooseExplicitAdvisoryReference` helper: callers must now provide the
chosen reference object to obtain non-null opt-in; merely giving a
`referenceId` fails closed. No live production caller was added.
Both field keys are ordinary forgeable **UI invalidation strings**, NOT
hashes, signed book receipts, verifiable provenance, accounting correctness,
permission grants or auditor acceptance. A malicious provider could
fabricate an apparently self-consistent reference and extraction. No
automated matching, accounting ledger writes, original voucher edits,
new Tauri commands, persistent annotations, re-upload requirement or
`App.tsx` mount is introduced. Live native read authorization and
denial testing, source-bound extractor provenance, DEC-01–08,
Issues #129/#138, legacy L1–L4 and independent acceptance remain open.

Regression tests cover same-ID changed values before/after render,
type and field membership changes, canonical order, malformed book
fields, forged/missing bindings, and the need for renewed explicit choice.
