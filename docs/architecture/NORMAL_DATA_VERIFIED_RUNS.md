# Normal Data — Verified CSV comparison runs (first adapter)

The native Tauri run command accepts only a comparison recipe-version UUID. It resolves both immutable dataset versions, verifies the source bytes against their stored SHA-256 values using existing approved-root native path validation, and applies the deterministic comparison core.

## Explicit source contract

- Both dataset versions must be stable and HASH_VERIFIED; metadata and fingerprints alone never authorize a run.
- UTF-8 CSV only (optional BOM, quoted values, LF or CRLF). Other formats are rejected.
- Every dataset must declare exactly one TEXT BUSINESS_KEY, the selected period column and every chosen DECIMAL NUMERIC_VALUE column.
- Filing and accounting periods are YYYY-MM; the invoice-month basis requires valid YYYY-MM-DD input dates and groups explicitly by invoice month.
- **All numeric CSV cells must already be signed integer minor units.** There is no automatic decimal or currency-scale inference, rounding, or conversion. Both sides must share the same normalized scale; e.g. 10025 means 10,025 minor units, not an assumed major-currency value.
- Business keys match exactly. Duplicate keys are surfaced, not arbitrarily paired.

Each read is limited to 32 MiB and each side to 100,000 rows. Result JSON has an 8 MiB cap. Invalid or tampered inputs fail before writing history.

## Persistence boundary

Successful executions atomically write the immutable recipe-version ID, both verified input SHA-256 values, the deterministic result digest, result JSON, timestamps and a matching audit event. History queries return stored results without silently re-reading changed linked files.

The result digest describes normalized comparison outputs and effective settings; input hashes independently identify the source bytes. Runs are ordinary working-data history, not controlled evidence. Source originals may subsequently change or disappear; a run does not imply retained original bytes.

## Historical run provenance inspection (Slice D1)

`inspect_normal_data_run_provenance` accepts an exact completed run UUID and returns
the frozen recipe version, workspace, dataset/content versions, the two **source**
SHA-256 hashes recorded during execution, and the separate **result** SHA-256
digest. It recomputes the digest from stored normalized result values and checks
the stored metadata against frozen recipe and dataset versions. It does not
read current linked files, export originals, create a controlled evidence version,
attach to a workpaper or confer any authorization.

Historical inspection remains available after a linked source changes or
disappears, and reports `currentSourceBytesChecked: false` plus
`isControlledEvidence: false` and `specialistPromotionAuthorized: false`.
This is an inspection receipt, not a source-retention or independent audit
attestation. See [ADR_NORMAL_DATA_PROMOTION_BOUNDARY.md](ADR_NORMAL_DATA_PROMOTION_BOUNDARY.md).

## Explicit live source recheck (D2A, not promotion)

`recheck_normal_data_run_sources` accepts only a completed comparison-run UUID.
It first validates the frozen run, then separately reads source A and B through
the existing native approved-root/stable-read verifier (32 MiB maximum each).
Each source must match its immutable content version **and** the exact SHA-256
recorded in that historical run. Success returns two separately timestamped
read observations; either source's change, missing file, source-version
mismatch, or failed verification returns an error and no success receipt.

The observations are sequential, not an atomic two-file snapshot. There is no
stored source copy, permission grant, workpaper link, evidence capture, or
promise that bytes will remain the same for a later operation. A future
capture must independently authorize and re-verify the originals. This
command does not modify the database, evidence store, or run history.

## Deferred features

XLSX import, configured decimal scales, controlled evidence promotion, paginated history and authenticated reviewer workflow remain separate slices. Workspace, recipe and bounded history UI exist; formal specialist attachment and retention do not.
