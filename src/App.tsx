import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type ApprovedStorageRoot = {
  storageRootId: string;
  displayPath: string;
  availabilityState: string;
};

type IndexJob = {
  indexJobId: string;
  storageRootId: string;
  jobType: string;
  status: string;
  requestedAtMs: number;
  startedAtMs: number | null;
  completedAtMs: number | null;
  cancelRequestedAtMs: number | null;
  lastHeartbeatAtMs: number | null;
  currentPhase: string | null;
  directoriesSeen: number;
  filesSeen: number;
  bytesSeen: number;
  filesPersisted: number;
  errorsCount: number;
  scanGenerationId: string | null;
  failureCode: string | null;
  failureMessage: string | null;
};

type IndexedFile = {
  documentId: string;
  fileInstanceId: string;
  name: string;
  path: string;
  extension: string;
  sizeBytes: number;
  modifiedUnixMs: number | null;
  availabilityState: string;
};

type SearchResult = IndexedFile & {
  matchedField: string;
  score: number;
};

type RecentDocument = IndexedFile & {
  lastOpenedAtMs: number;
  openCount: number;
};

type PinnedDocument = IndexedFile & {
  pinnedAtMs: number;
};

type ViewMode = "home" | "recent" | "pinned";

const TERMINAL_JOB_STATUSES = new Set([
  "COMPLETE",
  "PARTIAL",
  "CANCELLED",
  "OFFLINE",
  "FAILED",
  "INTERRUPTED",
]);

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = units[0];

  for (let index = 1; index < units.length && value >= 1024; index += 1) {
    value /= 1024;
    unit = units[index];
  }

  return `${value.toFixed(value >= 10 ? 1 : 2)} ${unit}`;
}

function jobLabel(job: IndexJob | null | undefined) {
  if (!job) return "Not indexed";
  return job.status.replaceAll("_", " ");
}

function stateClass(availabilityState: string) {
  return `file-state file-state-${availabilityState.toLowerCase()}`;
}

