/**
 * Detached, controlled presentation of explicitly requested matching.
 * Not mounted in App, not an extraction UI, and not accounting reconciliation.
 * The host holds only visit-local selection/review state; no storage callbacks.
 */
import { createElement as h } from "react";
import type { ChangeEvent, ReactElement } from "react";
import { DocumentMatchAdvisoryPanel } from "./document-match-advisory-panel.ts";
import {
  chooseExplicitAdvisoryReference,
  stampVolatileAdvisoryAnnotation,
  volatileAnnotationIsCurrent,
  requestExplicitOptionalMatch,
  reviewIsCurrent,
  selectionIsCurrent,
} from "./document-match-advisory-flow.ts";
import type {
  ExplicitAdvisorySelection,
  OptionalMatchAttempt,
  VolatileAdvisoryReview,
  VolatileAdvisoryAnnotation,
} from "./document-match-advisory-flow.ts";
import type { IndexedDocumentIdentity } from "./document-match-advisory-adapter.ts";
import { correlateOptionalExtractionWithDocumentHistory } from "./document-match-source-version-gate.ts";
import type {
  ReadOnlyDocumentVersionHistory,
  VersionClaimedExtractionSnapshot,
  VersionCorrelationResult,
} from "./document-match-source-version-gate.ts";
import type {
  AdvisoryComparisonRule,
  LocalAdvisoryDisposition,
  OptionalReference,
} from "./document-match-advisory.ts";

export type DocumentMatchOptInControlProps = Readonly<{
  /** A fresh, local, nonpersistent token for each viewer opening. */
  visitKey: string;
  indexedFile: IndexedDocumentIdentity & Readonly<{availabilityState: string}>;
  snapshot: VersionClaimedExtractionSnapshot | null;
  /** Result of an independently authorized read-only history request. */
  history: ReadOnlyDocumentVersionHistory | null;
  /**
   * Fresh nonpersistent token for EVERY history read/reload/loss, never reused
   * within a viewer visit. This is NOT evidence provenance or authorization.
   */
  historyReadKey: string;
  /** Host-supplied read-only options only; NO automatic searching for matches. */
  references: readonly OptionalReference[];
  rules: readonly AdvisoryComparisonRule[];
  selection: ExplicitAdvisorySelection | null;
  review: VolatileAdvisoryReview | null;
  onSelectionChange: (selection: ExplicitAdvisorySelection | null) => void;
  onReview: (review: VolatileAdvisoryReview) => void;
  onClearReview: () => void;
  /** Optional visit-local issue notice only, never a document error. */
  onUnavailable?: (reason:
    | Extract<OptionalMatchAttempt, {status: "unavailable"}>["reason"]
    | Extract<VersionCorrelationResult, {status: "unavailable"}>["reason"]
    | "history-epoch-unavailable"
  ) => void;
  /**
   * Visit-local, exact-review annotation. Caller must revalidate before
   * reusing it; it is NOT an audit event or a professional approval.
   */
  onLocalAnnotation?: (annotation: VolatileAdvisoryAnnotation) => void;
}>;

/**
 * Each option is a user-visible reference ID, not an accounting match.
 * Invalid/duplicate/read-error lists fail closed AS A WHOLE so one bad entry
 * cannot escape validation, crash render, or revive a previous selection.
 * Do not open, sort by similarity, mutate or select any reference here.
 */
function distinctReferenceOptions(references: readonly OptionalReference[]): readonly Readonly<{
  id: string;
  reference: OptionalReference;
}>[] {
  try {
    if (!Array.isArray(references)) return [];
    const ids = new Set<string>();
    return references.map(reference => {
      if (!reference || typeof reference !== "object" || Array.isArray(reference)) {
        throw new TypeError("Malformed optional reference");
      }
      const id = reference.referenceId;
      if (typeof id !== "string" || id.trim().length === 0 || ids.has(id)) {
        throw new TypeError("Invalid or ambiguous optional reference ID");
      }
      ids.add(id);
      return { id, reference };
    });
  } catch {
    return [];
  }
}

