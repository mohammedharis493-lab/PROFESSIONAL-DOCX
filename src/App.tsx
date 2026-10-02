import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type ApprovedStorageRoot = {
  storageRootId: string;
  displayPath: string;
  availabilityState: string;
};

type FileEntry = {
  name: string;
  path: string;
  extension: string;
  sizeBytes: number;
  modifiedUnixMs: number | null;
};

type FolderScan = {
  storageRootId: string;
  rootDisplayPath: string;
  totalFiles: number;
  totalBytes: number;
  skippedEntries: number;
  previewFiles: FileEntry[];
};

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

export default function App() {
  const [query, setQuery] = useState("");
  const [approvedRoot, setApprovedRoot] = useState<ApprovedStorageRoot | null>(null);
  const [roots, setRoots] = useState<ApprovedStorageRoot[]>([]);
  const [scan, setScan] = useState<FolderScan | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [isLoadingRoots, setIsLoadingRoots] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void refreshRoots();
  }, []);

  const visibleFiles = useMemo(() => {
    if (!scan) return [];
    const normalizedQuery = query.trim().toLocaleLowerCase();

    if (!normalizedQuery) {
      return scan.previewFiles.slice(0, 50);
    }

    return scan.previewFiles
      .filter((file) => file.name.toLocaleLowerCase().includes(normalizedQuery))
      .slice(0, 50);
  }, [query, scan]);

  async function refreshRoots() {
    try {
      const storedRoots = await invoke<ApprovedStorageRoot[]>("list_storage_roots");
      setRoots(storedRoots);
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setIsLoadingRoots(false);
    }
  }

  async function scanRoot(root: ApprovedStorageRoot) {
    setError(null);
    setIsScanning(true);
    setApprovedRoot(root);

    try {
      const result = await invoke<FolderScan>("scan_storage_root", {
        storageRootId: root.storageRootId,
      });

      setScan(result);
      setQuery("");
      await refreshRoots();
    } catch (scanError) {
      setScan(null);
      setError(String(scanError));
      await refreshRoots();
    } finally {
      setIsScanning(false);
    }
  }

  async function chooseFolder() {
    setError(null);
    setIsScanning(true);

    try {
      const root = await invoke<ApprovedStorageRoot | null>(
        "choose_and_register_storage_root",
      );

      if (!root) {
        return;
      }

      const storedRoots = await invoke<ApprovedStorageRoot[]>("list_storage_roots");
      setRoots(storedRoots);
      setApprovedRoot(root);

      const result = await invoke<FolderScan>("scan_storage_root", {
        storageRootId: root.storageRootId,
      });

      setScan(result);
      setQuery("");
    } catch (scanError) {
      setError(String(scanError));
    } finally {
      setIsScanning(false);
    }
  }

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="brand">
          <div className="brand-mark">PD</div>
          <div>
            <strong>Professional DocX</strong>
            <span>Development foundation</span>
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
            Filter scanned filenames
          </label>
          <div className="search-wrap">
            <span aria-hidden="true">⌕</span>
            <input
              id="universal-search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search scanned filenames..."
              autoComplete="off"
              disabled={!scan}
            />
            <kbd>Ctrl K</kbd>
          </div>
          <button
            className="primary-button"
            type="button"
            onClick={chooseFolder}
            disabled={isScanning}
          >
            {isScanning ? "Scanning…" : "Choose folder"}
          </button>
        </header>

        <section className="hero">
          <p className="eyebrow">PERSISTED STORAGE ROOTS</p>
          <h1>Approved folders now survive restart.</h1>
          <p className="hero-copy">
            Professional DocX stores approved roots in its local SQLite database.
            The UI receives only opaque root IDs; Rust reconstructs the native
            filesystem path from authoritative database records when a scan starts.
          </p>

          {error ? (
            <div className="status-card status-error" role="alert">
              {error}
            </div>
          ) : scan ? (
            <div className="scan-summary" aria-live="polite">
              <div>
                <span>Approved folder</span>
                <strong>{scan.rootDisplayPath}</strong>
              </div>
              <div>
                <span>Files found</span>
                <strong>{scan.totalFiles.toLocaleString()}</strong>
              </div>
              <div>
                <span>Source size</span>
                <strong>{formatBytes(scan.totalBytes)}</strong>
              </div>
              <div>
                <span>Skipped</span>
                <strong>{scan.skippedEntries.toLocaleString()}</strong>
              </div>
            </div>
          ) : approvedRoot ? (
            <div className="status-card">
              Approved root selected: {approvedRoot.displayPath}
            </div>
          ) : (
            <div className="status-card">
              {isLoadingRoots
                ? "Loading approved storage roots…"
                : roots.length
                  ? `${roots.length} approved storage root${roots.length === 1 ? "" : "s"} restored from SQLite.`
                  : "No approved storage root has been registered yet."}
            </div>
          )}
        </section>

        {roots.length ? (
          <section className="roots-panel" aria-label="Approved storage roots">
            <div className="results-heading">
              <div>
                <p className="eyebrow">APPROVED ROOTS</p>
                <h2>Stored in Professional DocX</h2>
              </div>
              <span>These records are restored after application restart.</span>
            </div>

            <div className="root-list">
              {roots.map((root) => (
                <div className="root-row" key={root.storageRootId}>
                  <div className="root-main">
                    <strong>{root.displayPath}</strong>
                    <span>{root.availabilityState}</span>
                  </div>
                  <button
                    className="secondary-button"
                    type="button"
                    onClick={() => void scanRoot(root)}
                    disabled={isScanning}
                  >
                    Scan
                  </button>
                </div>
              ))}
            </div>
          </section>
        ) : null}

        {scan ? (
          <section className="results-panel" aria-label="Scanned file preview">
            <div className="results-heading">
              <div>
                <p className="eyebrow">FILE PREVIEW</p>
                <h2>
                  {query
                    ? `${visibleFiles.length} preview match${visibleFiles.length === 1 ? "" : "es"}`
                    : "First 50 files"}
                </h2>
              </div>
              <span>
                Source files remain at their original approved location.
              </span>
            </div>

            <div className="file-list">
              {visibleFiles.length ? (
                visibleFiles.map((file) => (
                  <div className="file-row" key={file.path}>
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
                  No preview filename contains “{query}”.
                </div>
              )}
            </div>

            {scan.totalFiles > scan.previewFiles.length ? (
              <p className="preview-note">
                This screen intentionally shows only the first{" "}
                {scan.previewFiles.length.toLocaleString()} files from the scan.
                Full indexing is added with the background indexing-job step.
              </p>
            ) : null}
          </section>
        ) : (
          <section className="quick-grid" aria-label="Foundation principles">
            <article>
              <span>01</span>
              <h2>Persistent approval</h2>
              <p>Approved roots are restored from SQLite after restart.</p>
            </article>
            <article>
              <span>02</span>
              <h2>ID-based access</h2>
              <p>The UI still cannot grant access using arbitrary path strings.</p>
            </article>
            <article>
              <span>03</span>
              <h2>Native path fidelity</h2>
              <p>Rust stores native path bytes separately from display text.</p>
            </article>
          </section>
        )}
      </main>
    </div>
  );
}