export default function App() {
  const [query, setQuery] = useState("");
  const [roots, setRoots] = useState<ApprovedStorageRoot[]>([]);
  const [selectedRoot, setSelectedRoot] = useState<ApprovedStorageRoot | null>(null);
  const [latestJobs, setLatestJobs] = useState<Record<string, IndexJob | null>>({});
  const [activeJob, setActiveJob] = useState<IndexJob | null>(null);
  const [previewFiles, setPreviewFiles] = useState<IndexedFile[]>([]);
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const [recentDocuments, setRecentDocuments] = useState<RecentDocument[]>([]);
  const [pinnedDocuments, setPinnedDocuments] = useState<PinnedDocument[]>([]);
  const [viewMode, setViewMode] = useState<ViewMode>("home");
  const [selectedSearchIndex, setSelectedSearchIndex] = useState(0);
  const [isLoading, setIsLoading] = useState(true);
  const [isStarting, setIsStarting] = useState(false);
  const [isSearching, setIsSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchError, setSearchError] = useState<string | null>(null);
  const searchSequence = useRef(0);

  useEffect(() => {
    void refreshRoots();
    void refreshQuickAccess();
  }, []);

  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        const input = document.getElementById("universal-search") as HTMLInputElement | null;
        input?.focus();
        input?.select();
      }
    };

    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, []);

  useEffect(() => {
    if (!activeJob || TERMINAL_JOB_STATUSES.has(activeJob.status)) {
      return;
    }

    const jobId = activeJob.indexJobId;
    const timer = window.setInterval(() => {
      void refreshJob(jobId);
    }, 700);

    return () => window.clearInterval(timer);
  }, [activeJob?.indexJobId, activeJob?.status]);

  useEffect(() => {
    const trimmedQuery = query.trim();
    const sequence = searchSequence.current + 1;
    searchSequence.current = sequence;

    if (!trimmedQuery) {
      setSearchResults([]);
      setSelectedSearchIndex(0);
      setSearchError(null);
      setIsSearching(false);
      return;
    }

    setIsSearching(true);
    setSearchError(null);

    const timer = window.setTimeout(() => {
      void invoke<SearchResult[]>("search_documents", {
        query: trimmedQuery,
        limit: 50,
      })
        .then((results) => {
          if (searchSequence.current !== sequence) return;
          setSearchResults(results);
          setSelectedSearchIndex(0);
          setSearchError(null);
        })
        .catch((searchFailure) => {
          if (searchSequence.current !== sequence) return;
          setSearchResults([]);
          setSelectedSearchIndex(0);
          setSearchError(String(searchFailure));
        })
        .finally(() => {
          if (searchSequence.current === sequence) {
            setIsSearching(false);
          }
        });
    }, 120);

    return () => window.clearTimeout(timer);
  }, [query]);

  async function refreshRoots() {
    try {
      const storedRoots = await invoke<ApprovedStorageRoot[]>("list_storage_roots");
      setRoots(storedRoots);

      const jobPairs = await Promise.all(
        storedRoots.map(async (root) => {
          const job = await invoke<IndexJob | null>("get_latest_index_job_for_root", {
            storageRootId: root.storageRootId,
          });
          return [root.storageRootId, job] as const;
        }),
      );

      setLatestJobs(Object.fromEntries(jobPairs));
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setIsLoading(false);
    }
  }

  async function refreshQuickAccess() {
    try {
      const [recents, pins] = await Promise.all([
        invoke<RecentDocument[]>("list_recent_documents", { limit: 20 }),
        invoke<PinnedDocument[]>("list_pinned_documents", { limit: 50 }),
      ]);
      setRecentDocuments(recents);
      setPinnedDocuments(pins);
    } catch (quickAccessError) {
      setError(String(quickAccessError));
    }
  }

  async function loadPreview(root: ApprovedStorageRoot) {
    const files = await invoke<IndexedFile[]>("list_indexed_file_preview", {
      storageRootId: root.storageRootId,
      limit: 200,
    });
    setPreviewFiles(files);
  }

  async function selectRoot(root: ApprovedStorageRoot) {
    setError(null);
    setSelectedRoot(root);

    try {
      const latest = await invoke<IndexJob | null>("get_latest_index_job_for_root", {
        storageRootId: root.storageRootId,
      });
      setActiveJob(latest);
      await loadPreview(root);
    } catch (selectionError) {
      setError(String(selectionError));
    }
  }

  async function refreshJob(indexJobId: string) {
    try {
      const job = await invoke<IndexJob | null>("get_index_job", { indexJobId });
      if (!job) return;

      setActiveJob(job);
      setLatestJobs((current) => ({
        ...current,
        [job.storageRootId]: job,
      }));

      if (TERMINAL_JOB_STATUSES.has(job.status)) {
        await refreshRoots();
        const root = roots.find((item) => item.storageRootId === job.storageRootId);
        if (root) {
          await loadPreview(root);
        }
      }
    } catch (jobError) {
      setError(String(jobError));
    }
  }

  async function startIndex(root: ApprovedStorageRoot) {
    setError(null);
    setSelectedRoot(root);
    setPreviewFiles([]);
    setIsStarting(true);

    try {
      const job = await invoke<IndexJob>("start_index_job", {
        storageRootId: root.storageRootId,
      });
      setActiveJob(job);
      setLatestJobs((current) => ({
        ...current,
        [root.storageRootId]: job,
      }));
    } catch (startError) {
      setError(String(startError));
      await refreshRoots();
    } finally {
      setIsStarting(false);
    }
  }

  async function chooseFolder() {
    setError(null);
    setIsStarting(true);

    try {
      const root = await invoke<ApprovedStorageRoot | null>(
        "choose_and_register_storage_root",
      );

      if (!root) return;

      await refreshRoots();
      await startIndex(root);
    } catch (chooseError) {
      setError(String(chooseError));
    } finally {
      setIsStarting(false);
    }
  }

  async function cancelActiveJob() {
    if (!activeJob) return;

    try {
      const job = await invoke<IndexJob | null>("cancel_index_job", {
        indexJobId: activeJob.indexJobId,
      });
      if (job) setActiveJob(job);
    } catch (cancelError) {
      setError(String(cancelError));
    }
  }

  async function openFileInstance(fileInstanceId: string) {
    setError(null);
    try {
      await invoke("open_file_instance", { fileInstanceId });
      await refreshQuickAccess();
    } catch (openError) {
      setError(String(openError));
    }
  }

  async function revealFileInstance(fileInstanceId: string) {
    setError(null);
    try {
      await invoke("reveal_file_instance", { fileInstanceId });
    } catch (revealError) {
      setError(String(revealError));
    }
  }

  async function toggleDocumentPin(documentId: string, pinned: boolean) {
    setError(null);
    try {
      await invoke("set_document_pin", { documentId, pinned });
      await refreshQuickAccess();
    } catch (pinError) {
      setError(String(pinError));
    }
  }

  function showView(mode: ViewMode) {
    setQuery("");
    setSelectedRoot(null);
    setViewMode(mode);
  }

  function handleSearchKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      setQuery("");
      setSelectedSearchIndex(0);
      return;
    }

    if (!searchResults.length) return;

    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelectedSearchIndex((current) =>
        Math.min(current + 1, searchResults.length - 1),
      );
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelectedSearchIndex((current) => Math.max(current - 1, 0));
      return;
    }

    if (event.key === "Enter") {
      const selected = searchResults[selectedSearchIndex];
      if (!selected || selected.availabilityState === "MISSING") return;
      event.preventDefault();
      void openFileInstance(selected.fileInstanceId);
    }
  }

  const activeIsRunning =
    activeJob !== null && !TERMINAL_JOB_STATUSES.has(activeJob.status);
  const hasQuery = query.trim().length > 0;
  const pinnedDocumentIds = new Set(
    pinnedDocuments.map((document) => document.documentId),
  );

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="brand">
          <div className="brand-mark">PD</div>
          <div>
            <strong>Professional DocX</strong>
            <span>Search-first workspace</span>
          </div>
        </div>

        <nav className="nav-list">
          <button
            className={`nav-item${viewMode === "home" && !selectedRoot ? " nav-item-active" : ""}`}
            type="button"
            onClick={() => showView("home")}
          >
            Home
          </button>
          <button className="nav-item" type="button" disabled>
            Clients
          </button>
          <button className="nav-item" type="button" disabled>
            Engagements
          </button>
          <button
            className={`nav-item${viewMode === "recent" ? " nav-item-active" : ""}`}
            type="button"
            onClick={() => showView("recent")}
          >
            Recent <span className="nav-count">{recentDocuments.length}</span>
          </button>
          <button
            className={`nav-item${viewMode === "pinned" ? " nav-item-active" : ""}`}
            type="button"
            onClick={() => showView("pinned")}
          >
            Pinned <span className="nav-count">{pinnedDocuments.length}</span>
          </button>
        </nav>
      </aside>

      <main className="main-content">
        <header className="topbar">
          <label className="search-label" htmlFor="universal-search">
            Universal search
          </label>
          <div className="search-wrap">
            <span aria-hidden="true">⌕</span>
            <input
              id="universal-search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={handleSearchKeyDown}
              placeholder="Search anything..."
              autoComplete="off"
              aria-controls="universal-search-results"
              aria-activedescendant={
                searchResults.length ? `search-result-${selectedSearchIndex}` : undefined
              }
              aria-busy={isSearching}
            />
            <kbd>Ctrl K</kbd>
          </div>
          <button
            className="primary-button"
            type="button"
            onClick={chooseFolder}
            disabled={isStarting || activeIsRunning}
          >
            {isStarting ? "Starting…" : "Add folder"}
          </button>
        </header>

        <section className="hero">
          <p className="eyebrow">UNIVERSAL SEARCH</p>
          <h1>Find the document without remembering where it lives.</h1>
          <p className="hero-copy">
            Search existing indexed files by filename or path across approved roots.
            Partial words, prefixes, common typing errors, and year separators are
            handled while the original file stays in its existing location.
          </p>

          {error ? (
            <div className="status-card status-error" role="alert">
              {error}
            </div>
          ) : activeJob ? (
            <div className="job-card" aria-live="polite">
              <div className="job-card-heading">
                <div>
                  <span>{activeJob.jobType.replaceAll("_", " ")}</span>
                  <strong>{activeJob.status}</strong>
                </div>
                {activeIsRunning ? (
                  <button
                    className="secondary-button"
                    type="button"
                    onClick={() => void cancelActiveJob()}
                    disabled={activeJob.cancelRequestedAtMs !== null}
                  >
                    {activeJob.cancelRequestedAtMs ? "Cancelling…" : "Cancel"}
                  </button>
                ) : null}
              </div>
              <div className="job-metrics">
                <div>
                  <span>Phase</span>
                  <strong>{activeJob.currentPhase ?? "—"}</strong>
                </div>
                <div>
                  <span>Files seen</span>
                  <strong>{activeJob.filesSeen.toLocaleString()}</strong>
                </div>
                <div>
                  <span>Persisted</span>
                  <strong>{activeJob.filesPersisted.toLocaleString()}</strong>
                </div>
                <div>
                  <span>Source size</span>
                  <strong>{formatBytes(activeJob.bytesSeen)}</strong>
                </div>
                <div>
                  <span>Errors</span>
                  <strong>{activeJob.errorsCount.toLocaleString()}</strong>
                </div>
              </div>
              {activeJob.failureMessage ? (
                <p className="job-message">{activeJob.failureMessage}</p>
              ) : null}
            </div>
          ) : (
            <div className="status-card">
              {isLoading
                ? "Loading approved storage roots…"
                : roots.length
                  ? `${roots.length} approved storage root${roots.length === 1 ? "" : "s"} ready for search.`
                  : "Add a folder to create the first searchable source."}
            </div>
          )}
        </section>

        {hasQuery ? (
          <section
            className="results-panel universal-results"
            id="universal-search-results"
            aria-label="Universal search results"
            aria-live="polite"
          >
            <div className="results-heading">
              <div>
                <p className="eyebrow">SEARCH RESULTS</p>
                <h2>
                  {isSearching
                    ? "Searching…"
                    : `${searchResults.length} result${searchResults.length === 1 ? "" : "s"}`}
                </h2>
              </div>
              <span>
                ↑↓ selects · Enter opens · Esc clears. Filename matches rank ahead of
                path and fuzzy-only matches.
              </span>
            </div>

            {searchError ? (
              <div className="empty-result status-error" role="alert">
                Search failed: {searchError}
              </div>
            ) : searchResults.length ? (
              <div className="file-list">
                {searchResults.map((file, index) => (
                  <div
                    className={`file-row search-result-row${index === selectedSearchIndex ? " file-row-selected" : ""}`}
                    id={`search-result-${index}`}
                    key={file.fileInstanceId}
                    onMouseEnter={() => setSelectedSearchIndex(index)}
                    onDoubleClick={() => {
                      if (file.availabilityState !== "MISSING") {
                        void openFileInstance(file.fileInstanceId);
                      }
                    }}
                  >
                    <div className="file-icon" aria-hidden="true">
                      {file.extension ? file.extension.slice(0, 4).toUpperCase() : "FILE"}
                    </div>
                    <div className="file-main">
                      <strong>{file.name}</strong>
                      <span>{file.path}</span>
                      <span className="match-source">Matched: {file.matchedField}</span>
                    </div>
                    <div className="file-meta">
                      <span className={stateClass(file.availabilityState)}>
                        {file.availabilityState}
                      </span>
                      <span className="file-size">{formatBytes(file.sizeBytes)}</span>
                      <button
                        className="file-action"
                        type="button"
                        onClick={() => void openFileInstance(file.fileInstanceId)}
                        disabled={file.availabilityState === "MISSING"}
                      >
                        Open
                      </button>
                      <button
                        className="file-action"
                        type="button"
                        onClick={() => void revealFileInstance(file.fileInstanceId)}
                        disabled={file.availabilityState === "MISSING"}
                      >
                        Location
                      </button>
                      <button
                        className={`file-action${pinnedDocumentIds.has(file.documentId) ? " file-action-pinned" : ""}`}
                        type="button"
                        aria-pressed={pinnedDocumentIds.has(file.documentId)}
                        onClick={() =>
                          void toggleDocumentPin(
                            file.documentId,
                            !pinnedDocumentIds.has(file.documentId),
                          )
                        }
                      >
                        {pinnedDocumentIds.has(file.documentId) ? "Unpin" : "Pin"}
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="empty-result">
                {isSearching
                  ? "Searching indexed metadata…"
                  : "No indexed document matched this search."}
              </div>
            )}
          </section>
        ) : null}

        {!hasQuery && !selectedRoot && viewMode === "recent" ? (
          <section className="results-panel quick-access-panel" aria-label="Recent documents">
            <div className="results-heading">
              <div>
                <p className="eyebrow">RECENT</p>
                <h2>Recently opened documents</h2>
              </div>
              <span>Successful opens appear here automatically, newest first.</span>
            </div>
            <div className="file-list">
              {recentDocuments.length ? (
                recentDocuments.map((file) => (
                  <div className="file-row" key={file.fileInstanceId}>
                    <div className="file-icon" aria-hidden="true">
                      {file.extension ? file.extension.slice(0, 4).toUpperCase() : "FILE"}
                    </div>
                    <div className="file-main">
                      <strong>{file.name}</strong>
                      <span>{file.path}</span>
                      <span className="match-source">
                        Opened {file.openCount} time{file.openCount === 1 ? "" : "s"}
                      </span>
                    </div>
                    <div className="file-meta">
                      <span className={stateClass(file.availabilityState)}>
                        {file.availabilityState}
                      </span>
                      <button
                        className="file-action"
                        type="button"
                        onClick={() => void openFileInstance(file.fileInstanceId)}
                        disabled={file.availabilityState === "MISSING"}
                      >
                        Open
                      </button>
                      <button
                        className={`file-action${pinnedDocumentIds.has(file.documentId) ? " file-action-pinned" : ""}`}
                        type="button"
                        aria-pressed={pinnedDocumentIds.has(file.documentId)}
                        onClick={() =>
                          void toggleDocumentPin(
                            file.documentId,
                            !pinnedDocumentIds.has(file.documentId),
                          )
                        }
                      >
                        {pinnedDocumentIds.has(file.documentId) ? "Unpin" : "Pin"}
                      </button>
                    </div>
                  </div>
                ))
              ) : (
                <div className="empty-result">No documents have been opened yet.</div>
              )}
            </div>
          </section>
        ) : null}

        {!hasQuery && !selectedRoot && viewMode === "pinned" ? (
          <section className="results-panel quick-access-panel" aria-label="Pinned documents">
            <div className="results-heading">
              <div>
                <p className="eyebrow">PINNED</p>
                <h2>Pinned documents</h2>
              </div>
              <span>Pins follow document identity across legitimate renames and moves.</span>
            </div>
            <div className="file-list">
              {pinnedDocuments.length ? (
                pinnedDocuments.map((file) => (
                  <div className="file-row" key={file.fileInstanceId}>
                    <div className="file-icon" aria-hidden="true">
                      {file.extension ? file.extension.slice(0, 4).toUpperCase() : "FILE"}
                    </div>
                    <div className="file-main">
                      <strong>{file.name}</strong>
                      <span>{file.path}</span>
                    </div>
                    <div className="file-meta">
                      <span className={stateClass(file.availabilityState)}>
                        {file.availabilityState}
                      </span>
                      <button
                        className="file-action"
                        type="button"
                        onClick={() => void openFileInstance(file.fileInstanceId)}
                        disabled={file.availabilityState === "MISSING"}
                      >
                        Open
                      </button>
                      <button
                        className="file-action file-action-pinned"
                        type="button"
                        aria-pressed="true"
                        onClick={() => void toggleDocumentPin(file.documentId, false)}
                      >
                        Unpin
                      </button>
                    </div>
                  </div>
                ))
              ) : (
                <div className="empty-result">
                  Pin a search result to keep important documents one click away.
                </div>
              )}
            </div>
          </section>
        ) : null}

        {roots.length ? (
          <section className="roots-panel" aria-label="Approved storage roots">
            <div className="results-heading">
              <div>
                <p className="eyebrow">APPROVED ROOTS</p>
                <h2>Search sources</h2>
              </div>
              <span>
                Existing files stay in place. Reconcile a completed root to refresh
                moves, edits, additions, and missing-file status.
              </span>
            </div>

            <div className="root-list">
              {roots.map((root) => {
                const latest = latestJobs[root.storageRootId];
                const completed = latest?.status === "COMPLETE";
                const running =
                  latest !== null &&
                  latest !== undefined &&
                  !TERMINAL_JOB_STATUSES.has(latest.status);

                return (
                  <div className="root-row" key={root.storageRootId}>
                    <button
                      className="root-select"
                      type="button"
                      onClick={() => void selectRoot(root)}
                    >
                      <strong>{root.displayPath}</strong>
                      <span>
                        {root.availabilityState} · {jobLabel(latest)}
                      </span>
                    </button>
                    <button
                      className="secondary-button"
                      type="button"
                      onClick={() => void startIndex(root)}
                      disabled={isStarting || activeIsRunning || running}
                    >
                      {running ? "Indexing…" : completed ? "Reconcile" : "Index"}
                    </button>
                  </div>
                );
              })}
            </div>
          </section>
        ) : null}

        {!hasQuery && selectedRoot ? (
          <section className="results-panel" aria-label="Indexed file preview">
            <div className="results-heading">
              <div>
                <p className="eyebrow">INDEXED SOURCE</p>
                <h2>
                  {previewFiles.length
                    ? `First ${previewFiles.length} indexed files`
                    : "No persisted files yet"}
                </h2>
              </div>
              <span>
                This SQLite-backed source view is diagnostic. Use the universal search
                above for the normal document-access path.
              </span>
            </div>

            <div className="file-list">
              {previewFiles.length ? (
                previewFiles.map((file) => (
                  <div className="file-row" key={file.fileInstanceId}>
                    <div className="file-icon" aria-hidden="true">
                      {file.extension ? file.extension.slice(0, 4).toUpperCase() : "FILE"}
                    </div>
                    <div className="file-main">
                      <strong>{file.name}</strong>
                      <span>{file.path}</span>
                    </div>
                    <div className="file-meta">
                      <span className={stateClass(file.availabilityState)}>
                        {file.availabilityState}
                      </span>
                      <span className="file-size">{formatBytes(file.sizeBytes)}</span>
                      <button
                        className="file-action"
                        type="button"
                        onClick={() => void openFileInstance(file.fileInstanceId)}
                        disabled={file.availabilityState === "MISSING"}
                      >
                        Open
                      </button>
                      <button
                        className="file-action"
                        type="button"
                        onClick={() => void revealFileInstance(file.fileInstanceId)}
                        disabled={file.availabilityState === "MISSING"}
                      >
                        Location
                      </button>
                      <button
                        className={`file-action${pinnedDocumentIds.has(file.documentId) ? " file-action-pinned" : ""}`}
                        type="button"
                        aria-pressed={pinnedDocumentIds.has(file.documentId)}
                        onClick={() =>
                          void toggleDocumentPin(
                            file.documentId,
                            !pinnedDocumentIds.has(file.documentId),
                          )
                        }
                      >
                        {pinnedDocumentIds.has(file.documentId) ? "Unpin" : "Pin"}
                      </button>
                    </div>
                  </div>
                ))
              ) : (
                <div className="empty-result">
                  {activeIsRunning
                    ? "The background worker is persisting files in batches."
                    : "No indexed file preview is available for this root."}
                </div>
              )}
            </div>
          </section>
        ) : !hasQuery && !selectedRoot && viewMode === "home" ? (
          <section className="quick-grid" aria-label="Search capabilities">
            <article>
              <span>01</span>
              <h2>Recent documents</h2>
              <p>
                {recentDocuments.length
                  ? `${recentDocuments.length} recently opened document${recentDocuments.length === 1 ? "" : "s"} ready from the sidebar.`
                  : "Successfully opened documents will appear in Recent automatically."}
              </p>
            </article>
            <article>
              <span>02</span>
              <h2>Pinned documents</h2>
              <p>
                {pinnedDocuments.length
                  ? `${pinnedDocuments.length} pinned document${pinnedDocuments.length === 1 ? "" : "s"} available in one click.`
                  : "Pin important search results so they remain one click away."}
              </p>
            </article>
            <article>
              <span>03</span>
              <h2>Keyboard first</h2>
              <p>Ctrl+K → type → ↑↓ → Enter reaches and opens an indexed original.</p>
            </article>
          </section>
        ) : null}
      </main>
    </div>
  );
}