/**
 * A document can always be opened, previewed and managed independently of this
 * component. A distinct, explicit button is the only matching trigger.
 */
export function DocumentMatchOptInControl(props: DocumentMatchOptInControlProps): ReactElement {
  const {
    visitKey, indexedFile, snapshot, history, historyReadKey, references, rules,
    selection, review, onSelectionChange, onReview, onClearReview,
    onUnavailable, onLocalAnnotation,
  } = props;

  const uniqueRefs = distinctReferenceOptions(references);
  // requested=false never compares document/reference fields on render.
  // Metadata agreement is not extraction provenance or authorization.
  const versionCheck = correlateOptionalExtractionWithDocumentHistory(
    indexedFile, snapshot, history, null, [], false,
  );
  // The host MUST rotate this on each history refresh or loss. Reusing it
  // is a host contract violation; no client epoch can prove native freshness.
  const epochReady = typeof historyReadKey === "string" && historyReadKey.trim().length > 0;
  const versionReady = versionCheck.status === "correlated" && epochReady;
  // Controlled props may arrive from an eventually trusted provider. These
  // checks are presentation safety only; malformed client state must not
  // interrupt the original document viewer.
  let validSelection = false;
  let currentReview: VolatileAdvisoryReview | null = null;
  let selectedId = "";
  try {
    validSelection = versionReady &&
      selectionIsCurrent(selection, visitKey, indexedFile, snapshot, historyReadKey) &&
      uniqueRefs.some(option => option.id === selection!.referenceId);
    selectedId = validSelection ? selection!.referenceId : "";
    if (validSelection && reviewIsCurrent(
      review, visitKey, indexedFile, snapshot, references, rules, selection, historyReadKey,
    )) currentReview = review;
  } catch {
    validSelection = false;
    currentReview = null;
    selectedId = "";
  }

  // Render-time correlation only controls what is shown. An independently
  // supplied history/reference object may be replaced or mutated before an
  // already-rendered DOM handler runs. Recheck at every action boundary.
  function historyStillCurrent(): boolean {
    return epochReady && correlateOptionalExtractionWithDocumentHistory(
      indexedFile, snapshot, history, null, [], false,
    ).status === "correlated";
  }

  function selectReference(event: ChangeEvent<HTMLSelectElement>): void {
    let nextSelection: ExplicitAdvisorySelection | null = null;
    try {
      const nextId = event.currentTarget.value;
      const freshOptions = distinctReferenceOptions(references);
      if (typeof nextId === "string" &&
          uniqueRefs.some(option => option.id === nextId) &&
          freshOptions.some(option => option.id === nextId) &&
          historyStillCurrent()) {
        nextSelection = chooseExplicitAdvisoryReference(
          visitKey, indexedFile, snapshot, nextId, historyReadKey,
        );
      }
    } catch {
      // Malformed/revoked event, reference or snapshot input clears opt-in.
    }
    onClearReview();
    onSelectionChange(nextSelection);
  }

  function compare(): void {
    // Recheck on the click path: a disabled UI control is not an authority gate.
    if (!validSelection) {
      onUnavailable?.(versionReady ? "no-explicit-selection"
        : !epochReady ? "history-epoch-unavailable"
          : versionCheck.reason!);
      return;
    }
    // The option list and selection may have changed since render without a
    // React state update. Recheck at the event boundary, not just disabled UI.
    let selectedReference: OptionalReference | null = null;
    try {
      const latestOptions = distinctReferenceOptions(references);
      if (historyStillCurrent() && selectionIsCurrent(
        selection, visitKey, indexedFile, snapshot, historyReadKey,
      )) {
        selectedReference = latestOptions.find(
          option => option.id === selection!.referenceId,
        )?.reference ?? null;
      }
    } catch {
      selectedReference = null;
    }
    if (!selectedReference) {
      onUnavailable?.("reference-unavailable");
      return;
    }
    const checked = correlateOptionalExtractionWithDocumentHistory(
      indexedFile, snapshot, history, selectedReference, rules, true,
    );
    if (checked.status !== "correlated" || checked.preparation.status !== "ready") {
      onUnavailable?.(checked.status === "unavailable" ? checked.reason : "extraction-unavailable");
      return;
    }
    const attempted = requestExplicitOptionalMatch(
      visitKey, indexedFile, snapshot, references, rules, selection, historyReadKey,
    );
    if (attempted.status === "ready") {
      onReview(attempted.review);
    } else {
      onUnavailable?.(attempted.reason);
    }
  }

  // The gate handles malformed read-only DTOs without throwing. Keep its
  // detached presentation equally resilient if a future provider supplies
  // a throwing identity getter or revoked proxy.
  let snapshotAvailable = false;
  try {
    snapshotAvailable = snapshot !== null &&
      snapshot.documentId === indexedFile.documentId &&
      snapshot.fileInstanceId === indexedFile.fileInstanceId &&
      typeof snapshot.extractionSnapshotId === "string" &&
      snapshot.extractionSnapshotId.trim().length > 0;
  } catch {
    snapshotAvailable = false;
  }

  return h("section", {
    className: "document-match-opt-in",
    role: "region",
    "aria-label": "Optional document comparison",
  },
    h("h3", null, "Optional reference comparison"),
    h("p", null, "Your original document remains available whether or not you compare it."),
    h("p", null, "Comparison is only an advisory review. A matching version-history row does not establish extraction provenance, certify accounts or edit the source."),
    !snapshotAvailable
      ? h("p", { role: "status" },
          "No bound structured extraction snapshot is available for this document. You can continue to preview and manage it normally.")
      : null,
    snapshotAvailable && !versionReady
      ? h("p", { role: "status" },
          "Optional comparison unavailable: the visit-local history read is missing or source-version metadata is unverified or does not match this indexed file. Document access is unaffected.")
      : null,
    h("label", null,
      "Select a reference explicitly (optional)",
      h("select", {
        "aria-label": "Select optional reference",
        value: selectedId,
        disabled: !versionReady || uniqueRefs.length === 0,
        onChange: selectReference,
      },
        h("option", { value: "" }, "No reference selected"),
        ...uniqueRefs.map(option => h("option", {
          value: option.id,
          key: option.id,
        }, option.id)),
      ),
    ),
    h("button", {
      type: "button",
      disabled: !versionReady || !validSelection,
      onClick: compare,
    }, "Compare selected reference"),
    currentReview
      ? h("div", { className: "document-match-volatile-review" },
          h("button", { type: "button", onClick: onClearReview }, "Clear local comparison"),
          h(DocumentMatchAdvisoryPanel, {
            observation: currentReview.observation,
            onLocalAnnotation: onLocalAnnotation
              ? (annotation: LocalAdvisoryDisposition) => {
                  // A stale mounted handler, a mutated history row, or a
                  // malformed panel event must not emit a new annotation.
                  // Only input errors are caught; host callback errors are
                  // intentionally not reclassified as document failures.
                  if (!historyStillCurrent()) return;
                  let stamped: VolatileAdvisoryAnnotation;
                  try {
                    stamped = stampVolatileAdvisoryAnnotation(currentReview, annotation);
                    if (!volatileAnnotationIsCurrent(
                      stamped, currentReview, visitKey, indexedFile, snapshot,
                      references, rules, selection, historyReadKey,
                    )) return;
                  } catch {
                    return;
                  }
                  onLocalAnnotation(stamped);
                }
              : undefined,
          }),
        )
      : null,
    h("p", null, "Changing the document, viewer visit, history-read token, extraction claim, source-version correspondence, reference or rules hides stale comparisons."),
  );
}
