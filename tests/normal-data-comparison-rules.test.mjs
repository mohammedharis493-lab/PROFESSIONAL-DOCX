import { test } from "node:test";
import assert from "node:assert/strict";
import {
  commonNumericColumns,
  exceptionPreview,
  eligiblePeriodColumns,
  hasVerifiedBinding,
  isValidMinorTolerance,
} from "../src/normal-data-comparison-rules.ts";

const columns = [
  { columnName: "Return month", semanticRole: "FILING_PERIOD", dataType: "PERIOD" },
  { columnName: "Invoice date", semanticRole: "INVOICE_DATE", dataType: "DATE" },
  { columnName: "Posting month", semanticRole: "ACCOUNTING_PERIOD", dataType: "PERIOD" },
  { columnName: "Wrong type", semanticRole: "INVOICE_DATE", dataType: "PERIOD" },
];

test("filing, invoice and accounting bases do not silently substitute each other", () => {
  assert.deepEqual(eligiblePeriodColumns(columns, "FILING_PERIOD").map(c => c.columnName),
    ["Return month"]);
  assert.deepEqual(eligiblePeriodColumns(columns, "INVOICE_MONTH").map(c => c.columnName),
    ["Invoice date"]);
  assert.deepEqual(eligiblePeriodColumns(columns, "ACCOUNTING_PERIOD").map(c => c.columnName),
    ["Posting month"]);
});

test("numeric choices require exact common declared headers and DECIMAL roles", () => {
  const a = [
    { columnName: "Tax minor units", semanticRole: "NUMERIC_VALUE", dataType: "DECIMAL" },
    { columnName: "Tax minor units", semanticRole: "NUMERIC_VALUE", dataType: "DECIMAL" },
    { columnName: "Wrong role", semanticRole: "OTHER", dataType: "DECIMAL" },
    { columnName: "Wrong type", semanticRole: "NUMERIC_VALUE", dataType: "TEXT" },
    { columnName: "A only", semanticRole: "NUMERIC_VALUE", dataType: "DECIMAL" },
  ];
  const b = [
    { columnName: "Tax minor units", semanticRole: "NUMERIC_VALUE", dataType: "DECIMAL" },
    { columnName: "Wrong role", semanticRole: "OTHER", dataType: "DECIMAL" },
    { columnName: "Wrong type", semanticRole: "NUMERIC_VALUE", dataType: "DECIMAL" },
    { columnName: "tax minor units", semanticRole: "NUMERIC_VALUE", dataType: "DECIMAL" },
  ];
  assert.deepEqual(commonNumericColumns(a, b), ["Tax minor units"]);
});

test("minor-unit tolerance rejects silent rounding, leading zeroes and precision loss", () => {
  for (const accepted of ["0", "1", "10025", String(Number.MAX_SAFE_INTEGER)]) {
    assert.equal(isValidMinorTolerance(accepted), true, accepted);
  }
  for (const rejected of ["", "-1", "+1", "01", "1.5", "1e3", " 1",
    "1 ", "9007199254740992", "18446744073709551615"]) {
    assert.equal(isValidMinorTolerance(rejected), false, rejected);
  }
});

test("source labels are advisory and never promote a fingerprint to hash verification", () => {
  assert.equal(hasVerifiedBinding(undefined), false);
  assert.equal(hasVerifiedBinding({ sourceVerificationState: "FINGERPRINTED", sourceSha256Hex: "abc" }), false);
  assert.equal(hasVerifiedBinding({ sourceVerificationState: "HASH_VERIFIED", sourceSha256Hex: null }), false);
  assert.equal(hasVerifiedBinding({ sourceVerificationState: "HASH_VERIFIED", sourceSha256Hex: "abc" }), true);
});

test("exception preview skips matching rows and remains bounded", () => {
  const entries = [
    { businessKey: "A", classification: "PRESENT_BOTH" },
    { businessKey: "B", classification: "PERIOD_MOVED" },
    { businessKey: "C", classification: "AMOUNT_DIFFERENCE" },
    { businessKey: "D", classification: "ONLY_B" },
  ];
  assert.deepEqual(exceptionPreview(entries, 2).map((x) => x.businessKey), ["B", "C"]);
  assert.deepEqual(exceptionPreview(entries, 0), []);
});
