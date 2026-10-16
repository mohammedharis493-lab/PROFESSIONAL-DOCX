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
  inspectWorkpaperRevisionEditorFreshness,
  previewWorkpaperRevisionEditorSession,
} from "./workpaper-revision-edit-draft";
import type {
  WorkpaperRevisionContentField,
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
   * Display-only hint from historical records, not an authorization flag
   * or proof of sign-off validity for the current exact revision.
   */
  hasHistoricalSignoff?: boolean;
}>;

export default function WorkpaperRevisionDraftPanel({
  session,
  onSessionChange,
  observedLatestRevisionId,
  hasHistoricalSignoff = false,
}: WorkpaperRevisionDraftPanelProps) {
  const prefix = useId();
  const preview = previewWorkpaperRevisionEditorSession(session);
  const changed = new Set<string>(preview.changedFields);
  const freshness =
    observedLatestRevisionId === undefined || observedLatestRevisionId === null
      ? null
      : inspectWorkpaperRevisionEditorFreshness(session, {
          workpaperId: session.base.workpaperId,
          workpaperRevisionId: observedLatestRevisionId,
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
      <div className="workspace-grid workspace-grid-two">
        {WORKPAPER_REVISION_CONTENT_FIELDS.map((field) => (
          <label key={field} htmlFor={`${prefix}-${field}`}>
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
        ))}
      </div>
      <p role="status">
        {preview.kind === "content-unchanged"
          ? "Canonical text unchanged. Evidence and approval state are not compared."
          : `${preview.changedFields.length} canonical text field(s) edited. Evidence and approval state are not compared.`}
      </p>
      <p className="evidence-integrity-note">
        No Save or sign-off action is connected. A later implementation must
        verify authenticated workpaper-scoped revision rights and exact
        revision/evidence state inside the backend write transaction.
      </p>
    </section>
  );
}
