import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

type FileEntry = {
  name: string;
  path: string;
  extension: string;
  sizeBytes: number;
  modifiedUnixMs: number | null;
};

type FolderScan = {
  root: string;
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
  const [scan, setScan] = useState<FolderScan | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  async function chooseFolder() {
    setError(null);

    const selected = await open({
      directory: true,
      multiple: false,
      recursive: true,
      title: "Choose a folder to scan",
    });

    if (!selected || Array.isArray(selected)) {
      return;
    }

    setIsScanning(true);

    try {
      const result = await invoke<FolderScan>("scan_folder", {
        root: selected,
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
          <p className="eyebrow">FIRST WORKING FEATURE</p>
          <h1>Scan existing files without copying them.</h1>
          <p className="hero-copy">
            Choose a folder already on your computer or network storage.
            Professional DocX reads file metadata in place; this step does not
            import or duplicate those files.
          </p>

          {error ? (
            <div className="status-card status-error" role="alert">
              {error}
            </div>
          ) : scan ? (
            <div className="scan-summary" aria-live="polite">
              <div>
                <span>Selected folder</span>
                <strong>{scan.root}</strong>
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
          ) : (
            <div className="status-card">
              No folder has been scanned. Your existing files remain untouched.
            </div>
          )}
        </section>

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
                Scan keeps source files at their original paths.
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
                Full indexing is added in the search phase.
              </p>
            ) : null}
          </section>
        ) : (
          <section className="quick-grid" aria-label="Foundation principles">
            <article>
              <span>01</span>
              <h2>Search first</h2>
              <p>Partial names such as “salamudd” must find the right file.</p>
            </article>
            <article>
              <span>02</span>
              <h2>Files stay in place</h2>
              <p>Existing folders are referenced and indexed, not duplicated.</p>
            </article>
            <article>
              <span>03</span>
              <h2>Evidence when needed</h2>
              <p>Immutable copies are created only for controlled evidence.</p>
            </article>
          </section>
        )}
      </main>
    </div>
  );
}
