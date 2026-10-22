/**
 * Temporary UX containment for the already callable legacy three-field form.
 * Do not use this as authorization, an atomic stale-base check, evidence custody,
 * or a production Save boundary. Native legacy commands remain unchanged.
 */
export const LEGACY_REVISION_SUCCESSOR_BLOCKED =
  "The legacy three-field form cannot append to a workpaper that already has a revision. " +
  "Its successor path does not preserve seven-field edits and exact evidence links. " +
  "A separately reviewed corrective Save is not enabled.";

export function assertLegacyInitialRevisionOnly(revisions: readonly unknown[]): void {
  if (!Array.isArray(revisions) || revisions.length !== 0) {
    throw new Error(LEGACY_REVISION_SUCCESSOR_BLOCKED);
  }
}
