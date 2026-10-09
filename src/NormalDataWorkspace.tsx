import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./normal-data.css";

type NormalDataWorkspaceRecord = {
  normalDataWorkspaceId: string;
  normalDataWorkspaceVersionId: string;
  name: string;
  description: string | null;
  periodStart: string | null;
  periodEnd: string | null;
  clientId: string | null;
  createdAtMs: number;
};

type NormalDataDatasetRecord = {
  normalDataDatasetId: string;
  normalDataWorkspaceId: string;
  normalDataDatasetVersionId: string;
  name: string;
  documentId: string;
  fileInstanceId: string;
  contentVersionId: string;
  sourceVerificationState: string;
  sourceSha256Hex: string | null;
  sourceFingerprintPresent: boolean;
  sourceSizeBytes: number;
  createdAtMs: number;
};

type IndexedCsv = {
  fileInstanceId: string;
  documentId: string;
  name: string;
  path: string;
  extension: string;
  availabilityState: string;
};

type NormalDataColumn = {
  normalDataColumnSemanticId: string;
  normalDataDatasetVersionId: string;
  columnName: string;
  semanticRole: string;
  dataType: string;
  createdAtMs: number;
};

const COLUMN_ROLES: { role: string; label: string; type: string }[] = [
  { role: "BUSINESS_KEY", label: "Business key", type: "TEXT" },
  { role: "FILING_PERIOD", label: "Filing period (YYYY-MM)", type: "PERIOD" },
  { role: "INVOICE_DATE", label: "Invoice date (YYYY-MM-DD)", type: "DATE" },
  { role: "ACCOUNTING_PERIOD", label: "Accounting period (YYYY-MM)", type: "PERIOD" },
  { role: "POSTING_DATE", label: "Posting date", type: "DATE" },
  { role: "TRANSACTION_DATE", label: "Transaction date", type: "DATE" },
  { role: "NUMERIC_VALUE", label: "Numeric amount", type: "DECIMAL" },
  { role: "OTHER", label: "Other text", type: "TEXT" },
];

function readableTimestamp(value: number) {
  return new Date(value).toLocaleDateString();
}

function verificationDescription(dataset: NormalDataDatasetRecord) {
  if (dataset.sourceVerificationState === "HASH_VERIFIED" && dataset.sourceSha256Hex) {
    return "Hash-verified indexed source · verification is rechecked at execution";
  }
  return "Not hash-verified · comparison execution requires a stable SHA-256 source";
}

