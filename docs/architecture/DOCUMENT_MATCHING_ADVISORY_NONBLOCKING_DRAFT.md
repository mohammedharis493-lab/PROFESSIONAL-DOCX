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
