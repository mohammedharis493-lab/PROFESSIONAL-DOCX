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