export default function NormalDataWorkspace() {
  const [workspaces, setWorkspaces] = useState<NormalDataWorkspaceRecord[]>([]);
  const [workspaceId, setWorkspaceId] = useState("");
  const [datasets, setDatasets] = useState<NormalDataDatasetRecord[]>([]);
  const [datasetVersionId, setDatasetVersionId] = useState("");
  const [columns, setColumns] = useState<NormalDataColumn[]>([]);
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspaceDescription, setWorkspaceDescription] = useState("");
  const [periodStart, setPeriodStart] = useState("");
  const [periodEnd, setPeriodEnd] = useState("");
  const [sourceSearch, setSourceSearch] = useState("");
  const [sourceMatches, setSourceMatches] = useState<IndexedCsv[]>([]);
  const [datasetName, setDatasetName] = useState("");
  const [columnName, setColumnName] = useState("");
  const [columnRole, setColumnRole] = useState("BUSINESS_KEY");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [workspaceRevision, setWorkspaceRevision] = useState(0);
  const [datasetRevision, setDatasetRevision] = useState(0);
  const [columnRevision, setColumnRevision] = useState(0);
  const sourceSearchId = useRef(0);

  useEffect(() => {
    let live = true;
    setLoading(true);
    void invoke<NormalDataWorkspaceRecord[]>("list_normal_data_workspaces")
      .then((result) => {
        if (!live) return;
        setWorkspaces(result);
        setWorkspaceId((current) =>
          result.some((item) => item.normalDataWorkspaceId === current)
            ? current
            : (result[0]?.normalDataWorkspaceId ?? ""),
        );
      })
      .catch((cause: unknown) => { if (live) setError(String(cause)); })
      .finally(() => { if (live) setLoading(false); });
    return () => { live = false; };
  }, [workspaceRevision]);

  useEffect(() => {
    let live = true;
    setDatasets([]);
    setDatasetVersionId("");
    setColumns([]);
    setSourceMatches([]);
    setSourceSearch("");
    if (!workspaceId) return () => { live = false; };
    void invoke<NormalDataDatasetRecord[]>("list_normal_data_datasets", {
      normalDataWorkspaceId: workspaceId,
    }).then((result) => {
      if (!live) return;
      setDatasets(result);
      setDatasetVersionId(result[0]?.normalDataDatasetVersionId ?? "");
    }).catch((cause: unknown) => { if (live) setError(String(cause)); });
    return () => { live = false; };
  }, [workspaceId, datasetRevision]);

  useEffect(() => {
    let live = true;
    setColumns([]);
    if (!datasetVersionId) return () => { live = false; };
    void invoke<NormalDataColumn[]>("list_normal_data_column_semantics", {
      normalDataDatasetVersionId: datasetVersionId,
    }).then((result) => { if (live) setColumns(result); })
      .catch((cause: unknown) => { if (live) setError(String(cause)); });
    return () => { live = false; };
  }, [datasetVersionId, columnRevision]);

  const selectedWorkspace = workspaces.find(
    (item) => item.normalDataWorkspaceId === workspaceId,
  );
  const selectedDataset = datasets.find(
    (item) => item.normalDataDatasetVersionId === datasetVersionId,
  );
  const currentRole = COLUMN_ROLES.find((item) => item.role === columnRole) ?? COLUMN_ROLES[0];

  async function createWorkspace(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!workspaceName.trim() || busy) return;
    setBusy(true);
    setError("");
    setMessage("");
    try {
      const created = await invoke<NormalDataWorkspaceRecord>("create_normal_data_workspace", {
        name: workspaceName.trim(),
        description: workspaceDescription.trim() || null,
        clientId: null,
        periodStart: periodStart || null,
        periodEnd: periodEnd || null,
      });
      setWorkspaceName("");
      setWorkspaceDescription("");
      setPeriodStart("");
      setPeriodEnd("");
      setWorkspaceId(created.normalDataWorkspaceId);
      setWorkspaceRevision((value) => value + 1);
      setMessage("Normal Data workspace created. No client or engagement was required.");
    } catch (cause: unknown) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function findSources(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!sourceSearch.trim() || !workspaceId) return;
    const requestId = ++sourceSearchId.current;
    setBusy(true);
    setError("");
    try {
      const result = await invoke<IndexedCsv[]>("search_documents", {
        query: sourceSearch.trim(),
        limit: 50,
      });
      if (requestId === sourceSearchId.current) {
        setSourceMatches(result.filter((file) =>
          file.extension.replace(/^\./, "").toLowerCase() === "csv" &&
          file.availabilityState === "AVAILABLE",
        ));
      }
    } catch (cause: unknown) {
      if (requestId === sourceSearchId.current) setError(String(cause));
    } finally {
      if (requestId === sourceSearchId.current) setBusy(false);
    }
  }

  async function bindDataset(file: IndexedCsv) {
    if (!workspaceId || busy) return;
    setBusy(true);
    setError("");
    setMessage("");
    try {
      const created = await invoke<NormalDataDatasetRecord>("create_normal_data_dataset", {
        normalDataWorkspaceId: workspaceId,
        fileInstanceId: file.fileInstanceId,
        name: datasetName.trim() || file.name,
      });
      setSourceMatches([]);
      setDatasetName("");
      setDatasetRevision((value) => value + 1);
      setMessage("Dataset source bound to indexed file identity " + created.fileInstanceId +
        ". Column roles are declarations; no source headers have been verified by this step.");
    } catch (cause: unknown) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function declareColumn(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!datasetVersionId || !columnName.trim() || busy) return;
    setBusy(true);
    setError("");
    setMessage("");
    try {
      await invoke<NormalDataColumn>("declare_normal_data_column_semantic", {
        normalDataDatasetVersionId: datasetVersionId,
        columnName: columnName.trim(),
        semanticRole: currentRole.role,
        dataType: currentRole.type,
      });
      setColumnName("");
      setColumnRevision((value) => value + 1);
      setMessage("Column meaning declared. The executor validates actual CSV headers and values when a comparison runs.");
    } catch (cause: unknown) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="normal-data-panel" aria-label="Normal Data workspaces">
      <header className="normal-data-heading">
        <div>
          <p className="eyebrow">ENGAGEMENT-INDEPENDENT DATA</p>
          <h2>Normal Data workspaces</h2>
          <p>
            Work with indexed datasets directly. Client, audit engagement,
            workpaper and reviewer records are not prerequisites.
          </p>
        </div>
        <span className="normal-data-count">{workspaces.length} workspaces</span>
      </header>

      {error ? <p className="normal-data-error" role="alert">{error}</p> : null}
      {message ? <p className="normal-data-success" role="status">{message}</p> : null}

      <div className="normal-data-grid">
        <section className="normal-data-card" aria-label="Workspace selection">
          <h3>1. Workspace</h3>
          <label className="normal-data-field">
            <span>Existing workspaces</span>
            <select value={workspaceId} onChange={(event) => setWorkspaceId(event.target.value)}
              disabled={loading || busy}>
              <option value="">Select workspace</option>
              {workspaces.map((item) => (
                <option key={item.normalDataWorkspaceId} value={item.normalDataWorkspaceId}>
                  {item.name}
                </option>
              ))}
            </select>
          </label>
          {selectedWorkspace ? (
            <p className="normal-data-meta">
              {selectedWorkspace.description || "No description"} · Created {readableTimestamp(selectedWorkspace.createdAtMs)}
              {selectedWorkspace.periodStart ? (
                <> · {selectedWorkspace.periodStart} to {selectedWorkspace.periodEnd}</>
              ) : null}
            </p>
          ) : <p className="normal-data-meta">Create a workspace to begin without an engagement.</p>}

          <form onSubmit={(event) => void createWorkspace(event)} className="normal-data-form">
            <h4>Create a new workspace</h4>
            <label className="normal-data-field">
              <span>Name</span>
              <input value={workspaceName} onChange={(event) => setWorkspaceName(event.target.value)}
                placeholder="e.g. August filing reconciliation" maxLength={240} required />
            </label>
            <label className="normal-data-field">
              <span>Description (optional)</span>
              <textarea value={workspaceDescription}
                onChange={(event) => setWorkspaceDescription(event.target.value)}
                placeholder="Working-data purpose" maxLength={8192} rows={2} />
            </label>
            <div className="normal-data-pair">
              <label className="normal-data-field">
                <span>Period from (optional)</span>
                <input type="date" value={periodStart} onChange={(event) => setPeriodStart(event.target.value)} />
              </label>
              <label className="normal-data-field">
                <span>Period to (optional)</span>
                <input type="date" value={periodEnd} onChange={(event) => setPeriodEnd(event.target.value)} />
              </label>
            </div>
            <button className="primary-button" type="submit"
              disabled={busy || !workspaceName.trim() || Boolean(periodStart) !== Boolean(periodEnd)}>
              {busy ? "Working…" : "Create workspace"}
            </button>
          </form>
        </section>

        <section className="normal-data-card" aria-label="Indexed dataset sources">
          <h3>2. Dataset sources</h3>
          <p className="normal-data-meta">
            Choose an existing indexed CSV file by ID. Files remain linked working data, not automatically retained evidence.
          </p>
          <label className="normal-data-field">
            <span>Bound datasets</span>
            <select value={datasetVersionId} onChange={(event) => setDatasetVersionId(event.target.value)}
              disabled={!workspaceId || busy}>
              <option value="">Select dataset</option>
              {datasets.map((item) => (
                <option key={item.normalDataDatasetVersionId} value={item.normalDataDatasetVersionId}>
                  {item.name}
                </option>
              ))}
            </select>
          </label>
          {selectedDataset ? (
            <div className="normal-data-source-detail">
              <strong>{selectedDataset.name}</strong>
              <span>{verificationDescription(selectedDataset)}</span>
              <small>Indexed source SHA-256: {selectedDataset.sourceSha256Hex || "Not available"}</small>
              <small>Source version: {selectedDataset.contentVersionId}</small>
            </div>
          ) : <p className="normal-data-meta">No dataset selected.</p>}

          <form onSubmit={(event) => void findSources(event)} className="normal-data-form">
            <h4>Register an indexed CSV</h4>
            <label className="normal-data-field">
              <span>Dataset name (optional)</span>
              <input value={datasetName} maxLength={240}
                onChange={(event) => setDatasetName(event.target.value)}
                placeholder="Defaults to indexed filename" />
            </label>
            <label className="normal-data-field">
              <span>Find indexed CSV files</span>
              <input value={sourceSearch} onChange={(event) => setSourceSearch(event.target.value)}
                placeholder="Filename or path fragment" />
            </label>
            <button className="secondary-button" type="submit"
              disabled={!workspaceId || busy || !sourceSearch.trim()}>
              Search indexed CSV files
            </button>
          </form>
          <div className="normal-data-search-results" aria-live="polite">
            {sourceMatches.map((file) => (
              <article key={file.fileInstanceId} className="normal-data-source-match">
                <div>
                  <strong>{file.name}</strong>
                  <span>{file.path}</span>
                </div>
                <button type="button" className="secondary-button" disabled={busy}
                  onClick={() => void bindDataset(file)}>Bind source</button>
              </article>
            ))}
          </div>
        </section>

        <section className="normal-data-card" aria-label="Dataset column roles">
          <h3>3. Column semantics</h3>
          <p className="normal-data-meta">
            Enter exact CSV header names. These declarations are append-only;
            file structure and values are checked by the native executor.
          </p>
          <div className="normal-data-declarations">
            {columns.length ? columns.map((column) => (
              <div key={column.normalDataColumnSemanticId} className="normal-data-declaration">
                <strong>{column.columnName}</strong>
                <span>{column.semanticRole.replaceAll("_", " ")} · {column.dataType}</span>
              </div>
            )) : <p className="normal-data-meta">No column meanings declared yet.</p>}
          </div>
          <form onSubmit={(event) => void declareColumn(event)} className="normal-data-form">
            <label className="normal-data-field">
              <span>Exact CSV header</span>
              <input value={columnName} maxLength={240} placeholder="e.g. Filing Period"
                onChange={(event) => setColumnName(event.target.value)} required />
            </label>
            <label className="normal-data-field">
              <span>Meaning</span>
              <select value={columnRole} onChange={(event) => setColumnRole(event.target.value)}>
                {COLUMN_ROLES.map((item) => (
                  <option key={item.role} value={item.role}>{item.label}</option>
                ))}
              </select>
            </label>
            <p className="normal-data-meta">
              Stored declaration type: <strong>{currentRole.type}</strong>. Numerical CSV values
              must be explicit integer minor units for the first comparison adapter.
            </p>
            <button type="submit" className="primary-button"
              disabled={!datasetVersionId || busy || !columnName.trim()}>
              Declare column meaning
            </button>
          </form>
        </section>
      </div>
      <p className="normal-data-footnote">
        Normal Data datasets are not controlled audit evidence. Historical comparison
        runs require unchanged, SHA-256-verified source bytes. Recipe setup and
        run history will be surfaced in a following UI slice.
      </p>
    </section>
  );
}
