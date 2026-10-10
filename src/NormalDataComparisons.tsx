import { useEffect, useMemo, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./normal-data-comparisons.css";
import {
  PERIOD_BASES,
  commonNumericColumns,
  exceptionPreview,
  eligiblePeriodColumns,
  hasVerifiedBinding,
  isValidMinorTolerance,
  type ColumnSemantic,
  type PeriodBasis,
} from "./normal-data-comparison-rules";

type Dataset = {
  normalDataDatasetVersionId: string;
  name: string;
  sourceVerificationState: string;
  sourceSha256Hex: string | null;
};

type Column = ColumnSemantic;

type Recipe = {
  normalDataComparisonRecipeId: string;
  normalDataComparisonRecipeVersionId: string;
  normalDataWorkspaceId: string;
  versionNumber: number;
  name: string;
  datasetAVersionId: string;
  datasetBVersionId: string;
  periodBasis: PeriodBasis;
  periodColumnA: string;
  periodColumnB: string;
  amountColumns: string[];
  toleranceMinorUnits: number;
  createdAtMs: number;
};

type ComparisonEntry = {
  businessKey: string;
  classification: string;
  periodA: string | null;
  periodB: string | null;
  rowsA: number;
  rowsB: number;
  amountDifferences: unknown[];
};

type ComparisonResult = {
  periodBasis: PeriodBasis;
  amountColumns: string[];
  toleranceMinorUnits: number;
  summary: {
    totalBusinessKeys: number;
    presentBoth: number;
    amountDifferences: number;
    periodMoved: number;
    onlyA: number;
    onlyB: number;
    duplicateKeys: number;
  };
  entries: ComparisonEntry[];
  resultSha256Hex: string;
};

type Run = {
  normalDataComparisonRunId: string;
  normalDataComparisonRecipeVersionId: string;
  datasetASourceSha256Hex: string;
  datasetBSourceSha256Hex: string;
  result: ComparisonResult;
  startedAtMs: number;
  completedAtMs: number;
};

const MAX_PREVIEW = 50;

function formatDate(ms: number) {
  return new Date(ms).toLocaleString();
}

function datasetLabel(dataset: Dataset) {
  return dataset.name + " · " + dataset.normalDataDatasetVersionId.slice(0, 8);
}

export default function NormalDataComparisons({
  workspaceId,
  datasets,
  semanticRevision,
}: {
  workspaceId: string;
  datasets: Dataset[];
  semanticRevision: number;
}) {
  const [recipes, setRecipes] = useState<Recipe[]>([]);
  const [recipeVersionId, setRecipeVersionId] = useState("");
  const [recipeRevision, setRecipeRevision] = useState(0);
  const [runs, setRuns] = useState<Run[]>([]);
  const [runId, setRunId] = useState("");
  const [runRevision, setRunRevision] = useState(0);
  const [datasetA, setDatasetA] = useState("");
  const [datasetB, setDatasetB] = useState("");
  const [columnsA, setColumnsA] = useState<Column[]>([]);
  const [columnsB, setColumnsB] = useState<Column[]>([]);
  const [periodBasis, setPeriodBasis] = useState<PeriodBasis>("FILING_PERIOD");
  const [periodColumnA, setPeriodColumnA] = useState("");
  const [periodColumnB, setPeriodColumnB] = useState("");
  const [amountFields, setAmountFields] = useState<string[]>([]);
  const [recipeName, setRecipeName] = useState("");
  const [tolerance, setTolerance] = useState("0");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [loadingColumns, setLoadingColumns] = useState(false);

  useEffect(() => {
    let live = true;
    if (!workspaceId) {
      setRecipes([]);
      setRecipeVersionId("");
      return () => { live = false; };
    }
    void invoke<Recipe[]>("list_normal_data_comparison_recipes", {
      normalDataWorkspaceId: workspaceId,
    }).then((records) => {
      if (!live) return;
      setRecipes(records);
      setRecipeVersionId((current) =>
        records.some((recipe) => recipe.normalDataComparisonRecipeVersionId === current)
          ? current : (records[0]?.normalDataComparisonRecipeVersionId ?? ""),
      );
    }).catch((cause: unknown) => { if (live) setError(String(cause)); });
    return () => { live = false; };
  }, [workspaceId, recipeRevision]);

  useEffect(() => {
    let live = true;
    setColumnsA([]);
    setColumnsB([]);
    setAmountFields([]);
    setPeriodColumnA("");
    setPeriodColumnB("");
    if (!datasetA || !datasetB || datasetA === datasetB) {
      setLoadingColumns(false);
      return () => { live = false; };
    }
    setLoadingColumns(true);
    void Promise.all([
      invoke<Column[]>("list_normal_data_column_semantics", { normalDataDatasetVersionId: datasetA }),
      invoke<Column[]>("list_normal_data_column_semantics", { normalDataDatasetVersionId: datasetB }),
    ]).then(([a, b]) => {
      if (!live) return;
      setColumnsA(a);
      setColumnsB(b);
    }).catch((cause: unknown) => { if (live) setError(String(cause)); })
      .finally(() => { if (live) setLoadingColumns(false); });
    return () => { live = false; };
  }, [datasetA, datasetB, semanticRevision]);

  useEffect(() => {
    setPeriodColumnA("");
    setPeriodColumnB("");
  }, [periodBasis]);

  useEffect(() => {
    let live = true;
    setRuns([]);
    setRunId("");
    if (!recipeVersionId) return () => { live = false; };
    void invoke<Run[]>("list_normal_data_comparison_runs", {
      normalDataComparisonRecipeVersionId: recipeVersionId,
    }).then((records) => {
      if (!live) return;
      setRuns(records);
      setRunId((current) => records.some((run) => run.normalDataComparisonRunId === current)
        ? current : (records[0]?.normalDataComparisonRunId ?? ""));
    }).catch((cause: unknown) => { if (live) setError(String(cause)); });
    return () => { live = false; };
  }, [recipeVersionId, runRevision]);

  const periodOptionsA = eligiblePeriodColumns(columnsA, periodBasis);
  const periodOptionsB = eligiblePeriodColumns(columnsB, periodBasis);

  const commonAmounts = useMemo(() =>
    commonNumericColumns(columnsA, columnsB), [columnsA, columnsB]);

  const toleranceValid = isValidMinorTolerance(tolerance);
  const a = datasets.find((item) => item.normalDataDatasetVersionId === datasetA);
  const b = datasets.find((item) => item.normalDataDatasetVersionId === datasetB);
  const canCreate = !!workspaceId && !!recipeName.trim() && datasetA !== datasetB &&
    !!a && !!b && !!periodColumnA && !!periodColumnB &&
    periodOptionsA.some((item) => item.columnName === periodColumnA) &&
    periodOptionsB.some((item) => item.columnName === periodColumnB) &&
    amountFields.length > 0 && amountFields.length <= 32 && amountFields.every((name) => commonAmounts.includes(name)) &&
    toleranceValid && !busy && !loadingColumns;
  const selectedRecipe = recipes.find((recipe) =>
    recipe.normalDataComparisonRecipeVersionId === recipeVersionId);
  const selectedRun = runs.find((run) => run.normalDataComparisonRunId === runId);
  const preview = selectedRun ? exceptionPreview(selectedRun.result.entries, MAX_PREVIEW) : [];

  async function createRecipe(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canCreate) return;
    setBusy(true);
    setError("");
    setMessage("");
    try {
      const created = await invoke<Recipe>("create_normal_data_comparison_recipe", {
        input: {
          normalDataWorkspaceId: workspaceId,
          name: recipeName.trim(),
          datasetAVersionId: datasetA,
          datasetBVersionId: datasetB,
          periodBasis,
          periodColumnA,
          periodColumnB,
          amountColumns: [...amountFields].sort(),
          toleranceMinorUnits: Number(tolerance),
        },
      });
      setRecipeName("");
      setRecipeVersionId(created.normalDataComparisonRecipeVersionId);
      setRecipeRevision((current) => current + 1);
      setMessage("Immutable recipe version created. No comparison has been executed.");
    } catch (cause: unknown) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function executeRecipe() {
    if (!selectedRecipe || busy) return;
    setBusy(true);
    setError("");
    setMessage("");
    try {
      const completed = await invoke<Run>("run_normal_data_comparison", {
        normalDataComparisonRecipeVersionId: selectedRecipe.normalDataComparisonRecipeVersionId,
      });
      setRunId(completed.normalDataComparisonRunId);
      setRunRevision((current) => current + 1);
      setMessage("Verified comparison completed and saved in immutable working-data history.");
    } catch (cause: unknown) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  function toggleAmount(column: string, checked: boolean) {
    setAmountFields((current) => checked
      ? [...current.filter((name) => name !== column), column]
      : current.filter((name) => name !== column));
  }

  return (
    <section className="normal-data-comparisons" aria-label="Normal Data comparison recipes and runs">
      <header className="normal-data-heading">
        <div>
          <p className="eyebrow">VERIFIED COMPARISONS</p>
          <h2>4. Recipes and run history</h2>
          <p>Configure exact column roles, execute verified CSV comparisons and inspect append-only results.</p>
        </div>
        <span className="normal-data-count">{recipes.length} recipes</span>
      </header>
      {error ? <p role="alert" className="normal-data-error">{error}</p> : null}
      {message ? <p role="status" className="normal-data-success">{message}</p> : null}
      <div className="normal-data-comparison-grid">
        <section className="normal-data-card" aria-label="Create comparison recipe">
          <h3>Define a comparison</h3>
          <form className="normal-data-form normal-data-comparison-form"
            onSubmit={(event) => void createRecipe(event)}>
            <label className="normal-data-field">
              <span>Recipe name</span>
              <input value={recipeName} maxLength={240}
                onChange={(event) => setRecipeName(event.target.value)}
                placeholder="e.g. Filing month reconciliation" required />
            </label>
            <div className="normal-data-pair">
              <label className="normal-data-field">
                <span>Dataset A</span>
                <select value={datasetA} disabled={!workspaceId || busy}
                  onChange={(event) => setDatasetA(event.target.value)}>
                  <option value="">Select source A</option>
                  {datasets.map((item) => <option key={item.normalDataDatasetVersionId}
                    value={item.normalDataDatasetVersionId}>{datasetLabel(item)}</option>)}
                </select>
              </label>
              <label className="normal-data-field">
                <span>Dataset B</span>
                <select value={datasetB} disabled={!workspaceId || busy}
                  onChange={(event) => setDatasetB(event.target.value)}>
                  <option value="">Select source B</option>
                  {datasets.map((item) => <option key={item.normalDataDatasetVersionId}
                    value={item.normalDataDatasetVersionId}>{datasetLabel(item)}</option>)}
                </select>
              </label>
            </div>
            {a && b && (!hasVerifiedBinding(a) || !hasVerifiedBinding(b)) ? (
              <p className="normal-data-meta" role="status">
                One or both selected datasets are not registered as hash-verified.
                A recipe may be declared, but native execution will reject unverified sources.
              </p>
            ) : null}
            <label className="normal-data-field">
              <span>Comparison period basis</span>
              <select value={periodBasis} onChange={(event) =>
                setPeriodBasis(event.target.value as PeriodBasis)} disabled={busy}>
                {PERIOD_BASES.map((item) =>
                  <option key={item.value} value={item.value}>{item.label}</option>)}
              </select>
            </label>
            <div className="normal-data-pair">
              <label className="normal-data-field">
                <span>Declared period column A</span>
                <select value={periodColumnA} disabled={busy || loadingColumns}
                  onChange={(event) => setPeriodColumnA(event.target.value)}>
                  <option value="">Select exact header</option>
                  {periodOptionsA.map((item) => <option key={item.columnName}
                    value={item.columnName}>{item.columnName}</option>)}
                </select>
              </label>
              <label className="normal-data-field">
                <span>Declared period column B</span>
                <select value={periodColumnB} disabled={busy || loadingColumns}
                  onChange={(event) => setPeriodColumnB(event.target.value)}>
                  <option value="">Select exact header</option>
                  {periodOptionsB.map((item) => <option key={item.columnName}
                    value={item.columnName}>{item.columnName}</option>)}
                </select>
              </label>
            </div>
            <fieldset className="normal-data-comparison-amounts" disabled={busy || loadingColumns}>
              <legend>Common declared numeric columns (integer minor units)</legend>
              {commonAmounts.length ? commonAmounts.map((column) => (
                <label key={column}>
                  <input type="checkbox" checked={amountFields.includes(column)} disabled={!amountFields.includes(column) && amountFields.length >= 32}
                    onChange={(event) => toggleAmount(column, event.target.checked)} />
                  <span>{column}</span>
                </label>
              )) : <p className="normal-data-meta">
                Declare matching NUMERIC_VALUE / DECIMAL headers on both selected datasets first.
              </p>}
            </fieldset>
            <label className="normal-data-field">
              <span>Tolerance in integer minor units (not major currency units)</span>
              <input inputMode="numeric" value={tolerance}
                onChange={(event) => setTolerance(event.target.value)}
                aria-invalid={!!tolerance && !toleranceValid} />
            </label>
            <p className="normal-data-meta">
              Each source needs exactly one declared BUSINESS_KEY. Exact CSV headers,
              numeric values and source SHA-256 are verified only by the native executor.
            </p>
            <button className="primary-button" type="submit" disabled={!canCreate}>
              {busy ? "Working…" : "Save immutable recipe"}
            </button>
          </form>
        </section>

        <section className="normal-data-card" aria-label="Execute existing recipe">
          <h3>Execute an existing recipe</h3>
          <label className="normal-data-field">
            <span>Immutable recipe version</span>
            <select value={recipeVersionId} disabled={!workspaceId || busy}
              onChange={(event) => setRecipeVersionId(event.target.value)}>
              <option value="">Select recipe</option>
              {recipes.map((recipe) => <option key={recipe.normalDataComparisonRecipeVersionId}
                value={recipe.normalDataComparisonRecipeVersionId}>
                {recipe.name} (v{recipe.versionNumber})
              </option>)}
            </select>
          </label>
          {selectedRecipe ? (
            <div className="normal-data-comparison-details">
              <p><strong>Basis:</strong> {selectedRecipe.periodBasis.replaceAll("_", " ")}</p>
              <p><strong>Period headers:</strong> {selectedRecipe.periodColumnA} / {selectedRecipe.periodColumnB}</p>
              <p><strong>Numeric fields:</strong> {selectedRecipe.amountColumns.join(", ")}</p>
              <p><strong>Tolerance:</strong> {selectedRecipe.toleranceMinorUnits} minor units</p>
              <p className="normal-data-meta">Recipe version ID: {selectedRecipe.normalDataComparisonRecipeVersionId}</p>
              <p className="normal-data-meta">
                Input A: {selectedRecipe.datasetAVersionId}
                <br />
                Input B: {selectedRecipe.datasetBVersionId}
              </p>
            </div>
          ) : <p className="normal-data-meta">Create or select a recipe to execute.</p>}
          <button type="button" className="primary-button"
            disabled={!selectedRecipe || busy}
            onClick={() => void executeRecipe()}>
            {busy ? "Running…" : "Verify sources and run"}
          </button>
          <p className="normal-data-meta">
            The native command rechecks approved-root boundaries, registered source hashes,
            strict CSV formats and semantic declarations before appending a run.
            Executing again creates a new immutable run, not an overwrite.
          </p>
        </section>
      </div>

      <section className="normal-data-card normal-data-run-history" aria-label="Immutable comparison run history">
        <h3>Run history</h3>
        <label className="normal-data-field">
          <span>Stored run for selected recipe version</span>
          <select value={runId} disabled={!recipeVersionId || busy}
            onChange={(event) => setRunId(event.target.value)}>
            <option value="">Select historic run</option>
            {runs.map((run) =>
              <option key={run.normalDataComparisonRunId} value={run.normalDataComparisonRunId}>
                {formatDate(run.completedAtMs)} · {run.normalDataComparisonRunId.slice(0, 8)}
              </option>)}
          </select>
        </label>
        {selectedRun ? (
          <>
            <div className="normal-data-comparison-digests">
              <p><strong>Result digest (SHA-256):</strong> <code>{selectedRun.result.resultSha256Hex}</code></p>
              <p><strong>Verified source A (SHA-256):</strong> <code>{selectedRun.datasetASourceSha256Hex}</code></p>
              <p><strong>Verified source B (SHA-256):</strong> <code>{selectedRun.datasetBSourceSha256Hex}</code></p>
              <p className="normal-data-meta">Completed: {formatDate(selectedRun.completedAtMs)} ·
                Run ID: {selectedRun.normalDataComparisonRunId}</p>
            </div>
            <dl className="normal-data-comparison-summary">
              {([
                ["Business keys", selectedRun.result.summary.totalBusinessKeys],
                ["Present both", selectedRun.result.summary.presentBoth],
                ["Amount differences", selectedRun.result.summary.amountDifferences],
                ["Period moved", selectedRun.result.summary.periodMoved],
                ["Only A", selectedRun.result.summary.onlyA],
                ["Only B", selectedRun.result.summary.onlyB],
                ["Duplicate keys", selectedRun.result.summary.duplicateKeys],
              ] as const).map(([name, value]) => (
                <div key={name}><dt>{name}</dt><dd>{value.toLocaleString()}</dd></div>
              ))}
            </dl>
            <div className="normal-data-run-table-wrap">
              <table className="normal-data-run-table">
                <thead><tr><th scope="col">Business key</th><th scope="col">Classification</th>
                  <th scope="col">Period A</th><th scope="col">Period B</th></tr></thead>
                <tbody>
                  {preview.map((entry, index) =>
                    <tr key={entry.businessKey + "-" + index}>
                      <td>{entry.businessKey}</td>
                      <td>{entry.classification.replaceAll("_", " ")}</td>
                      <td>{entry.periodA ?? "—"}</td>
                      <td>{entry.periodB ?? "—"}</td>
                    </tr>)}
                </tbody>
              </table>
            </div>
            <p className="normal-data-meta">
              Showing {preview.length.toLocaleString()} exception rows (maximum {MAX_PREVIEW}) from
              {" "}{selectedRun.result.entries.length.toLocaleString()} classified keys.
              Matching keys are omitted from this preview; all summary counts and hashes
              come from stored immutable history.
              Amount deltas are intentionally omitted: JavaScript JSON parsing can lose precision for large signed integers.
            </p>
          </>
        ) : <p className="normal-data-meta">No completed runs for this recipe version.</p>}
      </section>
    </section>
  );
}
