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
  keepLocalAdvisoryDisposition,
  requestExplicitOptionalMatch,
  reviewIsCurrent,
  selectionIsCurrent,
} from "./document-match-advisory-flow.ts";
import type {
  ExplicitAdvisorySelection,
  OptionalMatchAttempt,
  VolatileAdvisoryReview,
} from "./document-match-advisory-flow.ts";
import type {
  IndexedDocumentIdentity,
  ReadOnlyExtractionFieldSnapshot,
} from "./document-match-advisory-adapter.ts";
import type {
  AdvisoryComparisonRule,
  LocalAdvisoryDisposition,
  OptionalReference,
} from "./document-match-advisory.ts";

export type DocumentMatchOptInControlProps = Readonly<{
  /** A fresh, local, nonpersistent token for each viewer opening. */
  visitKey: string;
  indexedFile: IndexedDocumentIdentity;
  snapshot: ReadOnlyExtractionFieldSnapshot | null;
  /** Host-supplied read-only options only; NO automatic searching for matches. */
  references: readonly OptionalReference[];
  rules: readonly AdvisoryComparisonRule[];
  selection: ExplicitAdvisorySelection | null;
  review: VolatileAdvisoryReview | null;
  onSelectionChange: (selection: ExplicitAdvisorySelection | null) => void;
  onReview: (review: VolatileAdvisoryReview) => void;
  onClearReview: () => void;
  /** Optional visit-local issue notice only, never a document error. */
  onUnavailable?: (reason: Extract<OptionalMatchAttempt, {status: "unavailable"}>["reason"]) => void;
  onLocalAnnotation?: (annotation: LocalAdvisoryDisposition) => void;
}>;

/**
 * A document can always be opened, previewed and managed independently of this
 * component. A distinct, explicit button is the only matching trigger.
 */
export function DocumentMatchOptInControl(props: DocumentMatchOptInControlProps): ReactElement {
  const {
    visitKey, indexedFile, snapshot, references, rules,
    selection, review, onSelectionChange, onReview, onClearReview,
    onUnavailable, onLocalAnnotation,
  } = props;

  const uniqueRefs = Array.isArray(references) ? references.filter((ref, index) =>
    ref && typeof ref.referenceId === "string" &&
    ref.referenceId.trim().length > 0 &&
    references.findIndex(candidate => candidate?.referenceId === ref.referenceId) === index &&
    references.filter(candidate => candidate?.referenceId === ref.referenceId).length === 1,
  ) : [];
  const validSelection = selectionIsCurrent(selection, visitKey, indexedFile, snapshot) &&
    uniqueRefs.some(reference => reference.referenceId === selection!.referenceId);
  const selectedId = validSelection ? selection!.referenceId : "";
  const currentReview = validSelection && reviewIsCurrent(
    review, visitKey, indexedFile, snapshot, references, rules, selection,
  ) ? review : null;

  function selectReference(event: ChangeEvent<HTMLSelectElement>): void {
    const nextId = event.currentTarget.value;
    const nextSelection = nextId && uniqueRefs.some(reference => reference.referenceId === nextId)
      ? chooseExplicitAdvisoryReference(visitKey, indexedFile, snapshot, nextId)
      : null;
    onClearReview();
    onSelectionChange(nextSelection);
  }

  function compare(): void {
    const attempted = requestExplicitOptionalMatch(
      visitKey, indexedFile, snapshot, references, rules, validSelection ? selection : null,
    );
    if (attempted.status === "ready") {
      onReview(attempted.review);
    } else {
      onUnavailable?.(attempted.reason);
    }
  }

  const snapshotAvailable = snapshot !== null &&
    snapshot.documentId === indexedFile.documentId &&
    snapshot.fileInstanceId === indexedFile.fileInstanceId &&
    typeof snapshot.extractionSnapshotId === "string" &&
    snapshot.extractionSnapshotId.trim().length > 0;

  return h("section", {
    className: "document-match-opt-in",
    role: "region",
    "aria-label": "Optional document comparison",
  },
    h("h3", null, "Optional reference comparison"),
    h("p", null, "Your original document remains available whether or not you compare it."),
    h("p", null, "Comparison is only an advisory review. It does not verify accounts, approve workpapers, or edit the source."),
    !snapshotAvailable
      ? h("p", { role: "status" },
          "No bound structured extraction snapshot is available for this document. You can continue to preview and manage it normally.")
      : null,
    h("label", null,
      "Select a reference explicitly (optional)",
      h("select", {
        "aria-label": "Select optional reference",
        value: selectedId,
        disabled: !snapshotAvailable || uniqueRefs.length === 0,
        onChange: selectReference,
      },
        h("option", { value: "" }, "No reference selected"),
        ...uniqueRefs.map(reference => h("option", {
          value: reference.referenceId,
          key: reference.referenceId,
        }, reference.referenceId)),
      ),
    ),
    h("button", {
      type: "button",
      disabled: !snapshotAvailable || !validSelection,
      onClick: compare,
    }, "Compare selected reference"),
    currentReview
      ? h("div", { className: "document-match-volatile-review" },
          h("button", { type: "button", onClick: onClearReview }, "Clear local comparison"),
          h(DocumentMatchAdvisoryPanel, {
            observation: currentReview.observation,
            onLocalAnnotation: onLocalAnnotation
              ? (annotation: LocalAdvisoryDisposition) => onLocalAnnotation(
                  keepLocalAdvisoryDisposition(currentReview, annotation),
                )
              : undefined,
          }),
        )
      : null,
    h("p", null, "Changing the document, viewer visit, snapshot, reference or rules hides any stale comparison."),
  );
}
