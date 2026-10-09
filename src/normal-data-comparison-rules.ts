// Pure frontend declaration filtering and lossless configuration validation.
// This never substitutes for native source-byte/header validation.
export type PeriodBasis = "FILING_PERIOD" | "INVOICE_MONTH" | "ACCOUNTING_PERIOD";

export type ColumnSemantic = {
  columnName: string;
  semanticRole: string;
  dataType: string;
};

export type SourceBinding = {
  sourceVerificationState: string;
  sourceSha256Hex: string | null;
};

export const PERIOD_BASES: {
  value: PeriodBasis;
  label: string;
  role: string;
  dataType: string;
}[] = [
  { value: "FILING_PERIOD", label: "Filing period", role: "FILING_PERIOD", dataType: "PERIOD" },
  { value: "INVOICE_MONTH", label: "Invoice month (from invoice date)", role: "INVOICE_DATE", dataType: "DATE" },
  { value: "ACCOUNTING_PERIOD", label: "Accounting period", role: "ACCOUNTING_PERIOD", dataType: "PERIOD" },
];

export function eligiblePeriodColumns(columns: ColumnSemantic[], basis: PeriodBasis): ColumnSemantic[] {
  const expected = PERIOD_BASES.find((item) => item.value === basis);
  if (!expected) return [];
  return columns.filter((item) =>
    item.semanticRole === expected.role && item.dataType === expected.dataType);
}

export function commonNumericColumns(a: ColumnSemantic[], b: ColumnSemantic[]): string[] {
  const right = new Set(b
    .filter((item) => item.semanticRole === "NUMERIC_VALUE" && item.dataType === "DECIMAL")
    .map((item) => item.columnName));
  return [...new Set(a
    .filter((item) => item.semanticRole === "NUMERIC_VALUE" &&
      item.dataType === "DECIMAL" && right.has(item.columnName))
    .map((item) => item.columnName))].sort();
}

export function isValidMinorTolerance(text: string): boolean {
  return /^(0|[1-9][0-9]*)$/.test(text) && Number.isSafeInteger(Number(text));
}

export function hasVerifiedBinding(dataset?: SourceBinding): boolean {
  return !!dataset && dataset.sourceVerificationState === "HASH_VERIFIED" &&
    !!dataset.sourceSha256Hex;
}
