/**
 * Preview-only, controlled seven-field revision editor.
 *
 * Not mounted by App.tsx. Never invokes Tauri, SQLite, a permission service,
 * evidence custody or sign-off. Its parent owns the immutable session and is
 * responsible for preserving unsaved drafts. Showing fields and collecting
 * changes does not grant authority to create a successor revision.
 */
import { useId } from "react";
import {
  WORKPAPER_REVISION_CONTENT_FIELDS,
  editWorkpaperRevisionSessionField,
  restoreWorkpaperRevisionSessionField,
  inspectWorkpaperRevisionEditorFreshness,
  previewWorkpaperRevisionEditorSession,
} from "./workpaper-revision-edit-draft";
import { inspectWorkpaperRevisionObservedText } from "./workpaper-revision-conflict-inspection";
import type {
  WorkpaperRevisionContentField,
  WorkpaperRevisionEditBase,
  WorkpaperRevisionEditorSession,
} from "./workpaper-revision-edit-draft";

const FIELD_LABELS: Readonly<Record<WorkpaperRevisionContentField, string>> = {
  objective: "Objective",
  procedurePerformed: "Procedure performed",
  population: "Population",
  sample: "Sample",
  exceptions: "Exceptions",
  managementExplanation: "Management explanation",
  conclusion: "Conclusion",
};

export type WorkpaperRevisionDraftPanelProps = Readonly<{
  session: WorkpaperRevisionEditorSession;
  onSessionChange: (next: WorkpaperRevisionEditorSession) => void;
  /**
   * Optional client-side observation; never authoritative for concurrency.
   * Missing means the UI cannot report freshness, not that the base is current.
   */
  observedLatestRevisionId?: string | null;
  /**
   * Optional complete text-only observation for explaining overlapping edits.
   * It never supplies authority or evidence completeness and cannot rebase.
   */
  observedLatestRevision?: WorkpaperRevisionEditBase;
  /**
   * Display-only hint from historical records, not an authorization flag
   * or proof of sign-off validity for the current exact revision.
   */
  hasHistoricalSignoff?: boolean;
}>;

export default function WorkpaperRevisionDraftPanel({
  session,
  onSessionChange,
  observedLatestRevisionId,
  observedLatestRevision,
  hasHistoricalSignoff = false,
}: WorkpaperRevisionDraftPanelProps) {
  const prefix = useId();
  if (
    observedLatestRevision &&
    observedLatestRevisionId != null &&
    observedLatestRevision.workpaperRevisionId !== observedLatestRevisionId
  ) {
    throw new TypeError("Conflicting latest revision observations must be reconciled by the caller");
  }
  const comparedText = observedLatestRevision
    ? inspectWorkpaperRevisionObservedText(session, observedLatestRevision)
    : null;
  const latestId = observedLatestRevision?.workpaperRevisionId ?? observedLatestRevisionId;
  const preview = previewWorkpaperRevisionEditorSession(session);
  const changed = new Set<string>(preview.changedFields);
  const freshness =
    latestId === undefined || latestId === null
      ? null
      : inspectWorkpaperRevisionEditorFreshness(session, {
          workpaperId: session.base.workpaperId,
          workpaperRevisionId: latestId,
        });

  return (
    <section className="workspace-card workspace-form" aria-label="Local workpaper revision draft">
      <div className="workspace-detail-heading">
        <div>
          <span className="workspace-label">SEVEN-FIELD EDITOR PREVIEW</span>
          <h3>Draft based on revision {session.base.workpaperRevisionId}</h3>
        </div>
      </div>
      <p className="evidence-integrity-note">
        Draft only. Editing text here does not save a workpaper, acquire revision
        permission, change evidence links or create professional approval.
      </p>
      {hasHistoricalSignoff ? (
        <p className="evidence-integrity-note">
          A historical sign-off remains linked to its exact previous revision.
          This draft does not inherit that sign-off.
        </p>
      ) : null}
      {freshness === null ? (
        <p className="evidence-integrity-note">
          Latest-revision status has not been checked. Saving would require an
          independent trusted backend check.
        </p>
      ) : freshness.state === "base-stale" ? (
        <p className="evidence-integrity-note" role="alert">
          Another revision is now visible ({freshness.observedLatestRevisionId}).
          Your unsaved text remains in this draft. Do not overwrite or
          automatically rebase it.
        </p>
      ) : (
        <p className="evidence-integrity-note">
          The client currently sees the same base revision. This is not
          permission to save and does not replace backend conflict checking.
        </p>
      )}
      {comparedText && comparedText.differentRevisionObserved ? (
        <div className="evidence-integrity-note" aria-label="Observed revision text comparison">
          <p>
            Client-only comparison against the observed revision. These field
            indicators do not compare evidence or approval, and cannot rebase
            or authorize a Save. Review the full revision before deciding how
            to continue; your draft and its original base are unchanged.
          </p>
          <ul>
            {comparedText.fields.filter((entry) => entry.status !== "unchanged").map((entry) => (
              <li key={entry.field}>
                {FIELD_LABELS[entry.field]}: {
                  entry.status === "local-only" ? "Edited in this draft only"
                    : entry.status === "observed-only" ? "Changed in observed revision only"
                    : entry.status === "converged-text" ? "Same text entered in both revisions"
                    : "Different edits in both revisions — manual review needed"
                }
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      <div className="workspace-grid workspace-grid-two">
        {WORKPAPER_REVISION_CONTENT_FIELDS.map((field) => (
          <div key={field}>
            <label htmlFor={`${prefix}-${field}`}>
              <span>
                {FIELD_LABELS[field]}
                {changed.has(field) ? " · Edited" : ""}
              </span>
              <textarea
                id={`${prefix}-${field}`}
                name={field}
                rows={4}
                value={session.draft.content[field]}
                onChange={(event) => {
                  onSessionChange(
                    editWorkpaperRevisionSessionField(session, field, event.currentTarget.value),
                  );
                }}
              />
            </label>
            {changed.has(field) ? (
              <button
                type="button"
                className="secondary-button"
                aria-label={`Restore original ${FIELD_LABELS[field]} text`}
                onClick={() => {
                  onSessionChange(restoreWorkpaperRevisionSessionField(session, field));
                }}
              >
                Restore original text
              </button>
            ) : null}
          </div>
        ))}
      </div>
      <p role="status">
        {preview.kind === "content-unchanged"
          ? "Canonical text unchanged. Evidence and approval state are not compared."
          : `${preview.changedFields.length} canonical text field(s) edited. Evidence and approval state are not compared.`}
      </p>
      <p className="evidence-integrity-note">
        Restore original text affects only that field in this local draft,
        using the original opened revision rather than any newer revision.
      </p>
      <p className="evidence-integrity-note">
        No Save or sign-off action is connected. A later implementation must
        verify authenticated workpaper-scoped revision rights and exact
        revision/evidence state inside the backend write transaction.
      </p>
    </section>
  );
}
