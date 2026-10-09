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

## Deferred features

XLSX import, configured decimal scales, UI, evidence promotion, paginated history and reviewer workflow remain separate slices.
