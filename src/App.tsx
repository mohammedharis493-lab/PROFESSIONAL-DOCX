import { useEffect, useMemo, useState } from "react";
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
};

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

export default function App() {
  const [query, setQuery] = useState("");
  const [roots, setRoots] = useState<ApprovedStorageRoot[]>([]);
  const [selectedRoot, setSelectedRoot] = useState<ApprovedStorageRoot | null>(null);
  const [latestJobs, setLatestJobs] = useState<Record<string, IndexJob | null>>({});
  const [activeJob, setActiveJob] = useState<IndexJob | null>(null);
  const [previewFiles, setPreviewFiles] = useState<IndexedFile[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isStarting, setIsStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void refreshRoots();
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

  const visibleFiles = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();

    if (!normalizedQuery) {
      return previewFiles.slice(0, 100);
    }

    return previewFiles
      .filter((file) => file.name.toLocaleLowerCase().includes(normalizedQuery))
      .slice(0, 100);
  }, [previewFiles, query]);

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

  async function loadPreview(root: ApprovedStorageRoot) {
    const files = await invoke<IndexedFile[]>("list_indexed_file_preview", {
      storageRootId: root.storageRootId,
      limit: 200,
    });
    setPreviewFiles(files);
    setQuery("");
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

  const activeIsRunning =
    activeJob !== null && !TERMINAL_JOB_STATUSES.has(activeJob.status);

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="brand">
          <div className="brand-mark">PD</div>
          <div>
            <strong>Professional DocX</strong>
            <span>Indexing foundation</span>
          </div>
        </div>

        <nav className="nav-list">
          <button className="nav-item nav-item-active" type="button">
            Home
          </button>
          <button className="nav-item" type="button" disabled>
            Clients
          </button>
          <button className="nav-item" type="button" disabled>
            Engagements
          </button>
          <button className="nav-item" type="button" disabled>
            Recent
          </button>
          <button className="nav-item" type="button" disabled>
            Pinned
          </button>
        </nav>
      </aside>

      <main className="main-content">
        <header className="topbar">
          <label className="search-label" htmlFor="universal-search">
            Filter indexed filename preview
          </label>
          <div className="search-wrap">
            <span aria-hidden="true">⌕</span>
            <input
              id="universal-search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Filter indexed filename preview..."
              autoComplete="off"
              disabled={!previewFiles.length}
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
          <p className="eyebrow">BACKGROUND INDEXING</p>
          <h1>Large folder scans no longer block the application.</h1>
          <p className="hero-copy">
            Each approved root is indexed as a persistent job and scan generation.
            File observations are written to SQLite in bounded batches while the UI
            reads progress from durable job state.
          </p>

          {error ? (
            <div className="status-card status-error" role="alert">
              {error}
            </div>
          ) : activeJob ? (
            <div className="job-card" aria-live="polite">
              <div className="job-card-heading">
                <div>
                  <span>INDEX JOB</span>
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
                  ? `${roots.length} approved storage root${roots.length === 1 ? "" : "s"} ready.`
                  : "Add a folder to create the first persistent background index job."}
            </div>
          )}
        </section>

        {roots.length ? (
          <section className="roots-panel" aria-label="Approved storage roots">
            <div className="results-heading">
              <div>
                <p className="eyebrow">APPROVED ROOTS</p>
                <h2>Index sources</h2>
              </div>
              <span>Only one filesystem index job runs at a time in this foundation build.</span>
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
                      disabled={isStarting || activeIsRunning || completed || running}
                    >
                      {completed ? "Indexed" : running ? "Indexing…" : "Index"}
                    </button>
                  </div>
                );
              })}
            </div>
          </section>
        ) : null}

        {selectedRoot ? (
          <section className="results-panel" aria-label="Indexed file preview">
            <div className="results-heading">
              <div>
                <p className="eyebrow">SQLITE FILE PREVIEW</p>
                <h2>
                  {query
                    ? `${visibleFiles.length} preview match${visibleFiles.length === 1 ? "" : "es"}`
                    : previewFiles.length
                      ? `First ${previewFiles.length} indexed files`
                      : "No persisted files yet"}
                </h2>
              </div>
              <span>
                This is a SQLite preview. Tantivy-backed universal search comes after
                the indexing/reconciliation foundation.
              </span>
            </div>

            <div className="file-list">
              {visibleFiles.length ? (
                visibleFiles.map((file) => (
                  <div className="file-row" key={file.fileInstanceId}>
                    <div className="file-icon" aria-hidden="true">
                      {file.extension ? file.extension.slice(0, 4).toUpperCase() : "FILE"}
                    </div>
                    <div className="file-main">
                      <strong>{file.name}</strong>
                      <span>{file.path}</span>
                    </div>
                    <span className="file-size">{formatBytes(file.sizeBytes)}</span>
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
        ) : (
          <section className="quick-grid" aria-label="Indexing principles">
            <article>
              <span>01</span>
              <h2>Persistent jobs</h2>
              <p>Job and generation status survive application crashes.</p>
            </article>
            <article>
              <span>02</span>
              <h2>Bounded batches</h2>
              <p>File observations are committed in batches instead of one giant scan.</p>
            </article>
            <article>
              <span>03</span>
              <h2>Safe cancellation</h2>
              <p>Cancelled or interrupted scans never become authoritative generations.</p>
            </article>
          </section>
        )}
      </main>
    </div>
  );
}
