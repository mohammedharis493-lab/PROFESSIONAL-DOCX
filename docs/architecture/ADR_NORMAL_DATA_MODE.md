# ADR — First-class Normal Data Mode and deterministic data workspaces

**Status:** Proposed  
**Date:** 2026-10-09  
**Scope:** Architecture only; no runtime, schema, dependency, security, or permission changes in this PR.

## Context

Professional DocX already provides universal document access and specialist statutory audit, internal audit, compliance, reconciliation, and due-diligence workflows. Ordinary professional data work must not require an engagement, workpaper, audit sign-off, or controlled-evidence capture. Existing ledger/TB and reconciliation domains must not be recast as a generic spreadsheet engine.

## Decision

1. Introduce **Normal Data Mode** as a first-class, engagement-independent workspace profile on the common document/search/storage platform. Avoid a fixed schema enum of all present and future specialist modes. The initial normal-data workspace may optionally reference a client, but requires neither client nor engagement.
2. Implement the generic-data domain **alongside** existing engagement/workpaper tables. Do not rewrite specialist entities into a premature common superclass. Extract shared processing capabilities only where actual reuse is demonstrated.
3. Sources are selected by existing stable application IDs. Native Rust resolves approved storage roots and canonical paths; the frontend may not submit arbitrary filesystem paths. Linked sources stay linked by default; managed imports and immutable controlled evidence remain distinct.
4. Distinguish **dataset definition/source**, **exact dataset version/snapshot or verified fingerprint**, **column schema and semantic roles**, **immutable deterministic recipe version**, and **append-only run**. A run binds its recipe version, input identities and exact content-state evidence, parameters, output digest, counts, and execution time. Reopening history must not silently re-read changed linked bytes as though they were the original run.
5. Use explicit **period basis** and semantic date roles: invoice/document/transaction date, posting date, filing/return period, and accounting period are different concepts. Period grouping comes from a user-confirmed selection or validated import profile, never an inferred date merely because that field exists.
6. Keep the authoritative computation in deterministic backend code. AI may explain or suggest mappings, never silently alter matches, balances, classifications, or exceptions. Material assumptions remain inspectable.
7. A normal-data result is a working artifact, **not** formal controlled evidence. Future promotion to engagement/workpaper/evidence must be explicit and bind exact source/run versions, with the controlled-evidence workflow applied where policy requires.
8. Authorization claims remain limited to actual implemented enforcement. Actor strings and workflow status metadata must not be represented as authenticated RBAC.

## First implementation slices

- **Slice A — foundation:** Introduce minimal normal workspace, dataset reference, semantic-column mapping, recipe/version, and run-history schema/API with UUID validation, engagement independence, immutable historical records, and migration tests. No broad spreadsheet engine or dependencies.
- **Slice B — deterministic comparison:** Two source datasets, explicitly selected keys, period role, and numeric fields. Persist exact inputs and a deterministic comparison run; classify present-both, A-only, B-only, period-moved, numeric-difference, and duplicate cases. Keep filing period and invoice month separately visible.
- **Slice C — UI:** Normal Data navigation and workspace creation, source selection, column-role confirmation, comparison builder, result inspection, and exact historical run reopening.
- **Slice D — opt-in promotion:** Link exact normal-data run/source versions to specialist workflows after integrity and authorization design review.

## Acceptance and non-regression constraints

- No engagement, audit area, workpaper, reviewer, PBC request, or controlled-evidence capture is needed to create a normal-data workspace.
- A linked workbook is selected by indexed file identity without accepting a user-provided path; source change detection prevents misleading historical reruns.
- Given invoice date `2026-07-29` and filing period `2026-08`, a filing-period comparison assigns the record to **August**, not July. Same invoice key appearing under August in A and September in B is reportable as a **period movement**.
- Stable input versions and parameters yield stable outputs regardless of input row ordering; duplicate pairing/tolerance policy is explicit and tested.
- Historical recipes, source bindings, comparison results, and classifications are append-only or immutable; no AI or UI path can rewrite official deterministic results.
- Existing controlled-evidence hash verification, retained status, managed-store boundaries, canonical path checks, ledger/TB mapping invariants, and CI/Security remain unchanged.

## Consequences and deferred questions

The platform gains a general professional-data foundation without forcing specialist audit terminology. The immediate cost is a small parallel domain model and careful provenance semantics. Do not add a full spreadsheet language, universal connector system, or centralized multi-user RBAC in this ADR slice. Define concrete source-version retention/rehydration, resource limits, numeric scale/tolerance, and optional client linkage as narrow implementation decisions with tests before each backend PR.
