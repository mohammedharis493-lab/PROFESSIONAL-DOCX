import { useEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

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

type RecentSearch = {
  queryText: string;
  normalizedQuery: string;
  lastUsedAtMs: number;
  useCount: number;
};

type Client = {
  clientId: string;
  name: string;
  createdAtMs: number;
};

type ServiceType = {
  serviceTypeId: string;
  name: string;
  createdAtMs: number;
};

type Engagement = {
  engagementId: string;
  clientId: string;
  serviceTypeId: string;
  name: string;
  periodStart: string | null;
  periodEnd: string | null;
  status: string;
  createdAtMs: number;
};

type EngagementTemplate = {
  engagementTemplateId: string;
  name: string;
  description: string | null;
  latestVersionId: string;
  latestVersionNumber: number;
  serviceTypeId: string;
  sourceEngagementId: string | null;
  createdAtMs: number;
};

type FirmLibraryItem = {
  firmLibraryItemId: string;
  category: string;
  name: string;
  description: string | null;
  serviceTypeId: string | null;
  latestVersionId: string;
  latestVersionNumber: number;
  latestDefinitionHashHex: string;
  createdAtMs: number;
};

type FirmLibraryVersion = {
  firmLibraryVersionId: string;
  firmLibraryItemId: string;
  versionNumber: number;
  definitionJson: string;
  definitionHashHex: string;
  createdAtMs: number;
};

type EngagementArea = {
  engagementAreaId: string;
  engagementId: string;
  parentAreaId: string | null;
  name: string;
  code: string | null;
  displayOrder: number;
  status: string;
  createdAtMs: number;
};

type Procedure = {
  procedureId: string;
  engagementId: string;
  engagementAreaId: string | null;
  reference: string | null;
  title: string;
  description: string | null;
  status: string;
  createdAtMs: number;
};

type Workpaper = {
  workpaperId: string;
  engagementId: string;
  engagementAreaId: string | null;
  procedureId: string | null;
  reference: string;
  title: string;
  workflowState: string;
  createdAtMs: number;
  latestRevisionNumber: number | null;
};

type WorkpaperRevision = {
  workpaperRevisionId: string;
  workpaperId: string;
  revisionNumber: number;
  createdAtMs: number;
  revisionReason: string | null;
  supersedesRevisionId: string | null;
  objective: string;
  procedurePerformed: string;
  population: string;
  sample: string;
  exceptions: string;
  managementExplanation: string;
  conclusion: string;
  contentHashHex: string | null;
};

type WorkpaperEvidenceLink = {
  evidenceLinkId: string;
  workpaperRevisionId: string;
  documentId: string;
  documentName: string;
  contentVersionId: string | null;
  contentObservedAtMs: number | null;
  controlledEvidenceVersionId: string | null;
  controlledVersionNumber: number | null;
  controlledCapturedAtMs: number | null;
  relationshipType: string;
  description: string | null;
  createdAtMs: number;
};

type WorkpaperSignoff = {
  signoffId: string;
  workpaperId: string;
  workpaperRevisionId: string;
  revisionNumber: number;
  signoffType: string;
  actorId: string;
  actorRole: string;
  signedAtMs: number;
  comment: string | null;
  evidenceLinkIds: string[];
  supersededAtMs: number | null;
  supersededReason: string | null;
  supersededByRevisionId: string | null;
};

type PbcRequest = {
  pbcRequestId: string;
  engagementId: string;
  engagementAreaId: string | null;
  requestNumber: string;
  description: string;
  requestedFromParty: string;
  dueAtMs: number | null;
  status: string;
  clientVisibleContent: string | null;
  internalNotes: string | null;
  latestAssessment: string | null;
  createdAtMs: number;
  updatedAtMs: number;
};

type PbcRequestEvent = {
  pbcRequestEventId: string;
  pbcRequestId: string;
  eventType: string;
  actorId: string | null;
  fromStatus: string | null;
  toStatus: string | null;
  assessmentText: string | null;
  comment: string | null;
  occurredAtMs: number;
};

type PbcRequestEvidenceLink = {
  pbcRequestEvidenceLinkId: string;
  pbcRequestId: string;
  documentId: string;
  documentName: string;
  contentVersionId: string | null;
  contentObservedAtMs: number | null;
  controlledEvidenceVersionId: string | null;
  controlledVersionNumber: number | null;
  controlledCapturedAtMs: number | null;
  description: string | null;
  createdAtMs: number;
};

type WorkpaperWorkflowEvent = {
  workpaperWorkflowEventId: string;
  workpaperId: string;
  workpaperRevisionId: string | null;
  fromState: string;
  toState: string;
  actorId: string | null;
  comment: string | null;
  occurredAtMs: number;
};

type ReviewNote = {
  reviewNoteId: string;
  workpaperId: string;
  workpaperRevisionId: string;
  evidenceLinkId: string | null;
  title: string;
  body: string;
  ownerId: string | null;
  dueAtMs: number | null;
  locationKind: string | null;
  locationValue: string | null;
  currentState: string;
  raisedBy: string | null;
  createdAtMs: number;
  latestEventAtMs: number;
};

type ReviewNoteEvent = {
  reviewNoteEventId: string;
  reviewNoteId: string;
  eventType: string;
  actorId: string | null;
  responseText: string | null;
  comment: string | null;
  occurredAtMs: number;
};

type ControlledEvidenceVersion = {
  controlledEvidenceVersionId: string;
  evidenceCaptureJobId: string;
  documentId: string;
  sourceContentVersionId: string;
  versionNumber: number;
  sha256Hex: string;
  sizeBytes: number;
  capturedAtMs: number;
  verificationState: string;
};

type DocumentVersionHistoryEntry = {
  contentVersionId: string;
  observedAtMs: number;
  sizeBytes: number;
  lastWriteTimeMs: number | null;
  verificationState: string;
  sourceStableDuringRead: boolean | null;
  sha256Hex: string | null;
  controlledEvidenceVersionId: string | null;
  controlledVersionNumber: number | null;
  capturedAtMs: number | null;
  controlledVerificationState: string | null;
  capturedBy: string | null;
  captureReason: string | null;
  capturePolicy: string | null;
};

type LedgerImport = {
  ledgerImportId: string;
  engagementId: string;
  controlledEvidenceVersionId: string;
  documentId: string;
  sourceContentVersionId: string;
  sourceSha256Hex: string;
  sheetName: string;
  headerRowNumber: number;
  amountColumn: number;
  dateColumn: number | null;
  accountColumn: number | null;
  voucherColumn: number | null;
  narrationColumn: number | null;
  amountScale: number;
  transactionCount: number;
  importedAtMs: number;
};

type LedgerTestRun = {
  ledgerTestRunId: string;
  ledgerImportId: string;
  testType: string;
  thresholdMinor: number;
  exceptionCount: number;
  ranAtMs: number;
};

type LedgerException = {
  ledgerExceptionId: string;
  ledgerTestRunId: string;
  ledgerTransactionId: string;
  exceptionCode: string;
  amountMinor: number;
  transactionDateText: string | null;
  accountText: string | null;
  voucherText: string | null;
  narrationText: string | null;
  controlledEvidenceVersionId: string;
  documentId: string;
  sourceContentVersionId: string;
  sourceSha256Hex: string;
  sheetName: string;
  sourceRowNumber: number;
  sourceRowHashHex: string;
  createdAtMs: number;
};

type LedgerAccountSummary = {
  ledgerImportId: string;
  accountKey: string;
  accountText: string;
  transactionCount: number;
  totalMinor: number;
  firstSourceRowNumber: number;
  lastSourceRowNumber: number;
};

type LedgerTbMapping = {
  ledgerTbMappingId: string;
  ledgerImportId: string;
  trialBalanceImportId: string;
  ledgerAccountKey: string;
  ledgerAccountText: string;
  trialBalanceAccountId: string;
  trialBalanceAccountCodeText: string | null;
  trialBalanceAccountNameText: string;
  trialBalanceSourceRowNumber: number;
  trialBalanceSourceRowHashHex: string;
  versionNumber: number;
  supersedesMappingId: string | null;
  mappedAtMs: number;
};

type FinancialStatementSchedule = {
  financialStatementScheduleId: string;
  engagementId: string;
  reference: string;
  name: string;
  createdAtMs: number;
};

type TrialBalanceScheduleMapping = {
  trialBalanceScheduleMappingId: string;
  trialBalanceImportId: string;
  trialBalanceAccountId: string;
  trialBalanceAccountCodeText: string | null;
  trialBalanceAccountNameText: string;
  trialBalanceSourceRowNumber: number;
  trialBalanceSourceRowHashHex: string;
  financialStatementScheduleId: string;
  scheduleReference: string;
  scheduleName: string;
  versionNumber: number;
  supersedesMappingId: string | null;
  mappedAtMs: number;
};

type TrialBalanceImport = {
  trialBalanceImportId: string;
  engagementId: string;
  controlledEvidenceVersionId: string;
  documentId: string;
  sourceContentVersionId: string;
  sourceSha256Hex: string;
  sheetName: string;
  headerRowNumber: number;
  accountNameColumn: number;
  accountCodeColumn: number | null;
  openingBalanceColumn: number | null;
  closingBalanceColumn: number;
  amountScale: number;
  accountCount: number;
  openingTotalMinor: number;
  closingTotalMinor: number;
  importedAtMs: number;
};

type TrialBalanceAccount = {
  trialBalanceAccountId: string;
  trialBalanceImportId: string;
  sourceRowNumber: number;
  sourceRowHashHex: string;
  accountCodeText: string | null;
  accountNameText: string;
  openingMinor: number;
  closingMinor: number;
  createdAtMs: number;
};

type TrialBalanceMovement = {
  trialBalanceAccountId: string;
  sourceRowNumber: number;
  sourceRowHashHex: string;
  accountCodeText: string | null;
  accountNameText: string;
  openingMinor: number;
  closingMinor: number;
  movementMinor: number;
};

type TrialBalanceComparison = {
  trialBalanceImportId: string;
  openingTotalMinor: number;
  closingTotalMinor: number;
  netMovementMinor: number;
  accountCount: number;
  movements: TrialBalanceMovement[];
};

type ActiveDocumentVersionHistory = {
  file: IndexedFile;
  entries: DocumentVersionHistoryEntry[];
};

type DocumentRelationship = {
  documentRelationshipId: string;
  relationshipType: string;
  direction: string;
  createdAtMs: number;
  relatedDocumentId: string;
  relatedDocumentName: string;
  relatedFile: IndexedFile | null;
};

type ActiveRelationshipContext = {
  file: IndexedFile;
  relationships: DocumentRelationship[];
};

type EvidenceCaptureNotice = {
  fileName: string;
  versionNumber: number;
  sha256Hex: string;
};

type TextPreview = {
  content: string;
  truncated: boolean;
  totalSizeBytes: number;
  previewedBytes: number;
  extension: string;
};

type ActiveTextPreview = {
  file: IndexedFile;
  preview: TextPreview;
};

type ActivePdfPreview = {
  file: IndexedFile;
  url: string;
};

type ActiveImagePreview = {
  file: IndexedFile;
  url: string;
};

type WorkbookSheetInfo = {
  name: string;
  visibility: string;
  sheetType: string;
  previewable: boolean;
};

type WorkbookCell = {
  row: number;
  column: number;
  address: string;
  value: string;
  valueKind: string;
  formula: string | null;
};

type WorkbookRangeInfo = {
  startRow: number;
  startColumn: number;
  endRow: number;
  endColumn: number;
  address: string;
};

type WorkbookHyperlinkInfo = {
  range: WorkbookRangeInfo;
  target: string | null;
  location: string | null;
  displayedText: string | null;
  tooltip: string | null;
};

type WorkbookCommentInfo = {
  row: number;
  column: number;
  address: string;
  author: string | null;
  text: string;
};

type WorkbookPreview = {
  sheets: WorkbookSheetInfo[];
  selectedSheet: string;
  usedStartRow: number;
  usedStartColumn: number;
  usedEndRow: number;
  usedEndColumn: number;
  rowOffset: number;
  columnOffset: number;
  rowCount: number;
  columnCount: number;
  cells: WorkbookCell[];
  mergedRanges: WorkbookRangeInfo[];
  hyperlinks: WorkbookHyperlinkInfo[];
  comments: WorkbookCommentInfo[];
  hiddenRows: number[];
  hiddenColumns: number[];
  ooxmlMetadataAvailable: boolean;
  metadataTruncated: boolean;
  totalSizeBytes: number;
  extension: string;
};

type ActiveWorkbookPreview = {
  file: IndexedFile;
  preview: WorkbookPreview;
  cellsByPosition: Record<string, WorkbookCell>;
  commentsByPosition: Record<string, WorkbookCommentInfo>;
};

type WordParagraphBlock = {
  kind: "paragraph";
  text: string;
  style: string | null;
};

type WordTableBlock = {
  kind: "table";
  rows: string[][];
  truncated: boolean;
};

type WordBlock = WordParagraphBlock | WordTableBlock;

type WordPreview = {
  blocks: WordBlock[];
  truncated: boolean;
  totalSizeBytes: number;
  blockCount: number;
};

type ActiveWordPreview = {
  file: IndexedFile;
  preview: WordPreview;
};

type WorkbookSearchHit = {
  sheetName: string;
  row: number;
  column: number;
  address: string;
  value: string;
  formula: string | null;
  matchedField: string;
};

type WorkbookSearchResult = {
  hits: WorkbookSearchHit[];
  truncated: boolean;
  scannedCells: number;
};

type ViewerTextSegment = {
  key: string;
  text: string;
};

type ViewerLocalMatch = {
  segmentKey: string;
  start: number;
  end: number;
  index: number;
};

type ViewMode = "home" | "clients" | "engagements" | "recent" | "searches" | "pinned";

type NavigationLocation = {
  viewMode: ViewMode;
  query: string;
  storageRootId: string | null;
};

const NAVIGATION_HISTORY_LIMIT = 50;

const TERMINAL_JOB_STATUSES = new Set([
  "COMPLETE",
  "PARTIAL",
  "CANCELLED",
  "OFFLINE",
  "FAILED",
  "INTERRUPTED",
]);

const TEXT_PREVIEW_EXTENSIONS = new Set(["txt", "csv", "xml"]);

const IMAGE_PREVIEW_EXTENSIONS = new Set([
  "png",
  "jpg",
  "jpeg",
  "gif",
  "webp",
  "bmp",
]);

const EXCEL_PREVIEW_EXTENSIONS = new Set(["xlsx", "xlsm", "xls", "xlsb"]);
const WORD_PREVIEW_EXTENSIONS = new Set(["docx"]);
const WORKBOOK_PAGE_ROWS = 60;
const WORKBOOK_PAGE_COLUMNS = 20;

function supportsWorkbookPreview(file: IndexedFile) {
  return EXCEL_PREVIEW_EXTENSIONS.has(file.extension.toLowerCase());
}

function supportsWordPreview(file: IndexedFile) {
  return WORD_PREVIEW_EXTENSIONS.has(file.extension.toLowerCase());
}

function wordParagraphClass(style: string | null) {
  const match = style?.match(/^Heading\s*([1-6])$/i);
  if (!match) return "word-paragraph";

  const level = Number(match[1]);
  return `word-paragraph word-heading word-heading-${level}`;
}

function workbookCellKey(row: number, column: number) {
  return `${row}:${column}`;
}

function wordSearchSegments(preview: WordPreview): ViewerTextSegment[] {
  const segments: ViewerTextSegment[] = [];

  preview.blocks.forEach((block, blockIndex) => {
    if (block.kind === "paragraph") {
      segments.push({ key: `p:${blockIndex}`, text: block.text });
      return;
    }

    block.rows.forEach((row, rowIndex) => {
      row.forEach((cell, cellIndex) => {
        segments.push({
          key: `t:${blockIndex}:${rowIndex}:${cellIndex}`,
          text: cell,
        });
      });
    });
  });

  return segments;
}

function findViewerMatches(
  segments: ViewerTextSegment[],
  query: string,
  limit = 500,
): ViewerLocalMatch[] {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  if (!normalizedQuery) return [];

  const matches: ViewerLocalMatch[] = [];

  for (const segment of segments) {
    const normalizedText = segment.text.toLocaleLowerCase();
    let cursor = 0;

    while (cursor <= normalizedText.length - normalizedQuery.length) {
      const start = normalizedText.indexOf(normalizedQuery, cursor);
      if (start < 0) break;

      matches.push({
        segmentKey: segment.key,
        start,
        end: start + normalizedQuery.length,
        index: matches.length,
      });

      if (matches.length >= limit) return matches;
      cursor = start + Math.max(1, normalizedQuery.length);
    }
  }

  return matches;
}

function renderHighlightedText(
  text: string,
  segmentKey: string,
  matches: ViewerLocalMatch[],
  activeIndex: number,
) {
  const segmentMatches = matches.filter((match) => match.segmentKey === segmentKey);
  if (!segmentMatches.length) return text;

  const content = [];
  let cursor = 0;

  for (const match of segmentMatches) {
    if (match.start > cursor) {
      content.push(text.slice(cursor, match.start));
    }

    const isActive = match.index === activeIndex;
    content.push(
      <mark
        className={isActive ? "viewer-search-mark viewer-search-mark-active" : "viewer-search-mark"}
        id={isActive ? "viewer-search-active" : undefined}
        key={`${segmentKey}:${match.start}:${match.index}`}
      >
        {text.slice(match.start, match.end)}
      </mark>,
    );

    cursor = match.end;
  }

  if (cursor < text.length) {
    content.push(text.slice(cursor));
  }

  return content;
}

function renderInlineQueryHighlight(text: string, query: string) {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  if (!normalizedQuery) return text;

  const normalizedText = text.toLocaleLowerCase();
  const start = normalizedText.indexOf(normalizedQuery);
  if (start < 0) return text;

  const end = start + normalizedQuery.length;
  return (
    <>
      {text.slice(0, start)}
      <mark className="viewer-search-mark">{text.slice(start, end)}</mark>
      {text.slice(end)}
    </>
  );
}

function columnLabel(column: number) {
  let value = column + 1;
  let label = "";

  while (value > 0) {
    const remainder = (value - 1) % 26;
    label = String.fromCharCode(65 + remainder) + label;
    value = Math.floor((value - 1) / 26);
  }

  return label;
}

function supportsTextPreview(file: IndexedFile) {
  return TEXT_PREVIEW_EXTENSIONS.has(file.extension.toLowerCase());
}

function supportsImagePreview(file: IndexedFile) {
  return IMAGE_PREVIEW_EXTENSIONS.has(file.extension.toLowerCase());
}

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

function formatTimestamp(timestampMs: number | null) {
  if (timestampMs === null) return "Not recorded";
  return new Date(timestampMs).toLocaleString();
}

function reviewLocationValuePlaceholder(locationKind: string) {
  switch (locationKind) {
    case "PAGE":
      return "2";
    case "WORKSHEET":
      return "Trial Balance";
    case "CELL":
      return "Trial Balance!B12";
    case "RANGE":
      return "Trial Balance!B12:D20";
    default:
      return "Select a location kind";
  }
}

type EvidenceVersionOption = {
  key: string;
  label: string;
  controlled: boolean;
};

function workpaperEvidenceVersionOptions(
  entries: DocumentVersionHistoryEntry[],
): EvidenceVersionOption[] {
  const options: EvidenceVersionOption[] = [];
  const seen = new Set<string>();

  for (const entry of entries) {
    if (entry.controlledEvidenceVersionId) {
      const key = `controlled:${entry.controlledEvidenceVersionId}`;
      if (!seen.has(key)) {
        seen.add(key);
        options.push({
          key,
          controlled: true,
          label: `Controlled evidence v${entry.controlledVersionNumber ?? "?"} · captured ${formatTimestamp(entry.capturedAtMs)}`,
        });
      }
    }

    const contentKey = `content:${entry.contentVersionId}`;
    if (!seen.has(contentKey)) {
      seen.add(contentKey);
      options.push({
        key: contentKey,
        controlled: false,
        label: `Observed content · ${formatTimestamp(entry.observedAtMs)} · ${entry.verificationState.replaceAll("_", " ")}`,
      });
    }
  }

  return options;
}

function jobLabel(job: IndexJob | null | undefined) {
  if (!job) return "Not indexed";
  return job.status.replaceAll("_", " ");
}

function stateClass(availabilityState: string) {
  return `file-state file-state-${availabilityState.toLowerCase()}`;
}

function stateLabel(availabilityState: string) {
  switch (availabilityState) {
    case "CHANGED":
      return "RECONCILIATION REQUIRED";
    case "UNAVAILABLE":
      return "SOURCE UNAVAILABLE";
    default:
      return availabilityState;
  }
}

const FIRM_LIBRARY_CATEGORIES = [
  { key: "CHECKLIST", label: "Checklist" },
  { key: "AUDIT_QUERY", label: "Audit query" },
  { key: "RISK_TEMPLATE", label: "Risk template" },
  { key: "CONTROL_TEMPLATE", label: "Control template" },
  { key: "LEDGER_SCRUTINY_TEST", label: "Ledger-scrutiny test" },
  { key: "REPORT_TEMPLATE", label: "Report template" },
  { key: "MANAGEMENT_LETTER_POINT", label: "Management-letter point" },
  {
    key: "STATUTORY_COMPLIANCE_REQUIREMENT",
    label: "Statutory compliance requirement",
  },
] as const;

function firmLibraryCategoryLabel(category: string) {
  return (
    FIRM_LIBRARY_CATEGORIES.find((entry) => entry.key === category)?.label ?? category
  );
}

function firmLibraryContentFromDefinition(definitionJson: string) {
  try {
    const definition = JSON.parse(definitionJson) as { content?: unknown };
    return typeof definition.content === "string" ? definition.content : "";
  } catch {
    return "";
  }
}

function parseLedgerColumnIndex(value: string) {
  const normalized = value.trim().toUpperCase();
  if (!normalized) return null;

  if (/^\d+$/.test(normalized)) {
    const oneBased = Number.parseInt(normalized, 10);
    return oneBased >= 1 && oneBased <= 16_384 ? oneBased - 1 : null;
  }

  if (!/^[A-Z]{1,3}$/.test(normalized)) return null;
  let oneBased = 0;
  for (const character of normalized) {
    oneBased = oneBased * 26 + character.charCodeAt(0) - 64;
  }
  return oneBased >= 1 && oneBased <= 16_384 ? oneBased - 1 : null;
}

function parseMinorUnitAmount(value: string, scale: number) {
  const normalized = value.trim().replaceAll(",", "");
  const match = /^([+-]?)(\d*)(?:\.(\d*))?$/.exec(normalized);
  if (!match || (!match[2] && !match[3]) || scale < 0 || scale > 6) return null;

  const sign = match[1] === "-" ? -1n : 1n;
  const integerText = match[2] || "0";
  const fractionText = match[3] || "";
  const extraFraction = fractionText.slice(scale);
  if ([...extraFraction].some((character) => character !== "0")) return null;

  const normalizedFraction = fractionText.slice(0, scale).padEnd(scale, "0");
  const factor = 10n ** BigInt(scale);
  const magnitude =
    BigInt(integerText) * factor + BigInt(normalizedFraction || "0");
  const signed = sign * magnitude;
  if (
    signed > BigInt(Number.MAX_SAFE_INTEGER) ||
    signed < BigInt(Number.MIN_SAFE_INTEGER)
  ) {
    return null;
  }
  return Number(signed);
}

function formatMinorUnitAmount(value: number, scale: number) {
  const sign = value < 0 ? "-" : "";
  const magnitude = Math.abs(value);
  if (scale === 0) return `${sign}${magnitude.toLocaleString()}`;
  const factor = 10 ** scale;
  const integer = Math.floor(magnitude / factor);
  const fraction = String(magnitude % factor).padStart(scale, "0");
  return `${sign}${integer.toLocaleString()}.${fraction}`;
}

function sourceUnavailable(availabilityState: string) {
  return availabilityState === "MISSING" || availabilityState === "UNAVAILABLE";
}

function workflowStateKey(value: string) {
  return value
    .trim()
    .toUpperCase()
    .replace(/[^A-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

function isFormalReviewState(value: string) {
  return new Set(["SUBMITTED_FOR_REVIEW", "REVIEWED", "FINALISED", "FINAL"]).has(
    workflowStateKey(value),
  );
}

function isFormalSignoffType(value: string) {
  return new Set(["REVIEWED", "FINAL_APPROVAL", "FINAL"]).has(workflowStateKey(value));
}

export default function App() {
  const [query, setQuery] = useState("");
  const [clients, setClients] = useState<Client[]>([]);
  const [serviceTypes, setServiceTypes] = useState<ServiceType[]>([]);
  const [engagements, setEngagements] = useState<Engagement[]>([]);
  const [engagementTemplates, setEngagementTemplates] = useState<EngagementTemplate[]>([]);
  const [firmLibraryItems, setFirmLibraryItems] = useState<FirmLibraryItem[]>([]);
  const [selectedFirmLibraryItemId, setSelectedFirmLibraryItemId] =
    useState<string | null>(null);
  const [firmLibraryVersions, setFirmLibraryVersions] = useState<FirmLibraryVersion[]>([]);
  const [firmLibraryFilterCategory, setFirmLibraryFilterCategory] = useState("");
  const [selectedClientId, setSelectedClientId] = useState<string | null>(null);
  const [selectedEngagementId, setSelectedEngagementId] = useState<string | null>(null);
  const [engagementAreas, setEngagementAreas] = useState<EngagementArea[]>([]);
  const [procedures, setProcedures] = useState<Procedure[]>([]);
  const [workpapers, setWorkpapers] = useState<Workpaper[]>([]);
  const [selectedWorkpaperId, setSelectedWorkpaperId] = useState<string | null>(null);
  const [workpaperRevisions, setWorkpaperRevisions] = useState<WorkpaperRevision[]>([]);
  const [workpaperEvidenceLinks, setWorkpaperEvidenceLinks] =
    useState<WorkpaperEvidenceLink[]>([]);
  const [evidenceSearchQuery, setEvidenceSearchQuery] = useState("");
  const [evidenceSearchResults, setEvidenceSearchResults] = useState<SearchResult[]>([]);
  const [selectedEvidenceDocument, setSelectedEvidenceDocument] =
    useState<SearchResult | null>(null);
  const [evidenceVersionHistory, setEvidenceVersionHistory] =
    useState<DocumentVersionHistoryEntry[]>([]);
  const [selectedEvidenceVersionKey, setSelectedEvidenceVersionKey] = useState("");
  const [evidenceRelationshipType, setEvidenceRelationshipType] = useState("SUPPORTS");
  const [evidenceDescription, setEvidenceDescription] = useState("");
  const [evidenceSearchBusy, setEvidenceSearchBusy] = useState(false);
  const [workpaperWorkflowEvents, setWorkpaperWorkflowEvents] =
    useState<WorkpaperWorkflowEvent[]>([]);
  const [workpaperSignoffs, setWorkpaperSignoffs] = useState<WorkpaperSignoff[]>([]);
  const [newSignoffType, setNewSignoffType] = useState("PREPARED");
  const [newSignoffActorId, setNewSignoffActorId] = useState("");
  const [newSignoffActorRole, setNewSignoffActorRole] = useState("");
  const [newSignoffComment, setNewSignoffComment] = useState("");
  const [reviewNotes, setReviewNotes] = useState<ReviewNote[]>([]);
  const [selectedReviewNoteId, setSelectedReviewNoteId] = useState<string | null>(null);
  const [reviewNoteEvents, setReviewNoteEvents] = useState<ReviewNoteEvent[]>([]);
  const [nextWorkflowState, setNextWorkflowState] = useState("PREPARED");
  const [workflowActorId, setWorkflowActorId] = useState("");
  const [workflowComment, setWorkflowComment] = useState("");
  const [newReviewTitle, setNewReviewTitle] = useState("");
  const [newReviewBody, setNewReviewBody] = useState("");
  const [newReviewOwnerId, setNewReviewOwnerId] = useState("");
  const [newReviewDueLocal, setNewReviewDueLocal] = useState("");
  const [newReviewEvidenceLinkId, setNewReviewEvidenceLinkId] = useState("");
  const [newReviewLocationKind, setNewReviewLocationKind] = useState("");
  const [newReviewLocationValue, setNewReviewLocationValue] = useState("");
  const [newReviewRaisedBy, setNewReviewRaisedBy] = useState("");
  const [reviewActionActorId, setReviewActionActorId] = useState("");
  const [reviewResponseText, setReviewResponseText] = useState("");
  const [reviewActionComment, setReviewActionComment] = useState("");
  const [pbcRequests, setPbcRequests] = useState<PbcRequest[]>([]);
  const [selectedPbcRequestId, setSelectedPbcRequestId] = useState<string | null>(null);
  const [pbcRequestEvents, setPbcRequestEvents] = useState<PbcRequestEvent[]>([]);
  const [pbcEvidenceLinks, setPbcEvidenceLinks] = useState<PbcRequestEvidenceLink[]>([]);
  const [newPbcRequestNumber, setNewPbcRequestNumber] = useState("");
  const [newPbcAreaId, setNewPbcAreaId] = useState("");
  const [newPbcDescription, setNewPbcDescription] = useState("");
  const [newPbcRequestedFrom, setNewPbcRequestedFrom] = useState("");
  const [newPbcDueLocal, setNewPbcDueLocal] = useState("");
  const [newPbcClientVisibleContent, setNewPbcClientVisibleContent] = useState("");
  const [newPbcInternalNotes, setNewPbcInternalNotes] = useState("");
  const [pbcNextStatus, setPbcNextStatus] = useState("REQUESTED");
  const [pbcActorId, setPbcActorId] = useState("");
  const [pbcStatusComment, setPbcStatusComment] = useState("");
  const [pbcAssessmentText, setPbcAssessmentText] = useState("");
  const [pbcEvidenceSearchQuery, setPbcEvidenceSearchQuery] = useState("");
  const [pbcEvidenceSearchResults, setPbcEvidenceSearchResults] = useState<SearchResult[]>([]);
  const [selectedPbcEvidenceDocument, setSelectedPbcEvidenceDocument] =
    useState<SearchResult | null>(null);
  const [pbcEvidenceVersionHistory, setPbcEvidenceVersionHistory] =
    useState<DocumentVersionHistoryEntry[]>([]);
  const [selectedPbcEvidenceVersionKey, setSelectedPbcEvidenceVersionKey] = useState("");
  const [pbcEvidenceDescription, setPbcEvidenceDescription] = useState("");
  const [pbcEvidenceSearchBusy, setPbcEvidenceSearchBusy] = useState(false);
  const [workspaceBusy, setWorkspaceBusy] = useState(false);
  const [newClientName, setNewClientName] = useState("");
  const [newServiceTypeName, setNewServiceTypeName] = useState("");
  const [newEngagementName, setNewEngagementName] = useState("");
  const [newEngagementServiceTypeId, setNewEngagementServiceTypeId] = useState("");
  const [newEngagementTemplateVersionId, setNewEngagementTemplateVersionId] = useState("");
  const [newEngagementPeriodStart, setNewEngagementPeriodStart] = useState("");
  const [newEngagementPeriodEnd, setNewEngagementPeriodEnd] = useState("");
  const [newTemplateName, setNewTemplateName] = useState("");
  const [newTemplateDescription, setNewTemplateDescription] = useState("");
  const [templateUpdateId, setTemplateUpdateId] = useState("");
  const [newFirmLibraryCategory, setNewFirmLibraryCategory] = useState("CHECKLIST");
  const [newFirmLibraryName, setNewFirmLibraryName] = useState("");
  const [newFirmLibraryDescription, setNewFirmLibraryDescription] = useState("");
  const [newFirmLibraryServiceTypeId, setNewFirmLibraryServiceTypeId] = useState("");
  const [newFirmLibraryContent, setNewFirmLibraryContent] = useState("");
  const [firmLibraryDraftContent, setFirmLibraryDraftContent] = useState("");
  const [ledgerImports, setLedgerImports] = useState<LedgerImport[]>([]);
  const [selectedLedgerImportId, setSelectedLedgerImportId] = useState<string | null>(null);
  const [ledgerEvidenceSearchQuery, setLedgerEvidenceSearchQuery] = useState("");
  const [ledgerEvidenceSearchResults, setLedgerEvidenceSearchResults] = useState<SearchResult[]>([]);
  const [selectedLedgerEvidenceDocument, setSelectedLedgerEvidenceDocument] =
    useState<SearchResult | null>(null);
  const [ledgerEvidenceVersionHistory, setLedgerEvidenceVersionHistory] =
    useState<DocumentVersionHistoryEntry[]>([]);
  const [selectedLedgerControlledVersionId, setSelectedLedgerControlledVersionId] =
    useState("");
  const [ledgerEvidenceSearchBusy, setLedgerEvidenceSearchBusy] = useState(false);
  const [ledgerSheetName, setLedgerSheetName] = useState("Ledger");
  const [ledgerHeaderRowNumber, setLedgerHeaderRowNumber] = useState("1");
  const [ledgerAmountScale, setLedgerAmountScale] = useState("2");
  const [ledgerDateColumn, setLedgerDateColumn] = useState("A");
  const [ledgerAccountColumn, setLedgerAccountColumn] = useState("B");
  const [ledgerVoucherColumn, setLedgerVoucherColumn] = useState("C");
  const [ledgerNarrationColumn, setLedgerNarrationColumn] = useState("D");
  const [ledgerAmountColumn, setLedgerAmountColumn] = useState("E");
  const [ledgerHighValueThreshold, setLedgerHighValueThreshold] = useState("100000.00");
  const [ledgerTestRuns, setLedgerTestRuns] = useState<LedgerTestRun[]>([]);
  const [ledgerTestRun, setLedgerTestRun] = useState<LedgerTestRun | null>(null);
  const [ledgerExceptions, setLedgerExceptions] = useState<LedgerException[]>([]);
  const [trialBalanceImports, setTrialBalanceImports] = useState<TrialBalanceImport[]>([]);
  const [selectedTrialBalanceImportId, setSelectedTrialBalanceImportId] = useState<string | null>(null);
  const [trialBalanceComparison, setTrialBalanceComparison] =
    useState<TrialBalanceComparison | null>(null);
  const [trialBalanceEvidenceSearchQuery, setTrialBalanceEvidenceSearchQuery] = useState("");
  const [trialBalanceEvidenceSearchResults, setTrialBalanceEvidenceSearchResults] = useState<SearchResult[]>([]);
  const [selectedTrialBalanceEvidenceDocument, setSelectedTrialBalanceEvidenceDocument] =
    useState<SearchResult | null>(null);
  const [trialBalanceEvidenceVersionHistory, setTrialBalanceEvidenceVersionHistory] =
    useState<DocumentVersionHistoryEntry[]>([]);
  const [selectedTrialBalanceControlledVersionId, setSelectedTrialBalanceControlledVersionId] =
    useState("");
  const [trialBalanceEvidenceSearchBusy, setTrialBalanceEvidenceSearchBusy] = useState(false);
  const [trialBalanceSheetName, setTrialBalanceSheetName] = useState("Trial Balance");
  const [trialBalanceHeaderRowNumber, setTrialBalanceHeaderRowNumber] = useState("1");
  const [trialBalanceAmountScale, setTrialBalanceAmountScale] = useState("2");
  const [trialBalanceAccountCodeColumn, setTrialBalanceAccountCodeColumn] = useState("A");
  const [trialBalanceAccountNameColumn, setTrialBalanceAccountNameColumn] = useState("B");
  const [trialBalanceOpeningColumn, setTrialBalanceOpeningColumn] = useState("C");
  const [trialBalanceClosingColumn, setTrialBalanceClosingColumn] = useState("D");
  const [mappingLedgerImportId, setMappingLedgerImportId] = useState<string | null>(null);
  const [mappingTrialBalanceImportId, setMappingTrialBalanceImportId] =
    useState<string | null>(null);
  const [ledgerAccountSummaries, setLedgerAccountSummaries] =
    useState<LedgerAccountSummary[]>([]);
  const [mappingTrialBalanceAccounts, setMappingTrialBalanceAccounts] =
    useState<TrialBalanceAccount[]>([]);
  const [ledgerTbMappings, setLedgerTbMappings] = useState<LedgerTbMapping[]>([]);
  const [selectedMappingLedgerAccountKey, setSelectedMappingLedgerAccountKey] = useState("");
  const [selectedMappingTrialBalanceAccountId, setSelectedMappingTrialBalanceAccountId] =
    useState("");
  const [financialStatementSchedules, setFinancialStatementSchedules] =
    useState<FinancialStatementSchedule[]>([]);
  const [newFinancialStatementScheduleReference, setNewFinancialStatementScheduleReference] =
    useState("");
  const [newFinancialStatementScheduleName, setNewFinancialStatementScheduleName] = useState("");
  const [scheduleMappingTrialBalanceImportId, setScheduleMappingTrialBalanceImportId] =
    useState<string | null>(null);
  const [scheduleMappingTrialBalanceAccounts, setScheduleMappingTrialBalanceAccounts] =
    useState<TrialBalanceAccount[]>([]);
  const [trialBalanceScheduleMappings, setTrialBalanceScheduleMappings] =
    useState<TrialBalanceScheduleMapping[]>([]);
  const [selectedScheduleMappingTrialBalanceAccountId, setSelectedScheduleMappingTrialBalanceAccountId] =
    useState("");
  const [selectedFinancialStatementScheduleId, setSelectedFinancialStatementScheduleId] =
    useState("");
  const [newAreaName, setNewAreaName] = useState("");
  const [newAreaParentId, setNewAreaParentId] = useState("");
  const [newProcedureTitle, setNewProcedureTitle] = useState("");
  const [newProcedureAreaId, setNewProcedureAreaId] = useState("");
  const [newWorkpaperReference, setNewWorkpaperReference] = useState("");
  const [newWorkpaperTitle, setNewWorkpaperTitle] = useState("");
  const [newWorkpaperAreaId, setNewWorkpaperAreaId] = useState("");
  const [newRevisionObjective, setNewRevisionObjective] = useState("");
  const [newRevisionProcedure, setNewRevisionProcedure] = useState("");
  const [newRevisionConclusion, setNewRevisionConclusion] = useState("");
  const [roots, setRoots] = useState<ApprovedStorageRoot[]>([]);
  const [selectedRoot, setSelectedRoot] = useState<ApprovedStorageRoot | null>(null);
  const [latestJobs, setLatestJobs] = useState<Record<string, IndexJob | null>>({});
  const [activeJob, setActiveJob] = useState<IndexJob | null>(null);
  const [previewFiles, setPreviewFiles] = useState<IndexedFile[]>([]);
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const [recentDocuments, setRecentDocuments] = useState<RecentDocument[]>([]);
  const [pinnedDocuments, setPinnedDocuments] = useState<PinnedDocument[]>([]);
  const [recentSearches, setRecentSearches] = useState<RecentSearch[]>([]);
  const [capturingFileInstanceId, setCapturingFileInstanceId] = useState<string | null>(null);
  const [reconcilingFileInstanceId, setReconcilingFileInstanceId] =
    useState<string | null>(null);
  const [relinkingFileInstanceId, setRelinkingFileInstanceId] =
    useState<string | null>(null);
  const [evidenceCaptureNotice, setEvidenceCaptureNotice] =
    useState<EvidenceCaptureNotice | null>(null);
  const [activeVersionHistory, setActiveVersionHistory] =
    useState<ActiveDocumentVersionHistory | null>(null);
  const [versionHistoryLoadingDocumentId, setVersionHistoryLoadingDocumentId] =
    useState<string | null>(null);
  const [activeRelationshipContext, setActiveRelationshipContext] =
    useState<ActiveRelationshipContext | null>(null);
  const [relationshipContextLoadingDocumentId, setRelationshipContextLoadingDocumentId] =
    useState<string | null>(null);
  const [relationshipSearchQuery, setRelationshipSearchQuery] = useState("");
  const [relationshipSearchResults, setRelationshipSearchResults] =
    useState<SearchResult[]>([]);
  const [relationshipType, setRelationshipType] = useState("RELATED");
  const [isRelationshipSearching, setIsRelationshipSearching] = useState(false);
  const [relationshipMutationId, setRelationshipMutationId] =
    useState<string | null>(null);
  const [activeTextPreview, setActiveTextPreview] =
    useState<ActiveTextPreview | null>(null);
  const [activePdfPreview, setActivePdfPreview] =
    useState<ActivePdfPreview | null>(null);
  const [activeImagePreview, setActiveImagePreview] =
    useState<ActiveImagePreview | null>(null);
  const [activeWorkbookPreview, setActiveWorkbookPreview] =
    useState<ActiveWorkbookPreview | null>(null);
  const [activeWordPreview, setActiveWordPreview] =
    useState<ActiveWordPreview | null>(null);
  const [previewingFileInstanceId, setPreviewingFileInstanceId] =
    useState<string | null>(null);
  const [viewerSearchQuery, setViewerSearchQuery] = useState("");
  const [viewerSearchIndex, setViewerSearchIndex] = useState(0);
  const [workbookSearchResult, setWorkbookSearchResult] =
    useState<WorkbookSearchResult | null>(null);
  const [activeWorkbookSearchHit, setActiveWorkbookSearchHit] =
    useState<WorkbookSearchHit | null>(null);
  const [isViewerSearching, setIsViewerSearching] = useState(false);
  const [viewMode, setViewMode] = useState<ViewMode>("home");
  const [selectedSearchIndex, setSelectedSearchIndex] = useState(0);
  const [isLoading, setIsLoading] = useState(true);
  const [isStarting, setIsStarting] = useState(false);
  const [isSearching, setIsSearching] = useState(false);
  const [searchElapsedMs, setSearchElapsedMs] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [searchError, setSearchError] = useState<string | null>(null);
  const pdfBlobUrlRef = useRef<string | null>(null);
  const imageBlobUrlRef = useRef<string | null>(null);
  const searchSequence = useRef(0);
  const navigationSequence = useRef(0);
  const backHistory = useRef<NavigationLocation[]>([]);
  const forwardHistory = useRef<NavigationLocation[]>([]);
  const [, setNavigationRevision] = useState(0);

  useEffect(() => {
    void refreshRoots();
    void refreshQuickAccess();
    void refreshProfessionalWorkspace();
  }, []);

  useEffect(() => {
    return () => {
      if (pdfBlobUrlRef.current) {
        URL.revokeObjectURL(pdfBlobUrlRef.current);
      }
      if (imageBlobUrlRef.current) {
        URL.revokeObjectURL(imageBlobUrlRef.current);
      }
    };
  }, []);

  useEffect(() => {
    const activeMatch = document.getElementById("viewer-search-active");
    if (activeMatch) {
      activeMatch.scrollIntoView({ block: "center", inline: "nearest" });
    }
  }, [viewerSearchIndex, viewerSearchQuery, activeTextPreview, activeWordPreview]);

  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      if (event.altKey && event.key === "ArrowLeft") {
        event.preventDefault();
        void navigateBack();
        return;
      }

      if (event.altKey && event.key === "ArrowRight") {
        event.preventDefault();
        void navigateForward();
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        const input = document.getElementById("universal-search") as HTMLInputElement | null;
        input?.focus();
        input?.select();
      }
    };

    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, [query, roots, selectedRoot?.storageRootId, viewMode]);

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
      setSearchElapsedMs(null);
      setIsSearching(false);
      return;
    }

    setIsSearching(true);
    setSearchError(null);
    setSearchElapsedMs(null);
    const startedAt = performance.now();

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
          setSearchElapsedMs(Math.max(0, Math.round(performance.now() - startedAt)));
        })
        .catch((searchFailure) => {
          if (searchSequence.current !== sequence) return;
          setSearchResults([]);
          setSelectedSearchIndex(0);
          setSearchElapsedMs(null);
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
      const [recents, searches, pins] = await Promise.all([
        invoke<RecentDocument[]>("list_recent_documents", { limit: 20 }),
        invoke<RecentSearch[]>("list_recent_searches", { limit: 20 }),
        invoke<PinnedDocument[]>("list_pinned_documents", { limit: 50 }),
      ]);
      setRecentDocuments(recents);
      setRecentSearches(searches);
      setPinnedDocuments(pins);
    } catch (quickAccessError) {
      setError(String(quickAccessError));
    }
  }

  async function refreshProfessionalWorkspace() {
    try {
      const [
        clientRecords,
        serviceTypeRecords,
        engagementRecords,
        templateRecords,
        firmLibraryRecords,
      ] = await Promise.all([
        invoke<Client[]>("list_clients"),
        invoke<ServiceType[]>("list_service_types"),
        invoke<Engagement[]>("list_engagements", { clientId: null }),
        invoke<EngagementTemplate[]>("list_engagement_templates"),
        invoke<FirmLibraryItem[]>("list_firm_library_items", {
          category: null,
          serviceTypeId: null,
        }),
      ]);
      setClients(clientRecords);
      setServiceTypes(serviceTypeRecords);
      setEngagements(engagementRecords);
      setEngagementTemplates(templateRecords);
      setFirmLibraryItems(firmLibraryRecords);
      if (!newEngagementServiceTypeId && serviceTypeRecords.length) {
        setNewEngagementServiceTypeId(serviceTypeRecords[0].serviceTypeId);
      }
    } catch (workspaceError) {
      setError(String(workspaceError));
    }
  }

  async function loadEngagementWorkspace(engagementId: string) {
    setWorkspaceBusy(true);
    setSelectedEngagementId(engagementId);
    setSelectedWorkpaperId(null);
    setWorkpaperRevisions([]);
    setWorkpaperEvidenceLinks([]);
    setWorkpaperWorkflowEvents([]);
    setWorkpaperSignoffs([]);
    setReviewNotes([]);
    setSelectedReviewNoteId(null);
    setReviewNoteEvents([]);
    setPbcRequests([]);
    setSelectedPbcRequestId(null);
    setPbcRequestEvents([]);
    setPbcEvidenceLinks([]);
    setPbcEvidenceSearchResults([]);
    setSelectedPbcEvidenceDocument(null);
    setPbcEvidenceVersionHistory([]);
    setSelectedPbcEvidenceVersionKey("");
    setEvidenceSearchResults([]);
    setSelectedEvidenceDocument(null);
    setEvidenceVersionHistory([]);
    setSelectedEvidenceVersionKey("");
    setLedgerImports([]);
    setSelectedLedgerImportId(null);
    setLedgerEvidenceSearchResults([]);
    setSelectedLedgerEvidenceDocument(null);
    setLedgerEvidenceVersionHistory([]);
    setSelectedLedgerControlledVersionId("");
    setLedgerTestRuns([]);
    setLedgerTestRun(null);
    setLedgerExceptions([]);
    setTrialBalanceImports([]);
    setSelectedTrialBalanceImportId(null);
    setTrialBalanceComparison(null);
    setTrialBalanceEvidenceSearchResults([]);
    setSelectedTrialBalanceEvidenceDocument(null);
    setTrialBalanceEvidenceVersionHistory([]);
    setSelectedTrialBalanceControlledVersionId("");
    setMappingLedgerImportId(null);
    setMappingTrialBalanceImportId(null);
    setLedgerAccountSummaries([]);
    setMappingTrialBalanceAccounts([]);
    setLedgerTbMappings([]);
    setSelectedMappingLedgerAccountKey("");
    setSelectedMappingTrialBalanceAccountId("");
    setFinancialStatementSchedules([]);
    setScheduleMappingTrialBalanceImportId(null);
    setScheduleMappingTrialBalanceAccounts([]);
    setTrialBalanceScheduleMappings([]);
    setSelectedScheduleMappingTrialBalanceAccountId("");
    setSelectedFinancialStatementScheduleId("");
    try {
      const [
        areas,
        procedureRecords,
        workpaperRecords,
        requestRecords,
        ledgerImportRecords,
        trialBalanceImportRecords,
        scheduleRecords,
      ] = await Promise.all([
        invoke<EngagementArea[]>("list_engagement_areas", { engagementId }),
        invoke<Procedure[]>("list_procedures", { engagementId }),
        invoke<Workpaper[]>("list_workpapers", { engagementId }),
        invoke<PbcRequest[]>("list_pbc_requests", { engagementId }),
        invoke<LedgerImport[]>("list_ledger_imports", { engagementId }),
        invoke<TrialBalanceImport[]>("list_trial_balance_imports", { engagementId }),
        invoke<FinancialStatementSchedule[]>("list_financial_statement_schedules", {
          engagementId,
        }),
      ]);
      setEngagementAreas(areas);
      setProcedures(procedureRecords);
      setWorkpapers(workpaperRecords);
      setPbcRequests(requestRecords);
      setLedgerImports(ledgerImportRecords);
      setTrialBalanceImports(trialBalanceImportRecords);
      setFinancialStatementSchedules(scheduleRecords);
      setSelectedFinancialStatementScheduleId(
        scheduleRecords[0]?.financialStatementScheduleId ?? "",
      );
      const firstTrialBalanceImportId =
        trialBalanceImportRecords[0]?.trialBalanceImportId ?? null;
      setSelectedTrialBalanceImportId(firstTrialBalanceImportId);
      setScheduleMappingTrialBalanceImportId(firstTrialBalanceImportId);
      if (firstTrialBalanceImportId) {
        const [comparison, scheduleAccounts, scheduleMappings] = await Promise.all([
          invoke<TrialBalanceComparison>("compare_trial_balance_opening_closing", {
            trialBalanceImportId: firstTrialBalanceImportId,
          }),
          invoke<TrialBalanceAccount[]>("list_trial_balance_accounts", {
            trialBalanceImportId: firstTrialBalanceImportId,
          }),
          invoke<TrialBalanceScheduleMapping[]>(
            "list_current_trial_balance_schedule_mappings",
            { trialBalanceImportId: firstTrialBalanceImportId },
          ),
        ]);
        setTrialBalanceComparison(comparison);
        setScheduleMappingTrialBalanceAccounts(scheduleAccounts);
        setTrialBalanceScheduleMappings(scheduleMappings);
        const firstScheduleAccountId = scheduleAccounts[0]?.trialBalanceAccountId ?? "";
        setSelectedScheduleMappingTrialBalanceAccountId(firstScheduleAccountId);
        const currentScheduleTarget = scheduleMappings.find(
          (mapping) => mapping.trialBalanceAccountId === firstScheduleAccountId,
        );
        setSelectedFinancialStatementScheduleId(
          currentScheduleTarget?.financialStatementScheduleId ??
            scheduleRecords[0]?.financialStatementScheduleId ??
            "",
        );
      }
      const firstLedgerImport = ledgerImportRecords[0] ?? null;
      const firstLedgerImportId = firstLedgerImport?.ledgerImportId ?? null;
      setSelectedLedgerImportId(firstLedgerImportId);
      if (firstLedgerImport) {
        setMappingLedgerImportId(firstLedgerImport.ledgerImportId);
        const compatibleTrialBalanceImport =
          trialBalanceImportRecords.find(
            (item) => item.amountScale === firstLedgerImport.amountScale,
          ) ?? null;
        setMappingTrialBalanceImportId(
          compatibleTrialBalanceImport?.trialBalanceImportId ?? null,
        );

        const summaries = await invoke<LedgerAccountSummary[]>(
          "list_ledger_account_summaries",
          { ledgerImportId: firstLedgerImport.ledgerImportId },
        );
        setLedgerAccountSummaries(summaries);
        setSelectedMappingLedgerAccountKey(summaries[0]?.accountKey ?? "");

        if (compatibleTrialBalanceImport) {
          const [mappingAccounts, mappings] = await Promise.all([
            invoke<TrialBalanceAccount[]>("list_trial_balance_accounts", {
              trialBalanceImportId: compatibleTrialBalanceImport.trialBalanceImportId,
            }),
            invoke<LedgerTbMapping[]>("list_current_ledger_tb_mappings", {
              ledgerImportId: firstLedgerImport.ledgerImportId,
              trialBalanceImportId: compatibleTrialBalanceImport.trialBalanceImportId,
            }),
          ]);
          setMappingTrialBalanceAccounts(mappingAccounts);
          setLedgerTbMappings(mappings);
          const existingTarget = mappings.find(
            (mapping) => mapping.ledgerAccountKey === summaries[0]?.accountKey,
          );
          setSelectedMappingTrialBalanceAccountId(
            existingTarget?.trialBalanceAccountId ??
              mappingAccounts[0]?.trialBalanceAccountId ??
              "",
          );
        }
      }
      if (firstLedgerImportId) {
        const runRecords = await invoke<LedgerTestRun[]>("list_ledger_test_runs", {
          ledgerImportId: firstLedgerImportId,
        });
        setLedgerTestRuns(runRecords);
        const latestRun = runRecords[0] ?? null;
        setLedgerTestRun(latestRun);
        if (latestRun) {
          const exceptions = await invoke<LedgerException[]>("list_ledger_exceptions", {
            ledgerTestRunId: latestRun.ledgerTestRunId,
          });
          setLedgerExceptions(exceptions);
        }
      }
      setSelectedPbcRequestId(null);
      setPbcRequestEvents([]);
      setPbcEvidenceLinks([]);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function loadWorkpaperEvidenceLinks(workpaperRevisionId: string) {
    const links = await invoke<WorkpaperEvidenceLink[]>("list_workpaper_evidence_links", {
      workpaperRevisionId,
    });
    setWorkpaperEvidenceLinks(links);
  }

  async function loadReviewNoteEvents(reviewNoteId: string) {
    setSelectedReviewNoteId(reviewNoteId);
    try {
      const events = await invoke<ReviewNoteEvent[]>("list_review_note_events", {
        reviewNoteId,
      });
      setReviewNoteEvents(events);
    } catch (workspaceError) {
      setError(String(workspaceError));
      setReviewNoteEvents([]);
    }
  }

  async function refreshWorkpaperReviewState(workpaperId: string) {
    const [workflowEvents, notes, signoffs] = await Promise.all([
      invoke<WorkpaperWorkflowEvent[]>("list_workpaper_workflow_events", {
        workpaperId,
      }),
      invoke<ReviewNote[]>("list_review_notes", { workpaperId }),
      invoke<WorkpaperSignoff[]>("list_workpaper_signoffs", { workpaperId }),
    ]);
    setWorkpaperWorkflowEvents(workflowEvents);
    setReviewNotes(notes);
    setWorkpaperSignoffs(signoffs);
    return notes;
  }

  async function loadWorkpaperRevisions(workpaperId: string) {
    setWorkspaceBusy(true);
    setSelectedWorkpaperId(workpaperId);
    setEvidenceSearchResults([]);
    setSelectedEvidenceDocument(null);
    setEvidenceVersionHistory([]);
    setSelectedEvidenceVersionKey("");
    setSelectedReviewNoteId(null);
    setReviewNoteEvents([]);
    setWorkpaperSignoffs([]);
    try {
      const [revisions] = await Promise.all([
        invoke<WorkpaperRevision[]>("list_workpaper_revisions", { workpaperId }),
        refreshWorkpaperReviewState(workpaperId),
      ]);
      setWorkpaperRevisions(revisions);
      if (revisions.length) {
        await loadWorkpaperEvidenceLinks(revisions[0].workpaperRevisionId);
      } else {
        setWorkpaperEvidenceLinks([]);
      }
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function searchWorkpaperEvidence(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const searchText = evidenceSearchQuery.trim();
    if (latestRevisionSigned || !searchText || !workpaperRevisions.length) return;

    setEvidenceSearchBusy(true);
    try {
      const results = await invoke<SearchResult[]>("search_documents", {
        query: searchText,
        limit: 12,
      });
      setEvidenceSearchResults(results);
      setSelectedEvidenceDocument(null);
      setEvidenceVersionHistory([]);
      setSelectedEvidenceVersionKey("");
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setEvidenceSearchBusy(false);
    }
  }

  async function selectWorkpaperEvidenceDocument(document: SearchResult) {
    setEvidenceSearchBusy(true);
    setSelectedEvidenceDocument(document);
    try {
      const history = await invoke<DocumentVersionHistoryEntry[]>(
        "list_document_version_history",
        { documentId: document.documentId },
      );
      setEvidenceVersionHistory(history);
      const controlled = history.find((entry) => entry.controlledEvidenceVersionId);
      if (controlled?.controlledEvidenceVersionId) {
        setSelectedEvidenceVersionKey(
          `controlled:${controlled.controlledEvidenceVersionId}`,
        );
      } else if (history[0]) {
        setSelectedEvidenceVersionKey(`content:${history[0].contentVersionId}`);
      } else {
        setSelectedEvidenceVersionKey("");
      }
    } catch (workspaceError) {
      setError(String(workspaceError));
      setEvidenceVersionHistory([]);
      setSelectedEvidenceVersionKey("");
    } finally {
      setEvidenceSearchBusy(false);
    }
  }

  async function submitWorkpaperEvidenceLink(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const latestRevision = workpaperRevisions[0];
    if (
      latestRevisionSigned ||
      !latestRevision ||
      !selectedEvidenceDocument ||
      !selectedEvidenceVersionKey
    ) {
      return;
    }

    const [versionKind, versionId] = selectedEvidenceVersionKey.split(":", 2);
    if (!versionId || (versionKind !== "content" && versionKind !== "controlled")) return;

    setWorkspaceBusy(true);
    try {
      await invoke<WorkpaperEvidenceLink>("create_workpaper_evidence_link", {
        workpaperRevisionId: latestRevision.workpaperRevisionId,
        documentId: selectedEvidenceDocument.documentId,
        contentVersionId: versionKind === "content" ? versionId : null,
        controlledEvidenceVersionId: versionKind === "controlled" ? versionId : null,
        relationshipType: evidenceRelationshipType.trim() || "SUPPORTS",
        description: evidenceDescription.trim() || null,
      });
      await loadWorkpaperEvidenceLinks(latestRevision.workpaperRevisionId);
      setEvidenceDescription("");
      setEvidenceSearchResults([]);
      setSelectedEvidenceDocument(null);
      setEvidenceVersionHistory([]);
      setSelectedEvidenceVersionKey("");
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function submitWorkflowTransition(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedWorkpaperId || !nextWorkflowState.trim()) return;

    setWorkspaceBusy(true);
    try {
      await invoke<WorkpaperWorkflowEvent>("transition_workpaper_state", {
        workpaperId: selectedWorkpaperId,
        toState: nextWorkflowState.trim(),
        actorId: workflowActorId.trim() || null,
        comment: workflowComment.trim() || null,
      });
      setWorkflowComment("");
      await refreshWorkpaperReviewState(selectedWorkpaperId);
      if (selectedEngagementId) {
        const refreshed = await invoke<Workpaper[]>("list_workpapers", {
          engagementId: selectedEngagementId,
        });
        setWorkpapers(refreshed);
      }
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function submitWorkpaperSignoff(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const latestRevision = workpaperRevisions[0];
    if (
      !selectedWorkpaperId ||
      !latestRevision ||
      !newSignoffType.trim() ||
      !newSignoffActorId.trim() ||
      !newSignoffActorRole.trim()
    ) {
      return;
    }

    setWorkspaceBusy(true);
    try {
      await invoke<WorkpaperSignoff>("create_workpaper_signoff", {
        workpaperId: selectedWorkpaperId,
        signoff: {
          workpaperRevisionId: latestRevision.workpaperRevisionId,
          signoffType: newSignoffType.trim(),
          actorId: newSignoffActorId.trim(),
          actorRole: newSignoffActorRole.trim(),
          comment: newSignoffComment.trim() || null,
        },
      });
      setNewSignoffComment("");
      await refreshWorkpaperReviewState(selectedWorkpaperId);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function submitReviewNote(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const latestRevision = workpaperRevisions[0];
    if (!selectedWorkpaperId || !latestRevision) return;
    if (!newReviewTitle.trim() || !newReviewBody.trim()) return;

    const dueAtMs = newReviewDueLocal ? new Date(newReviewDueLocal).getTime() : null;
    if (dueAtMs !== null && Number.isNaN(dueAtMs)) {
      setError("Review note due date is invalid.");
      return;
    }

    setWorkspaceBusy(true);
    try {
      const note = await invoke<ReviewNote>("create_review_note", {
        workpaperId: selectedWorkpaperId,
        note: {
          workpaperRevisionId: latestRevision.workpaperRevisionId,
          evidenceLinkId: newReviewEvidenceLinkId || null,
          title: newReviewTitle,
          body: newReviewBody,
          ownerId: newReviewOwnerId.trim() || null,
          dueAtMs,
          locationKind: newReviewLocationKind.trim() || null,
          locationValue: newReviewLocationValue.trim() || null,
          raisedBy: newReviewRaisedBy.trim() || null,
        },
      });
      setNewReviewTitle("");
      setNewReviewBody("");
      setNewReviewOwnerId("");
      setNewReviewDueLocal("");
      setNewReviewEvidenceLinkId("");
      setNewReviewLocationKind("");
      setNewReviewLocationValue("");
      const notes = await refreshWorkpaperReviewState(selectedWorkpaperId);
      if (notes.some((item) => item.reviewNoteId === note.reviewNoteId)) {
        await loadReviewNoteEvents(note.reviewNoteId);
      }
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function submitReviewNoteResponse(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedWorkpaperId || !selectedReviewNoteId || !reviewResponseText.trim()) return;

    setWorkspaceBusy(true);
    try {
      await invoke<ReviewNoteEvent>("respond_to_review_note", {
        reviewNoteId: selectedReviewNoteId,
        action: {
          actorId: reviewActionActorId.trim() || null,
          responseText: reviewResponseText,
          comment: reviewActionComment.trim() || null,
        },
      });
      setReviewResponseText("");
      setReviewActionComment("");
      await refreshWorkpaperReviewState(selectedWorkpaperId);
      await loadReviewNoteEvents(selectedReviewNoteId);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function changeReviewNoteState(actionName: "clear_review_note" | "reopen_review_note") {
    if (!selectedWorkpaperId || !selectedReviewNoteId) return;

    setWorkspaceBusy(true);
    try {
      await invoke<ReviewNoteEvent>(actionName, {
        reviewNoteId: selectedReviewNoteId,
        action: {
          actorId: reviewActionActorId.trim() || null,
          responseText: null,
          comment: reviewActionComment.trim() || null,
        },
      });
      setReviewActionComment("");
      await refreshWorkpaperReviewState(selectedWorkpaperId);
      await loadReviewNoteEvents(selectedReviewNoteId);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function loadPbcRequestDetail(pbcRequestId: string) {
    setWorkspaceBusy(true);
    setSelectedPbcRequestId(pbcRequestId);
    setPbcEvidenceSearchResults([]);
    setSelectedPbcEvidenceDocument(null);
    setPbcEvidenceVersionHistory([]);
    setSelectedPbcEvidenceVersionKey("");
    try {
      const [events, links] = await Promise.all([
        invoke<PbcRequestEvent[]>("list_pbc_request_events", { pbcRequestId }),
        invoke<PbcRequestEvidenceLink[]>("list_pbc_request_evidence_links", {
          pbcRequestId,
        }),
      ]);
      setPbcRequestEvents(events);
      setPbcEvidenceLinks(links);
    } catch (workspaceError) {
      setError(String(workspaceError));
      setPbcRequestEvents([]);
      setPbcEvidenceLinks([]);
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function refreshPbcRequests(engagementId: string) {
    const requests = await invoke<PbcRequest[]>("list_pbc_requests", { engagementId });
    setPbcRequests(requests);
    return requests;
  }

  async function submitPbcRequest(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedEngagementId) return;
    if (
      !newPbcRequestNumber.trim() ||
      !newPbcDescription.trim() ||
      !newPbcRequestedFrom.trim()
    ) {
      return;
    }

    const dueAtMs = newPbcDueLocal ? new Date(newPbcDueLocal).getTime() : null;
    if (dueAtMs !== null && Number.isNaN(dueAtMs)) {
      setError("PBC due date is invalid.");
      return;
    }

    setWorkspaceBusy(true);
    try {
      const created = await invoke<PbcRequest>("create_pbc_request", {
        engagementId: selectedEngagementId,
        request: {
          engagementAreaId: newPbcAreaId || null,
          requestNumber: newPbcRequestNumber.trim(),
          description: newPbcDescription.trim(),
          requestedFromParty: newPbcRequestedFrom.trim(),
          dueAtMs,
          status: "REQUESTED",
          clientVisibleContent: newPbcClientVisibleContent.trim() || null,
          internalNotes: newPbcInternalNotes.trim() || null,
        },
      });
      setNewPbcRequestNumber("");
      setNewPbcAreaId("");
      setNewPbcDescription("");
      setNewPbcRequestedFrom("");
      setNewPbcDueLocal("");
      setNewPbcClientVisibleContent("");
      setNewPbcInternalNotes("");
      await refreshPbcRequests(selectedEngagementId);
      await loadPbcRequestDetail(created.pbcRequestId);
    } catch (workspaceError) {
      setError(String(workspaceError));
      setWorkspaceBusy(false);
    }
  }

  async function submitPbcStatus(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedPbcRequestId || !selectedEngagementId || !pbcNextStatus.trim()) return;

    setWorkspaceBusy(true);
    try {
      await invoke<PbcRequestEvent>("transition_pbc_request_status", {
        pbcRequestId: selectedPbcRequestId,
        toStatus: pbcNextStatus.trim(),
        actorId: pbcActorId.trim() || null,
        comment: pbcStatusComment.trim() || null,
      });
      setPbcStatusComment("");
      await refreshPbcRequests(selectedEngagementId);
      await loadPbcRequestDetail(selectedPbcRequestId);
    } catch (workspaceError) {
      setError(String(workspaceError));
      setWorkspaceBusy(false);
    }
  }

  async function submitPbcAssessment(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedPbcRequestId || !selectedEngagementId || !pbcAssessmentText.trim()) return;

    setWorkspaceBusy(true);
    try {
      await invoke<PbcRequestEvent>("add_pbc_request_assessment", {
        pbcRequestId: selectedPbcRequestId,
        assessmentText: pbcAssessmentText.trim(),
        actorId: pbcActorId.trim() || null,
      });
      setPbcAssessmentText("");
      await refreshPbcRequests(selectedEngagementId);
      await loadPbcRequestDetail(selectedPbcRequestId);
    } catch (workspaceError) {
      setError(String(workspaceError));
      setWorkspaceBusy(false);
    }
  }

  async function searchPbcEvidence(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const searchText = pbcEvidenceSearchQuery.trim();
    if (!selectedPbcRequestId || !searchText) return;

    setPbcEvidenceSearchBusy(true);
    try {
      const results = await invoke<SearchResult[]>("search_documents", {
        query: searchText,
        limit: 12,
      });
      setPbcEvidenceSearchResults(results);
      setSelectedPbcEvidenceDocument(null);
      setPbcEvidenceVersionHistory([]);
      setSelectedPbcEvidenceVersionKey("");
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setPbcEvidenceSearchBusy(false);
    }
  }

  async function selectPbcEvidenceDocument(document: SearchResult) {
    setPbcEvidenceSearchBusy(true);
    setSelectedPbcEvidenceDocument(document);
    try {
      const history = await invoke<DocumentVersionHistoryEntry[]>(
        "list_document_version_history",
        { documentId: document.documentId },
      );
      setPbcEvidenceVersionHistory(history);
      const controlled = history.find((entry) => entry.controlledEvidenceVersionId);
      if (controlled?.controlledEvidenceVersionId) {
        setSelectedPbcEvidenceVersionKey(
          `controlled:${controlled.controlledEvidenceVersionId}`,
        );
      } else if (history[0]) {
        setSelectedPbcEvidenceVersionKey(`content:${history[0].contentVersionId}`);
      } else {
        setSelectedPbcEvidenceVersionKey("");
      }
    } catch (workspaceError) {
      setError(String(workspaceError));
      setPbcEvidenceVersionHistory([]);
      setSelectedPbcEvidenceVersionKey("");
    } finally {
      setPbcEvidenceSearchBusy(false);
    }
  }

  async function submitPbcEvidenceLink(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (
      !selectedPbcRequestId ||
      !selectedPbcEvidenceDocument ||
      !selectedPbcEvidenceVersionKey
    ) {
      return;
    }

    const [versionKind, versionId] = selectedPbcEvidenceVersionKey.split(":", 2);
    if (!versionId || (versionKind !== "content" && versionKind !== "controlled")) return;

    setWorkspaceBusy(true);
    try {
      await invoke<PbcRequestEvidenceLink>("create_pbc_request_evidence_link", {
        pbcRequestId: selectedPbcRequestId,
        documentId: selectedPbcEvidenceDocument.documentId,
        contentVersionId: versionKind === "content" ? versionId : null,
        controlledEvidenceVersionId: versionKind === "controlled" ? versionId : null,
        description: pbcEvidenceDescription.trim() || null,
      });
      setPbcEvidenceDescription("");
      setPbcEvidenceSearchResults([]);
      setSelectedPbcEvidenceDocument(null);
      setPbcEvidenceVersionHistory([]);
      setSelectedPbcEvidenceVersionKey("");
      const links = await invoke<PbcRequestEvidenceLink[]>(
        "list_pbc_request_evidence_links",
        { pbcRequestId: selectedPbcRequestId },
      );
      setPbcEvidenceLinks(links);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function submitClient(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = newClientName.trim();
    if (!name) return;

    setWorkspaceBusy(true);
    try {
      const created = await invoke<Client>("create_client", { name });
      setClients((current) =>
        [...current, created].sort((left, right) => left.name.localeCompare(right.name)),
      );
      setNewClientName("");
      setSelectedClientId(created.clientId);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function submitServiceType(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = newServiceTypeName.trim();
    if (!name) return;

    setWorkspaceBusy(true);
    try {
      const created = await invoke<ServiceType>("create_service_type", { name });
      setServiceTypes((current) =>
        [...current, created].sort((left, right) => left.name.localeCompare(right.name)),
      );
      setNewServiceTypeName("");
      setNewEngagementServiceTypeId(created.serviceTypeId);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function loadFirmLibraryItem(firmLibraryItemId: string) {
    setWorkspaceBusy(true);
    setSelectedFirmLibraryItemId(firmLibraryItemId);
    try {
      const versions = await invoke<FirmLibraryVersion[]>("list_firm_library_versions", {
        firmLibraryItemId,
      });
      setFirmLibraryVersions(versions);
      setFirmLibraryDraftContent(
        versions.length ? firmLibraryContentFromDefinition(versions[0].definitionJson) : "",
      );
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function submitFirmLibraryItem(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = newFirmLibraryName.trim();
    const content = newFirmLibraryContent.trim();
    if (!name || !content) return;

    setWorkspaceBusy(true);
    try {
      const created = await invoke<FirmLibraryItem>("create_firm_library_item", {
        category: newFirmLibraryCategory,
        name,
        description: newFirmLibraryDescription.trim() || null,
        serviceTypeId: newFirmLibraryServiceTypeId || null,
        definitionJson: JSON.stringify({ content }),
      });
      setFirmLibraryItems((current) =>
        [...current, created].sort(
          (left, right) =>
            left.category.localeCompare(right.category) ||
            left.name.localeCompare(right.name),
        ),
      );
      setNewFirmLibraryName("");
      setNewFirmLibraryDescription("");
      setNewFirmLibraryContent("");
      await loadFirmLibraryItem(created.firmLibraryItemId);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function submitFirmLibraryVersion(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedFirmLibraryItemId) return;
    const content = firmLibraryDraftContent.trim();
    if (!content) return;

    setWorkspaceBusy(true);
    try {
      const published = await invoke<FirmLibraryItem>("publish_firm_library_version", {
        firmLibraryItemId: selectedFirmLibraryItemId,
        definitionJson: JSON.stringify({ content }),
      });
      setFirmLibraryItems((current) =>
        current.map((item) =>
          item.firmLibraryItemId === published.firmLibraryItemId ? published : item,
        ),
      );
      await loadFirmLibraryItem(selectedFirmLibraryItemId);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function submitEngagement(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedClientId) return;
    if (!newEngagementTemplateVersionId && !newEngagementServiceTypeId) return;
    const name = newEngagementName.trim();
    if (!name) return;

    setWorkspaceBusy(true);
    try {
      const engagementInput = {
        clientId: selectedClientId,
        name,
        periodStart: newEngagementPeriodStart.trim() || null,
        periodEnd: newEngagementPeriodEnd.trim() || null,
        status: "ACTIVE",
      };
      const created = newEngagementTemplateVersionId
        ? await invoke<Engagement>("create_engagement_from_template", {
            engagementTemplateVersionId: newEngagementTemplateVersionId,
            ...engagementInput,
          })
        : await invoke<Engagement>("create_engagement", {
            serviceTypeId: newEngagementServiceTypeId,
            ...engagementInput,
          });
      setEngagements((current) => [created, ...current]);
      setNewEngagementName("");
      setNewEngagementTemplateVersionId("");
      setNewEngagementPeriodStart("");
      setNewEngagementPeriodEnd("");
      await loadEngagementWorkspace(created.engagementId);
    } catch (workspaceError) {
      setError(String(workspaceError));
      setWorkspaceBusy(false);
    }
  }

  async function submitEngagementTemplate(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedEngagementId) return;
    const name = newTemplateName.trim();
    if (!templateUpdateId && !name) return;

    setWorkspaceBusy(true);
    try {
      const published = templateUpdateId
        ? await invoke<EngagementTemplate>(
            "create_engagement_template_version_from_engagement",
            {
              engagementTemplateId: templateUpdateId,
              sourceEngagementId: selectedEngagementId,
            },
          )
        : await invoke<EngagementTemplate>("create_engagement_template_from_engagement", {
            sourceEngagementId: selectedEngagementId,
            name,
            description: newTemplateDescription.trim() || null,
          });

      setEngagementTemplates((current) => {
        const next = templateUpdateId
          ? current.map((template) =>
              template.engagementTemplateId === published.engagementTemplateId
                ? published
                : template,
            )
          : [...current, published];
        return next.sort((left, right) => left.name.localeCompare(right.name));
      });
      setNewTemplateName("");
      setNewTemplateDescription("");
      setTemplateUpdateId("");
      setNewEngagementTemplateVersionId(published.latestVersionId);
      setNewEngagementServiceTypeId(published.serviceTypeId);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function searchTrialBalanceEvidence(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const searchText = trialBalanceEvidenceSearchQuery.trim();
    if (!searchText) return;

    setTrialBalanceEvidenceSearchBusy(true);
    try {
      const results = await invoke<SearchResult[]>("search_documents", {
        query: searchText,
        limit: 12,
      });
      setTrialBalanceEvidenceSearchResults(
        results.filter((result) =>
          new Set(["xlsx", "xls", "xlsm", "xlsb", "ods"]).has(
            result.extension.toLowerCase().replace(/^\./, ""),
          ),
        ),
      );
      setSelectedTrialBalanceEvidenceDocument(null);
      setTrialBalanceEvidenceVersionHistory([]);
      setSelectedTrialBalanceControlledVersionId("");
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setTrialBalanceEvidenceSearchBusy(false);
    }
  }

  async function selectTrialBalanceEvidenceDocument(document: SearchResult) {
    setTrialBalanceEvidenceSearchBusy(true);
    setSelectedTrialBalanceEvidenceDocument(document);
    try {
      const history = await invoke<DocumentVersionHistoryEntry[]>(
        "list_document_version_history",
        { documentId: document.documentId },
      );
      setTrialBalanceEvidenceVersionHistory(history);
      const controlled = history.find(
        (entry) =>
          entry.controlledEvidenceVersionId &&
          entry.controlledVerificationState === "HASH_VERIFIED",
      );
      setSelectedTrialBalanceControlledVersionId(
        controlled?.controlledEvidenceVersionId ?? "",
      );
    } catch (workspaceError) {
      setError(String(workspaceError));
      setTrialBalanceEvidenceVersionHistory([]);
      setSelectedTrialBalanceControlledVersionId("");
    } finally {
      setTrialBalanceEvidenceSearchBusy(false);
    }
  }

  async function selectTrialBalanceImport(trialBalanceImportId: string | null) {
    setSelectedTrialBalanceImportId(trialBalanceImportId);
    setTrialBalanceComparison(null);
    if (!trialBalanceImportId) return;

    setWorkspaceBusy(true);
    try {
      const comparison = await invoke<TrialBalanceComparison>(
        "compare_trial_balance_opening_closing",
        { trialBalanceImportId },
      );
      setTrialBalanceComparison(comparison);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function submitTrialBalanceImport(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedEngagementId || !selectedTrialBalanceControlledVersionId) return;

    const headerRowNumber = Number.parseInt(trialBalanceHeaderRowNumber, 10);
    const amountScale = Number.parseInt(trialBalanceAmountScale, 10);
    const accountNameColumn = parseLedgerColumnIndex(trialBalanceAccountNameColumn);
    const closingBalanceColumn = parseLedgerColumnIndex(trialBalanceClosingColumn);
    const accountCodeColumn = trialBalanceAccountCodeColumn.trim()
      ? parseLedgerColumnIndex(trialBalanceAccountCodeColumn)
      : null;
    const openingBalanceColumn = trialBalanceOpeningColumn.trim()
      ? parseLedgerColumnIndex(trialBalanceOpeningColumn)
      : null;

    if (
      !Number.isInteger(headerRowNumber) ||
      headerRowNumber < 1 ||
      !Number.isInteger(amountScale) ||
      amountScale < 0 ||
      amountScale > 6 ||
      accountNameColumn === null ||
      closingBalanceColumn === null ||
      (trialBalanceAccountCodeColumn.trim() && accountCodeColumn === null) ||
      (trialBalanceOpeningColumn.trim() && openingBalanceColumn === null)
    ) {
      setError(
        "Trial Balance sheet, header row, scale, and column mapping are invalid. Use Excel letters or 1-based column numbers.",
      );
      return;
    }

    setWorkspaceBusy(true);
    try {
      const created = await invoke<TrialBalanceImport>(
        "import_trial_balance_from_controlled_evidence",
        {
          input: {
            engagementId: selectedEngagementId,
            controlledEvidenceVersionId: selectedTrialBalanceControlledVersionId,
            sheetName: trialBalanceSheetName.trim(),
            headerRowNumber,
            amountScale,
            mapping: {
              accountNameColumn,
              accountCodeColumn,
              openingBalanceColumn,
              closingBalanceColumn,
            },
          },
        },
      );
      const comparison = await invoke<TrialBalanceComparison>(
        "compare_trial_balance_opening_closing",
        { trialBalanceImportId: created.trialBalanceImportId },
      );
      setTrialBalanceImports((current) => [
        created,
        ...current.filter(
          (item) => item.trialBalanceImportId !== created.trialBalanceImportId,
        ),
      ]);
      setSelectedTrialBalanceImportId(created.trialBalanceImportId);
      setTrialBalanceComparison(comparison);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function searchLedgerEvidence(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const searchText = ledgerEvidenceSearchQuery.trim();
    if (!searchText) return;

    setLedgerEvidenceSearchBusy(true);
    try {
      const results = await invoke<SearchResult[]>("search_documents", {
        query: searchText,
        limit: 12,
      });
      setLedgerEvidenceSearchResults(
        results.filter((result) =>
          new Set(["xlsx", "xls", "xlsm", "xlsb", "ods"]).has(
            result.extension.toLowerCase().replace(/^\./, ""),
          ),
        ),
      );
      setSelectedLedgerEvidenceDocument(null);
      setLedgerEvidenceVersionHistory([]);
      setSelectedLedgerControlledVersionId("");
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setLedgerEvidenceSearchBusy(false);
    }
  }

  async function selectLedgerEvidenceDocument(document: SearchResult) {
    setLedgerEvidenceSearchBusy(true);
    setSelectedLedgerEvidenceDocument(document);
    try {
      const history = await invoke<DocumentVersionHistoryEntry[]>(
        "list_document_version_history",
        { documentId: document.documentId },
      );
      setLedgerEvidenceVersionHistory(history);
      const controlled = history.find(
        (entry) =>
          entry.controlledEvidenceVersionId &&
          entry.controlledVerificationState === "HASH_VERIFIED",
      );
      setSelectedLedgerControlledVersionId(
        controlled?.controlledEvidenceVersionId ?? "",
      );
    } catch (workspaceError) {
      setError(String(workspaceError));
      setLedgerEvidenceVersionHistory([]);
      setSelectedLedgerControlledVersionId("");
    } finally {
      setLedgerEvidenceSearchBusy(false);
    }
  }

  async function submitLedgerImport(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedEngagementId || !selectedLedgerControlledVersionId) return;

    const headerRowNumber = Number.parseInt(ledgerHeaderRowNumber, 10);
    const amountScale = Number.parseInt(ledgerAmountScale, 10);
    const amountColumn = parseLedgerColumnIndex(ledgerAmountColumn);
    const optionalColumns = [
      ["date", ledgerDateColumn],
      ["account", ledgerAccountColumn],
      ["voucher", ledgerVoucherColumn],
      ["narration", ledgerNarrationColumn],
    ] as const;

    if (
      !Number.isInteger(headerRowNumber) ||
      headerRowNumber < 1 ||
      !Number.isInteger(amountScale) ||
      amountScale < 0 ||
      amountScale > 6 ||
      amountColumn === null
    ) {
      setError("Ledger sheet, header row, scale, and amount column mapping are invalid.");
      return;
    }

    const parsedOptionalColumns = optionalColumns.map(([label, value]) => {
      if (!value.trim()) return [label, null] as const;
      return [label, parseLedgerColumnIndex(value)] as const;
    });
    const invalidOptional = parsedOptionalColumns.find(
      ([, value], index) => optionalColumns[index][1].trim() && value === null,
    );
    if (invalidOptional) {
      setError(`Ledger ${invalidOptional[0]} column is invalid. Use Excel letters such as A or AA, or a 1-based column number.`);
      return;
    }

    const optionalByLabel = Object.fromEntries(parsedOptionalColumns) as Record<
      string,
      number | null
    >;

    setWorkspaceBusy(true);
    try {
      const created = await invoke<LedgerImport>(
        "import_ledger_from_controlled_evidence",
        {
          input: {
            engagementId: selectedEngagementId,
            controlledEvidenceVersionId: selectedLedgerControlledVersionId,
            sheetName: ledgerSheetName.trim(),
            headerRowNumber,
            amountScale,
            mapping: {
              amountColumn,
              dateColumn: optionalByLabel.date,
              accountColumn: optionalByLabel.account,
              voucherColumn: optionalByLabel.voucher,
              narrationColumn: optionalByLabel.narration,
            },
          },
        },
      );
      setLedgerImports((current) => [
        created,
        ...current.filter((item) => item.ledgerImportId !== created.ledgerImportId),
      ]);
      setSelectedLedgerImportId(created.ledgerImportId);
      setLedgerTestRuns([]);
      setLedgerTestRun(null);
      setLedgerExceptions([]);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function loadLedgerTbMappingPair(
    ledgerImportId: string,
    trialBalanceImportId: string | null,
  ) {
    setWorkspaceBusy(true);
    try {
      const summaries = await invoke<LedgerAccountSummary[]>("list_ledger_account_summaries", {
        ledgerImportId,
      });
      const [accounts, mappings] = trialBalanceImportId
        ? await Promise.all([
            invoke<TrialBalanceAccount[]>("list_trial_balance_accounts", {
              trialBalanceImportId,
            }),
            invoke<LedgerTbMapping[]>("list_current_ledger_tb_mappings", {
              ledgerImportId,
              trialBalanceImportId,
            }),
          ])
        : [[], []];

      setLedgerAccountSummaries(summaries);
      setMappingTrialBalanceAccounts(accounts);
      setLedgerTbMappings(mappings);
      const firstAccountKey = summaries[0]?.accountKey ?? "";
      setSelectedMappingLedgerAccountKey(firstAccountKey);
      const existingTarget = mappings.find(
        (mapping) => mapping.ledgerAccountKey === firstAccountKey,
      );
      setSelectedMappingTrialBalanceAccountId(
        existingTarget?.trialBalanceAccountId ?? accounts[0]?.trialBalanceAccountId ?? "",
      );
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function selectMappingLedgerImport(ledgerImportId: string | null) {
    setMappingLedgerImportId(ledgerImportId);
    setMappingTrialBalanceImportId(null);
    setLedgerAccountSummaries([]);
    setMappingTrialBalanceAccounts([]);
    setLedgerTbMappings([]);
    setSelectedMappingLedgerAccountKey("");
    setSelectedMappingTrialBalanceAccountId("");
    if (!ledgerImportId) return;

    const ledgerImport = ledgerImports.find((item) => item.ledgerImportId === ledgerImportId);
    const compatibleTrialBalanceImport = ledgerImport
      ? trialBalanceImports.find((item) => item.amountScale === ledgerImport.amountScale) ?? null
      : null;
    const trialBalanceImportId = compatibleTrialBalanceImport?.trialBalanceImportId ?? null;
    setMappingTrialBalanceImportId(trialBalanceImportId);
    await loadLedgerTbMappingPair(ledgerImportId, trialBalanceImportId);
  }

  async function selectMappingTrialBalanceImport(trialBalanceImportId: string | null) {
    setMappingTrialBalanceImportId(trialBalanceImportId);
    if (!mappingLedgerImportId) return;
    await loadLedgerTbMappingPair(mappingLedgerImportId, trialBalanceImportId);
  }

  function selectMappingLedgerAccount(accountKey: string) {
    setSelectedMappingLedgerAccountKey(accountKey);
    const existingTarget = ledgerTbMappings.find(
      (mapping) => mapping.ledgerAccountKey === accountKey,
    );
    setSelectedMappingTrialBalanceAccountId(
      existingTarget?.trialBalanceAccountId ??
        mappingTrialBalanceAccounts[0]?.trialBalanceAccountId ??
        "",
    );
  }

  async function submitLedgerTbMapping(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (
      !mappingLedgerImportId ||
      !mappingTrialBalanceImportId ||
      !selectedMappingLedgerAccountKey ||
      !selectedMappingTrialBalanceAccountId
    ) {
      return;
    }

    setWorkspaceBusy(true);
    try {
      await invoke<LedgerTbMapping>("create_ledger_tb_mapping", {
        ledgerImportId: mappingLedgerImportId,
        trialBalanceImportId: mappingTrialBalanceImportId,
        ledgerAccountKey: selectedMappingLedgerAccountKey,
        trialBalanceAccountId: selectedMappingTrialBalanceAccountId,
      });
      const mappings = await invoke<LedgerTbMapping[]>("list_current_ledger_tb_mappings", {
        ledgerImportId: mappingLedgerImportId,
        trialBalanceImportId: mappingTrialBalanceImportId,
      });
      setLedgerTbMappings(mappings);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function submitFinancialStatementSchedule(
    event: React.FormEvent<HTMLFormElement>,
  ) {
    event.preventDefault();
    if (!selectedEngagementId) return;
    const reference = newFinancialStatementScheduleReference.trim();
    const name = newFinancialStatementScheduleName.trim();
    if (!reference || !name) return;

    setWorkspaceBusy(true);
    try {
      const created = await invoke<FinancialStatementSchedule>(
        "create_financial_statement_schedule",
        {
          engagementId: selectedEngagementId,
          reference,
          name,
        },
      );
      setFinancialStatementSchedules((current) =>
        [...current, created].sort((left, right) =>
          left.reference.localeCompare(right.reference),
        ),
      );
      setSelectedFinancialStatementScheduleId(created.financialStatementScheduleId);
      setNewFinancialStatementScheduleReference("");
      setNewFinancialStatementScheduleName("");
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function loadTrialBalanceScheduleMapping(
    trialBalanceImportId: string | null,
  ) {
    setScheduleMappingTrialBalanceAccounts([]);
    setTrialBalanceScheduleMappings([]);
    setSelectedScheduleMappingTrialBalanceAccountId("");
    if (!trialBalanceImportId) return;

    setWorkspaceBusy(true);
    try {
      const [accounts, mappings] = await Promise.all([
        invoke<TrialBalanceAccount[]>("list_trial_balance_accounts", {
          trialBalanceImportId,
        }),
        invoke<TrialBalanceScheduleMapping[]>(
          "list_current_trial_balance_schedule_mappings",
          { trialBalanceImportId },
        ),
      ]);
      setScheduleMappingTrialBalanceAccounts(accounts);
      setTrialBalanceScheduleMappings(mappings);
      const firstAccountId = accounts[0]?.trialBalanceAccountId ?? "";
      setSelectedScheduleMappingTrialBalanceAccountId(firstAccountId);
      const currentTarget = mappings.find(
        (mapping) => mapping.trialBalanceAccountId === firstAccountId,
      );
      setSelectedFinancialStatementScheduleId(
        currentTarget?.financialStatementScheduleId ??
          financialStatementSchedules[0]?.financialStatementScheduleId ??
          "",
      );
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function selectScheduleMappingTrialBalanceImport(
    trialBalanceImportId: string | null,
  ) {
    setScheduleMappingTrialBalanceImportId(trialBalanceImportId);
    await loadTrialBalanceScheduleMapping(trialBalanceImportId);
  }

  function selectScheduleMappingTrialBalanceAccount(trialBalanceAccountId: string) {
    setSelectedScheduleMappingTrialBalanceAccountId(trialBalanceAccountId);
    const currentTarget = trialBalanceScheduleMappings.find(
      (mapping) => mapping.trialBalanceAccountId === trialBalanceAccountId,
    );
    setSelectedFinancialStatementScheduleId(
      currentTarget?.financialStatementScheduleId ??
        financialStatementSchedules[0]?.financialStatementScheduleId ??
        "",
    );
  }

  async function submitTrialBalanceScheduleMapping(
    event: React.FormEvent<HTMLFormElement>,
  ) {
    event.preventDefault();
    if (
      !scheduleMappingTrialBalanceImportId ||
      !selectedScheduleMappingTrialBalanceAccountId ||
      !selectedFinancialStatementScheduleId
    ) {
      return;
    }

    setWorkspaceBusy(true);
    try {
      await invoke<TrialBalanceScheduleMapping>(
        "create_trial_balance_schedule_mapping",
        {
          trialBalanceImportId: scheduleMappingTrialBalanceImportId,
          trialBalanceAccountId: selectedScheduleMappingTrialBalanceAccountId,
          financialStatementScheduleId: selectedFinancialStatementScheduleId,
        },
      );
      const mappings = await invoke<TrialBalanceScheduleMapping[]>(
        "list_current_trial_balance_schedule_mappings",
        { trialBalanceImportId: scheduleMappingTrialBalanceImportId },
      );
      setTrialBalanceScheduleMappings(mappings);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function selectLedgerImportForTesting(ledgerImportId: string | null) {
    setSelectedLedgerImportId(ledgerImportId);
    setLedgerTestRuns([]);
    setLedgerTestRun(null);
    setLedgerExceptions([]);
    if (!ledgerImportId) return;

    setWorkspaceBusy(true);
    try {
      const runs = await invoke<LedgerTestRun[]>("list_ledger_test_runs", {
        ledgerImportId,
      });
      setLedgerTestRuns(runs);
      const latestRun = runs[0] ?? null;
      setLedgerTestRun(latestRun);
      if (latestRun) {
        const exceptions = await invoke<LedgerException[]>("list_ledger_exceptions", {
          ledgerTestRunId: latestRun.ledgerTestRunId,
        });
        setLedgerExceptions(exceptions);
      }
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function openLedgerTestRun(ledgerTestRunId: string) {
    const run = ledgerTestRuns.find((item) => item.ledgerTestRunId === ledgerTestRunId);
    if (!run) {
      setLedgerTestRun(null);
      setLedgerExceptions([]);
      return;
    }

    setWorkspaceBusy(true);
    try {
      const exceptions = await invoke<LedgerException[]>("list_ledger_exceptions", {
        ledgerTestRunId,
      });
      setLedgerTestRun(run);
      setLedgerExceptions(exceptions);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function submitHighValueLedgerTest(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const selectedImport = ledgerImports.find(
      (item) => item.ledgerImportId === selectedLedgerImportId,
    );
    if (!selectedImport) return;

    const thresholdMinor = parseMinorUnitAmount(
      ledgerHighValueThreshold,
      selectedImport.amountScale,
    );
    if (thresholdMinor === null || thresholdMinor <= 0) {
      setError("High-value threshold must be a positive amount within the supported range.");
      return;
    }

    setWorkspaceBusy(true);
    try {
      const run = await invoke<LedgerTestRun>("run_high_value_ledger_test", {
        ledgerImportId: selectedImport.ledgerImportId,
        thresholdMinor,
      });
      const exceptions = await invoke<LedgerException[]>("list_ledger_exceptions", {
        ledgerTestRunId: run.ledgerTestRunId,
      });
      setLedgerTestRuns((current) => [
        run,
        ...current.filter((item) => item.ledgerTestRunId !== run.ledgerTestRunId),
      ]);
      setLedgerTestRun(run);
      setLedgerExceptions(exceptions);
    } catch (workspaceError) {
      setError(String(workspaceError));
    } finally {
      setWorkspaceBusy(false);
    }
  }

  async function submitEngagementArea(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedEngagementId) return;
    const name = newAreaName.trim();
    if (!name) return;

    setWorkspaceBusy(true);
    try {
      await invoke<EngagementArea>("create_engagement_area", {
        engagementId: selectedEngagementId,
        parentAreaId: newAreaParentId || null,
        name,
        code: null,
        displayOrder: engagementAreas.length * 10,
        status: "ACTIVE",
      });
      setNewAreaName("");
      setNewAreaParentId("");
      await loadEngagementWorkspace(selectedEngagementId);
    } catch (workspaceError) {
      setError(String(workspaceError));
      setWorkspaceBusy(false);
    }
  }

  async function submitProcedure(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedEngagementId) return;
    const title = newProcedureTitle.trim();
    if (!title) return;

    setWorkspaceBusy(true);
    try {
      await invoke<Procedure>("create_procedure", {
        engagementId: selectedEngagementId,
        engagementAreaId: newProcedureAreaId || null,
        reference: null,
        title,
        description: null,
        status: "ACTIVE",
      });
      setNewProcedureTitle("");
      setNewProcedureAreaId("");
      await loadEngagementWorkspace(selectedEngagementId);
    } catch (workspaceError) {
      setError(String(workspaceError));
      setWorkspaceBusy(false);
    }
  }

  async function submitWorkpaper(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedEngagementId) return;
    const reference = newWorkpaperReference.trim();
    const title = newWorkpaperTitle.trim();
    if (!reference || !title) return;

    setWorkspaceBusy(true);
    try {
      const created = await invoke<Workpaper>("create_workpaper", {
        engagementId: selectedEngagementId,
        engagementAreaId: newWorkpaperAreaId || null,
        procedureId: null,
        reference,
        title,
        workflowState: "IN_PROGRESS",
      });
      setNewWorkpaperReference("");
      setNewWorkpaperTitle("");
      setNewWorkpaperAreaId("");
      await loadEngagementWorkspace(selectedEngagementId);
      await loadWorkpaperRevisions(created.workpaperId);
    } catch (workspaceError) {
      setError(String(workspaceError));
      setWorkspaceBusy(false);
    }
  }

  async function submitWorkpaperRevision(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedWorkpaperId) return;
    if (
      !newRevisionObjective.trim() &&
      !newRevisionProcedure.trim() &&
      !newRevisionConclusion.trim()
    ) {
      return;
    }

    setWorkspaceBusy(true);
    try {
      await invoke<WorkpaperRevision>("create_workpaper_revision", {
        workpaperId: selectedWorkpaperId,
        revision: {
          revisionReason: workpaperRevisions.length
            ? "Updated from workpaper workspace"
            : "Initial workpaper documentation",
          objective: newRevisionObjective,
          procedurePerformed: newRevisionProcedure,
          population: "",
          sample: "",
          exceptions: "",
          managementExplanation: "",
          conclusion: newRevisionConclusion,
        },
      });
      setNewRevisionObjective("");
      setNewRevisionProcedure("");
      setNewRevisionConclusion("");
      await loadWorkpaperRevisions(selectedWorkpaperId);
      if (selectedEngagementId) {
        const refreshed = await invoke<Workpaper[]>("list_workpapers", {
          engagementId: selectedEngagementId,
        });
        setWorkpapers(refreshed);
      }
    } catch (workspaceError) {
      setError(String(workspaceError));
      setWorkspaceBusy(false);
    }
  }

  function openClientEngagements(clientId: string) {
    setSelectedClientId(clientId);
    setSelectedEngagementId(null);
    setEngagementAreas([]);
    setProcedures([]);
    setWorkpapers([]);
    setSelectedWorkpaperId(null);
    setWorkpaperRevisions([]);
    setWorkpaperEvidenceLinks([]);
    setWorkpaperWorkflowEvents([]);
    setWorkpaperSignoffs([]);
    setReviewNotes([]);
    setSelectedReviewNoteId(null);
    setReviewNoteEvents([]);
    setEvidenceSearchResults([]);
    setSelectedEvidenceDocument(null);
    setEvidenceVersionHistory([]);
    setSelectedEvidenceVersionKey("");
    showView("engagements");
  }

  async function loadPreview(root: ApprovedStorageRoot) {
    const files = await invoke<IndexedFile[]>("list_indexed_file_preview", {
      storageRootId: root.storageRootId,
      limit: 200,
    });
    setPreviewFiles(files);
  }

  function currentNavigationLocation(): NavigationLocation {
    return {
      viewMode,
      query,
      storageRootId: selectedRoot?.storageRootId ?? null,
    };
  }

  function navigationLocationsEqual(
    left: NavigationLocation,
    right: NavigationLocation,
  ) {
    return (
      left.viewMode === right.viewMode &&
      left.query === right.query &&
      left.storageRootId === right.storageRootId
    );
  }

  function pushBoundedHistory(
    history: NavigationLocation[],
    location: NavigationLocation,
  ) {
    history.push(location);
    if (history.length > NAVIGATION_HISTORY_LIMIT) {
      history.splice(0, history.length - NAVIGATION_HISTORY_LIMIT);
    }
  }

  function notifyNavigationHistoryChanged() {
    setNavigationRevision((current) => current + 1);
  }

  function recordNavigationChange(target: NavigationLocation) {
    const current = currentNavigationLocation();
    if (navigationLocationsEqual(current, target)) return;

    pushBoundedHistory(backHistory.current, current);
    forwardHistory.current = [];
    notifyNavigationHistoryChanged();
  }

  async function restoreNavigation(location: NavigationLocation) {
    const sequence = navigationSequence.current + 1;
    navigationSequence.current = sequence;

    setError(null);
    setQuery(location.query);
    setViewMode(location.viewMode);
    setSelectedSearchIndex(0);

    if (!location.storageRootId) {
      setSelectedRoot(null);
      return;
    }

    const root = roots.find(
      (item) => item.storageRootId === location.storageRootId,
    );

    if (!root) {
      setSelectedRoot(null);
      setViewMode("home");
      return;
    }

    setSelectedRoot(root);

    try {
      const [latest, files] = await Promise.all([
        invoke<IndexJob | null>("get_latest_index_job_for_root", {
          storageRootId: root.storageRootId,
        }),
        invoke<IndexedFile[]>("list_indexed_file_preview", {
          storageRootId: root.storageRootId,
          limit: 200,
        }),
      ]);

      if (navigationSequence.current !== sequence) return;

      setActiveJob(latest);
      setPreviewFiles(files);
    } catch (navigationError) {
      if (navigationSequence.current === sequence) {
        setError(String(navigationError));
      }
    }
  }

  async function navigateTo(location: NavigationLocation) {
    recordNavigationChange(location);
    await restoreNavigation(location);
  }

  async function navigateBack() {
    const target = backHistory.current.pop();
    if (!target) return;

    pushBoundedHistory(forwardHistory.current, currentNavigationLocation());
    notifyNavigationHistoryChanged();
    await restoreNavigation(target);
  }

  async function navigateForward() {
    const target = forwardHistory.current.pop();
    if (!target) return;

    pushBoundedHistory(backHistory.current, currentNavigationLocation());
    notifyNavigationHistoryChanged();
    await restoreNavigation(target);
  }

  async function selectRoot(root: ApprovedStorageRoot) {
    await navigateTo({
      viewMode: "home",
      query: "",
      storageRootId: root.storageRootId,
    });
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
    const target: NavigationLocation = {
      viewMode: "home",
      query: "",
      storageRootId: root.storageRootId,
    };
    recordNavigationChange(target);
    navigationSequence.current += 1;

    setError(null);
    setQuery("");
    setViewMode("home");
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

  function resetViewerSearch() {
    setViewerSearchQuery("");
    setViewerSearchIndex(0);
    setWorkbookSearchResult(null);
    setActiveWorkbookSearchHit(null);
    setIsViewerSearching(false);
  }

  function updateViewerSearchQuery(value: string) {
    setViewerSearchQuery(value);
    setViewerSearchIndex(0);
    setWorkbookSearchResult(null);
    setActiveWorkbookSearchHit(null);
  }

  function closeWordPreview() {
    setActiveWordPreview(null);
  }

  async function previewWordFileInstance(file: IndexedFile, usedQuery?: string) {
    if (!supportsWordPreview(file) || sourceUnavailable(file.availabilityState)) {
      return;
    }

    resetViewerSearch();
    setError(null);
    setPreviewingFileInstanceId(file.fileInstanceId);

    try {
      const preview = await invoke<WordPreview>("preview_word_file_instance", {
        fileInstanceId: file.fileInstanceId,
      });

      closePdfPreview();
      closeImagePreview();
      closeWorkbookPreview();
      setActiveTextPreview(null);
      setActiveWordPreview({ file, preview });

      if (usedQuery?.trim()) {
        try {
          await invoke("record_recent_search", { query: usedQuery.trim() });
        } catch (historyError) {
          console.error("Unable to record recent search", historyError);
        }
      }

      await refreshQuickAccess();
    } catch (previewError) {
      setError(String(previewError));
    } finally {
      setPreviewingFileInstanceId(null);
    }
  }

  function closeWorkbookPreview() {
    setActiveWorkbookPreview(null);
  }

  function activateWorkbookPreview(file: IndexedFile, preview: WorkbookPreview) {
    const cellsByPosition = Object.fromEntries(
      preview.cells.map((cell) => [workbookCellKey(cell.row, cell.column), cell]),
    );
    const commentsByPosition = Object.fromEntries(
      preview.comments.map((comment) => [
        workbookCellKey(comment.row, comment.column),
        comment,
      ]),
    );

    setActiveWorkbookPreview({
      file,
      preview,
      cellsByPosition,
      commentsByPosition,
    });
  }

  async function loadWorkbookPreview(
    file: IndexedFile,
    sheetName?: string,
    rowOffset?: number,
    columnOffset?: number,
  ) {
    if (!supportsWorkbookPreview(file) || sourceUnavailable(file.availabilityState)) {
      return false;
    }

    setError(null);
    setPreviewingFileInstanceId(file.fileInstanceId);

    try {
      const preview = await invoke<WorkbookPreview>("preview_workbook_file_instance", {
        fileInstanceId: file.fileInstanceId,
        sheetName,
        rowOffset,
        columnOffset,
        rowLimit: WORKBOOK_PAGE_ROWS,
        columnLimit: WORKBOOK_PAGE_COLUMNS,
      });

      closePdfPreview();
      closeImagePreview();
      closeWordPreview();
      setActiveTextPreview(null);
      activateWorkbookPreview(file, preview);
      return true;
    } catch (previewError) {
      setError(String(previewError));
      return false;
    } finally {
      setPreviewingFileInstanceId(null);
    }
  }

  async function previewWorkbookFileInstance(file: IndexedFile, usedQuery?: string) {
    resetViewerSearch();
    const opened = await loadWorkbookPreview(file);
    if (!opened) return;

    if (usedQuery?.trim()) {
      try {
        await invoke("record_recent_search", { query: usedQuery.trim() });
      } catch (historyError) {
        console.error("Unable to record recent search", historyError);
      }
    }

    await refreshQuickAccess();
  }

  function closePdfPreview() {
    if (pdfBlobUrlRef.current) {
      URL.revokeObjectURL(pdfBlobUrlRef.current);
      pdfBlobUrlRef.current = null;
    }
    setActivePdfPreview(null);
  }

  function closeImagePreview() {
    if (imageBlobUrlRef.current) {
      URL.revokeObjectURL(imageBlobUrlRef.current);
      imageBlobUrlRef.current = null;
    }
    setActiveImagePreview(null);
  }

  async function previewImageFileInstance(file: IndexedFile, usedQuery?: string) {
    if (!supportsImagePreview(file) || sourceUnavailable(file.availabilityState)) {
      return;
    }

    resetViewerSearch();
    setError(null);
    setPreviewingFileInstanceId(file.fileInstanceId);

    try {
      const protocolUrl = convertFileSrc(
        `/image/${file.fileInstanceId}`,
        "pdx-preview",
      );
      const response = await fetch(protocolUrl, { cache: "no-store" });

      if (!response.ok) {
        const message = (await response.text()).trim();
        throw new Error(message || `Image preview failed with status ${response.status}.`);
      }

      const blob = await response.blob();
      const blobUrl = URL.createObjectURL(blob);

      closeImagePreview();
      closePdfPreview();
      closeWorkbookPreview();
      closeWordPreview();
      setActiveTextPreview(null);
      imageBlobUrlRef.current = blobUrl;
      setActiveImagePreview({ file, url: blobUrl });

      if (usedQuery?.trim()) {
        try {
          await invoke("record_recent_search", { query: usedQuery.trim() });
        } catch (historyError) {
          console.error("Unable to record recent search", historyError);
        }
      }

      await refreshQuickAccess();
    } catch (previewError) {
      setError(String(previewError));
    } finally {
      setPreviewingFileInstanceId(null);
    }
  }

  async function previewPdfFileInstance(file: IndexedFile, usedQuery?: string) {
    if (file.extension.toLowerCase() !== "pdf" || sourceUnavailable(file.availabilityState)) {
      return;
    }

    resetViewerSearch();
    setError(null);
    setPreviewingFileInstanceId(file.fileInstanceId);

    try {
      const protocolUrl = convertFileSrc(
        `/pdf/${file.fileInstanceId}`,
        "pdx-preview",
      );
      const response = await fetch(protocolUrl, { cache: "no-store" });

      if (!response.ok) {
        const message = (await response.text()).trim();
        throw new Error(message || `PDF preview failed with status ${response.status}.`);
      }

      const blob = await response.blob();
      const blobUrl = URL.createObjectURL(blob);

      closePdfPreview();
      closeImagePreview();
      closeWorkbookPreview();
      closeWordPreview();
      setActiveTextPreview(null);
      pdfBlobUrlRef.current = blobUrl;
      setActivePdfPreview({ file, url: blobUrl });

      if (usedQuery?.trim()) {
        try {
          await invoke("record_recent_search", { query: usedQuery.trim() });
        } catch (historyError) {
          console.error("Unable to record recent search", historyError);
        }
      }

      await refreshQuickAccess();
    } catch (previewError) {
      setError(String(previewError));
    } finally {
      setPreviewingFileInstanceId(null);
    }
  }

  async function previewFileInstance(file: IndexedFile, usedQuery?: string) {
    if (!supportsTextPreview(file) || sourceUnavailable(file.availabilityState)) {
      return;
    }

    resetViewerSearch();
    setError(null);
    setPreviewingFileInstanceId(file.fileInstanceId);

    try {
      const preview = await invoke<TextPreview>("preview_text_file_instance", {
        fileInstanceId: file.fileInstanceId,
      });

      closePdfPreview();
      closeImagePreview();
      closeWorkbookPreview();
      closeWordPreview();
      setActiveTextPreview({ file, preview });

      if (usedQuery?.trim()) {
        try {
          await invoke("record_recent_search", { query: usedQuery.trim() });
        } catch (historyError) {
          console.error("Unable to record recent search", historyError);
        }
      }

      await refreshQuickAccess();
    } catch (previewError) {
      setError(String(previewError));
    } finally {
      setPreviewingFileInstanceId(null);
    }
  }

  async function openFileInstance(fileInstanceId: string, usedQuery?: string) {
    setError(null);
    try {
      await invoke("open_file_instance", { fileInstanceId });

      if (usedQuery?.trim()) {
        try {
          await invoke("record_recent_search", { query: usedQuery.trim() });
        } catch (historyError) {
          console.error("Unable to record recent search", historyError);
        }
      }

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

  async function navigateToWorkbookSearchHit(
    hit: WorkbookSearchHit,
    index: number,
  ) {
    if (!activeWorkbookPreview) return;

    const file = activeWorkbookPreview.file;
    setViewerSearchIndex(index);
    setActiveWorkbookSearchHit(hit);

    const loaded = await loadWorkbookPreview(
      file,
      hit.sheetName,
      hit.row,
      hit.column,
    );

    if (loaded) {
      window.requestAnimationFrame(() => {
        window.requestAnimationFrame(() => {
          document
            .getElementById("workbook-search-active")
            ?.scrollIntoView({ block: "center", inline: "center" });
        });
      });
    }
  }

  async function runWorkbookViewerSearch() {
    if (!activeWorkbookPreview) return;

    const trimmedQuery = viewerSearchQuery.trim();
    setViewerSearchIndex(0);
    setActiveWorkbookSearchHit(null);

    if (!trimmedQuery) {
      setWorkbookSearchResult(null);
      return;
    }

    setIsViewerSearching(true);
    setError(null);

    try {
      const result = await invoke<WorkbookSearchResult>("search_workbook_file_instance", {
        fileInstanceId: activeWorkbookPreview.file.fileInstanceId,
        query: trimmedQuery,
      });

      setWorkbookSearchResult(result);

      if (result.hits.length) {
        await navigateToWorkbookSearchHit(result.hits[0], 0);
      }
    } catch (viewerSearchError) {
      setWorkbookSearchResult(null);
      setError(String(viewerSearchError));
    } finally {
      setIsViewerSearching(false);
    }
  }

  async function relinkLinkedSource(file: IndexedFile) {
    if (
      !sourceUnavailable(file.availabilityState) ||
      relinkingFileInstanceId !== null
    ) {
      return;
    }

    setError(null);
    setRelinkingFileInstanceId(file.fileInstanceId);

    try {
      const relinked = await invoke<IndexedFile | null>(
        "choose_and_relink_file_instance",
        { fileInstanceId: file.fileInstanceId },
      );

      if (!relinked) {
        return;
      }

      const mergeRelinked = <T extends IndexedFile>(item: T): T =>
        item.fileInstanceId === relinked.fileInstanceId
          ? { ...item, ...relinked }
          : item;

      setSearchResults((current) => current.map(mergeRelinked));
      setRecentDocuments((current) => current.map(mergeRelinked));
      setPinnedDocuments((current) => current.map(mergeRelinked));

      if (selectedRoot) {
        await loadPreview(selectedRoot);
      }
    } catch (relinkError) {
      setError(String(relinkError));
    } finally {
      setRelinkingFileInstanceId(null);
    }
  }

  async function reconcileLinkedSource(file: IndexedFile) {
    if (
      file.availabilityState !== "CHANGED" ||
      reconcilingFileInstanceId !== null
    ) {
      return;
    }

    const confirmed = window.confirm(
      `Reconcile "${file.name}" as the current linked working source?\n\nThis accepts the current indexed source state for future linked-file checks. Historical content-version records and controlled evidence remain unchanged.`,
    );

    if (!confirmed) return;

    setError(null);
    setReconcilingFileInstanceId(file.fileInstanceId);

    try {
      await invoke("reconcile_linked_file_instance", {
        fileInstanceId: file.fileInstanceId,
      });

      setSearchResults((current) =>
        current.map((item) =>
          item.fileInstanceId === file.fileInstanceId
            ? { ...item, availabilityState: "AVAILABLE" }
            : item,
        ),
      );
      setPreviewFiles((current) =>
        current.map((item) =>
          item.fileInstanceId === file.fileInstanceId
            ? { ...item, availabilityState: "AVAILABLE" }
            : item,
        ),
      );
      setRecentDocuments((current) =>
        current.map((item) =>
          item.fileInstanceId === file.fileInstanceId
            ? { ...item, availabilityState: "AVAILABLE" }
            : item,
        ),
      );
      setPinnedDocuments((current) =>
        current.map((item) =>
          item.fileInstanceId === file.fileInstanceId
            ? { ...item, availabilityState: "AVAILABLE" }
            : item,
        ),
      );
    } catch (reconcileError) {
      setError(String(reconcileError));
    } finally {
      setReconcilingFileInstanceId(null);
    }
  }

  async function captureEvidence(file: IndexedFile) {
    if (file.availabilityState !== "AVAILABLE" || capturingFileInstanceId !== null) {
      return;
    }

    const confirmed = window.confirm(
      `Capture "${file.name}" as immutable controlled evidence?\n\nProfessional DocX will preserve a separate verified copy. Later source edits will not change this evidence version, and a later capture will create another version rather than overwrite it.`,
    );

    if (!confirmed) return;

    setError(null);
    setEvidenceCaptureNotice(null);
    setCapturingFileInstanceId(file.fileInstanceId);

    try {
      const captured = await invoke<ControlledEvidenceVersion>(
        "capture_controlled_evidence",
        { fileInstanceId: file.fileInstanceId },
      );

      setEvidenceCaptureNotice({
        fileName: file.name,
        versionNumber: captured.versionNumber,
        sha256Hex: captured.sha256Hex,
      });
    } catch (captureError) {
      setError(String(captureError));
    } finally {
      setCapturingFileInstanceId(null);
    }
  }

  async function loadRelationshipContext(file: IndexedFile) {
    if (relationshipContextLoadingDocumentId !== null) {
      return;
    }

    setError(null);
    setRelationshipContextLoadingDocumentId(file.documentId);

    try {
      const relationships = await invoke<DocumentRelationship[]>(
        "list_document_relationships",
        { documentId: file.documentId },
      );
      setActiveRelationshipContext({ file, relationships });
      setRelationshipSearchQuery("");
      setRelationshipSearchResults([]);
      setRelationshipType("RELATED");
    } catch (relationshipError) {
      setError(String(relationshipError));
    } finally {
      setRelationshipContextLoadingDocumentId(null);
    }
  }

  async function refreshRelationshipContext() {
    if (!activeRelationshipContext) {
      return;
    }

    const relationships = await invoke<DocumentRelationship[]>(
      "list_document_relationships",
      { documentId: activeRelationshipContext.file.documentId },
    );
    setActiveRelationshipContext((current) =>
      current ? { ...current, relationships } : current,
    );
  }

  async function searchRelationshipCandidates() {
    if (!activeRelationshipContext) {
      return;
    }

    const trimmedQuery = relationshipSearchQuery.trim();
    if (!trimmedQuery) {
      setRelationshipSearchResults([]);
      return;
    }

    setIsRelationshipSearching(true);
    setError(null);

    try {
      const results = await invoke<SearchResult[]>("search_documents", {
        query: trimmedQuery,
        limit: 15,
      });
      const seen = new Set<string>();
      setRelationshipSearchResults(
        results.filter((result) => {
          if (
            result.documentId === activeRelationshipContext.file.documentId ||
            seen.has(result.documentId)
          ) {
            return false;
          }
          seen.add(result.documentId);
          return true;
        }),
      );
    } catch (relationshipSearchError) {
      setRelationshipSearchResults([]);
      setError(String(relationshipSearchError));
    } finally {
      setIsRelationshipSearching(false);
    }
  }

  async function addDocumentRelationship(target: IndexedFile) {
    if (!activeRelationshipContext || relationshipMutationId !== null) {
      return;
    }

    const normalizedType = relationshipType.trim();
    if (!normalizedType) {
      setError("Enter a relationship label before linking documents.");
      return;
    }

    setError(null);
    setRelationshipMutationId(target.documentId);

    try {
      await invoke<string>("create_document_relationship", {
        sourceDocumentId: activeRelationshipContext.file.documentId,
        targetDocumentId: target.documentId,
        relationshipType: normalizedType,
      });
      await refreshRelationshipContext();
    } catch (relationshipError) {
      setError(String(relationshipError));
    } finally {
      setRelationshipMutationId(null);
    }
  }

  async function removeDocumentRelationship(relationship: DocumentRelationship) {
    if (!activeRelationshipContext || relationshipMutationId !== null) {
      return;
    }

    const confirmed = window.confirm(
      `Remove the "${relationship.relationshipType}" relationship with "${relationship.relatedDocumentName}"?\n\nThe removal is recorded in the audit trail.`,
    );
    if (!confirmed) {
      return;
    }

    setError(null);
    setRelationshipMutationId(relationship.documentRelationshipId);

    try {
      await invoke("remove_document_relationship", {
        documentRelationshipId: relationship.documentRelationshipId,
      });
      await refreshRelationshipContext();
    } catch (relationshipError) {
      setError(String(relationshipError));
    } finally {
      setRelationshipMutationId(null);
    }
  }

  async function showVersionHistory(file: IndexedFile) {
    if (versionHistoryLoadingDocumentId !== null) {
      return;
    }

    setError(null);
    setVersionHistoryLoadingDocumentId(file.documentId);

    try {
      const entries = await invoke<DocumentVersionHistoryEntry[]>(
        "list_document_version_history",
        { documentId: file.documentId },
      );
      setActiveVersionHistory({ file, entries });
    } catch (historyError) {
      setError(String(historyError));
    } finally {
      setVersionHistoryLoadingDocumentId(null);
    }
  }

  const localViewerSegments: ViewerTextSegment[] = activeTextPreview
    ? [{ key: "text", text: activeTextPreview.preview.content }]
    : activeWordPreview
      ? wordSearchSegments(activeWordPreview.preview)
      : [];

  const localViewerMatches = findViewerMatches(
    localViewerSegments,
    viewerSearchQuery,
  );

  const workbookSearchHits = workbookSearchResult?.hits ?? [];
  const localSearchTruncated = localViewerMatches.length >= 500;
  const viewerMatchCount = activeWorkbookPreview
    ? workbookSearchHits.length
    : localViewerMatches.length;
  const safeViewerSearchIndex =
    viewerMatchCount > 0
      ? Math.min(viewerSearchIndex, viewerMatchCount - 1)
      : 0;

  const workbookSearchHitKeys = new Set(
    workbookSearchHits.map(
      (hit) => `${hit.sheetName}:${hit.row}:${hit.column}`,
    ),
  );
  const workbookHiddenRows = new Set(
    activeWorkbookPreview?.preview.hiddenRows ?? [],
  );
  const workbookHiddenColumns = new Set(
    activeWorkbookPreview?.preview.hiddenColumns ?? [],
  );

  function moveViewerSearch(delta: number) {
    if (!viewerMatchCount) return;

    const nextIndex =
      (safeViewerSearchIndex + delta + viewerMatchCount) % viewerMatchCount;

    if (activeWorkbookPreview) {
      const hit = workbookSearchHits[nextIndex];
      if (hit) {
        void navigateToWorkbookSearchHit(hit, nextIndex);
      }
      return;
    }

    setViewerSearchIndex(nextIndex);
  }

  function renderViewerSearchBar(mode: "local" | "workbook") {
    const hasQuery = Boolean(viewerSearchQuery.trim());
    const hasExecutedWorkbookSearch = workbookSearchResult !== null;
    const showCount =
      mode === "local"
        ? hasQuery
        : hasQuery && hasExecutedWorkbookSearch;
    const countLabel = !showCount
      ? "Find in preview"
      : viewerMatchCount
        ? `${safeViewerSearchIndex + 1} of ${viewerMatchCount}${activeWorkbookPreview ? (workbookSearchResult?.truncated ? "+" : "") : (localSearchTruncated ? "+" : "")}`
        : "No matches";

    return (
      <form
        className="viewer-search-bar"
        onSubmit={(event) => {
          event.preventDefault();
          if (mode === "workbook") {
            void runWorkbookViewerSearch();
          } else {
            setViewerSearchIndex(0);
          }
        }}
      >
        <input
          aria-label="Find in document"
          placeholder="Find in document…"
          value={viewerSearchQuery}
          onChange={(event) => updateViewerSearchQuery(event.target.value)}
        />
        <button
          className="file-action"
          type="submit"
          disabled={mode === "workbook" && isViewerSearching}
        >
          {mode === "workbook" && isViewerSearching ? "Searching…" : "Find"}
        </button>
        <span className="viewer-search-count">{countLabel}</span>
        <button
          className="file-action"
          type="button"
          onClick={() => moveViewerSearch(-1)}
          disabled={!viewerMatchCount}
          aria-label="Previous match"
        >
          ↑
        </button>
        <button
          className="file-action"
          type="button"
          onClick={() => moveViewerSearch(1)}
          disabled={!viewerMatchCount}
          aria-label="Next match"
        >
          ↓
        </button>
        {mode === "workbook" && workbookSearchResult ? (
          <span className="viewer-search-scope">
            {workbookSearchResult.scannedCells.toLocaleString()} cells scanned
            {workbookSearchResult.truncated ? " · result limit reached" : ""}
          </span>
        ) : null}
      </form>
    );
  }

  function renderPreviewAction(file: IndexedFile, usedQuery?: string) {
    const isTextPreview = supportsTextPreview(file);
    const isPdfPreview = file.extension.toLowerCase() === "pdf";
    const isImagePreview = supportsImagePreview(file);
    const isWorkbookPreview = supportsWorkbookPreview(file);
    const isWordPreview = supportsWordPreview(file);
    if (
      !isTextPreview &&
      !isPdfPreview &&
      !isImagePreview &&
      !isWorkbookPreview &&
      !isWordPreview
    ) {
      return null;
    }

    const isThisPreview = previewingFileInstanceId === file.fileInstanceId;

    return (
      <button
        className="file-action file-action-preview"
        type="button"
        onClick={() =>
          void (isPdfPreview
            ? previewPdfFileInstance(file, usedQuery)
            : isImagePreview
              ? previewImageFileInstance(file, usedQuery)
              : isWorkbookPreview
                ? previewWorkbookFileInstance(file, usedQuery)
                : isWordPreview
                  ? previewWordFileInstance(file, usedQuery)
                  : previewFileInstance(file, usedQuery))
        }
        disabled={sourceUnavailable(file.availabilityState) || previewingFileInstanceId !== null}
        title={
          isPdfPreview
            ? "Preview PDF safely inside Professional DocX"
            : isImagePreview
              ? "Preview raster image safely inside Professional DocX"
              : isWorkbookPreview
                ? "Preview workbook safely inside Professional DocX"
                : isWordPreview
                  ? "Preview DOCX safely inside Professional DocX"
                  : "Preview text safely inside Professional DocX"
        }
      >
        {isThisPreview ? "Loading…" : "Preview"}
      </button>
    );
  }

  function renderRelationshipAction(file: IndexedFile) {
    const isThisContext = relationshipContextLoadingDocumentId === file.documentId;

    return (
      <button
        className="file-action file-action-related"
        type="button"
        onClick={() => void loadRelationshipContext(file)}
        disabled={relationshipContextLoadingDocumentId !== null}
        title="View, add, or remove explicit document relationships without leaving the current context"
      >
        {isThisContext ? "Loading links…" : "Related"}
      </button>
    );
  }

  function renderHistoryAction(file: IndexedFile) {
    const isThisHistory = versionHistoryLoadingDocumentId === file.documentId;

    return (
      <button
        className="file-action file-action-history"
        type="button"
        onClick={() => void showVersionHistory(file)}
        disabled={versionHistoryLoadingDocumentId !== null}
        title="View persisted source observations and immutable controlled evidence versions"
      >
        {isThisHistory ? "Loading history…" : "History"}
      </button>
    );
  }

  function renderCaptureAction(file: IndexedFile) {
    const isThisCapture = capturingFileInstanceId === file.fileInstanceId;
    const isThisReconcile = reconcilingFileInstanceId === file.fileInstanceId;
    const isThisRelink = relinkingFileInstanceId === file.fileInstanceId;
    const captureUnavailable =
      file.availabilityState !== "AVAILABLE" || capturingFileInstanceId !== null;

    return (
      <>
        {sourceUnavailable(file.availabilityState) ? (
          <button
            className="file-action file-action-relink"
            type="button"
            onClick={() => void relinkLinkedSource(file)}
            disabled={
              relinkingFileInstanceId !== null ||
              reconcilingFileInstanceId !== null ||
              capturingFileInstanceId !== null
            }
            title="Select the current file from an approved available storage root; reconciliation will still be required"
          >
            {isThisRelink ? "Relinking…" : "Relink source"}
          </button>
        ) : null}
        {file.availabilityState === "CHANGED" ? (
          <button
            className="file-action file-action-reconcile"
            type="button"
            onClick={() => void reconcileLinkedSource(file)}
            disabled={
              reconcilingFileInstanceId !== null ||
              relinkingFileInstanceId !== null ||
              capturingFileInstanceId !== null
            }
            title="Accept the current indexed linked source without changing historical or controlled evidence"
          >
            {isThisReconcile ? "Reconciling…" : "Reconcile source"}
          </button>
        ) : null}
        <button
          className="file-action file-action-capture"
          type="button"
          onClick={() => void captureEvidence(file)}
          disabled={captureUnavailable}
          title={
            file.availabilityState === "AVAILABLE"
              ? "Preserve an immutable verified evidence copy"
              : file.availabilityState === "CHANGED"
                ? "Review and reconcile the changed linked source before evidence capture"
                : "The linked source must be available before evidence capture"
          }
        >
          {isThisCapture ? "Capturing…" : "Capture evidence"}
        </button>
      </>
    );
  }

  function showView(mode: ViewMode) {
    void navigateTo({
      viewMode: mode,
      query: "",
      storageRootId: null,
    });
  }

  async function reuseRecentSearch(search: RecentSearch) {
    await navigateTo({
      viewMode: "home",
      query: search.queryText,
      storageRootId: null,
    });

    window.requestAnimationFrame(() => {
      const input = document.getElementById("universal-search") as HTMLInputElement | null;
      input?.focus();
      input?.setSelectionRange(search.queryText.length, search.queryText.length);
    });
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
      if (!selected || sourceUnavailable(selected.availabilityState)) return;
      event.preventDefault();
      void openFileInstance(selected.fileInstanceId, query);
    }
  }

  const activeIsRunning =
    activeJob !== null && !TERMINAL_JOB_STATUSES.has(activeJob.status);
  const hasQuery = query.trim().length > 0;
  const canGoBack = backHistory.current.length > 0;
  const canGoForward = forwardHistory.current.length > 0;
  const pinnedDocumentIds = new Set(
    pinnedDocuments.map((document) => document.documentId),
  );
  const selectedClient =
    clients.find((client) => client.clientId === selectedClientId) ?? null;
  const selectedFirmLibraryItem =
    firmLibraryItems.find(
      (item) => item.firmLibraryItemId === selectedFirmLibraryItemId,
    ) ?? null;
  const visibleFirmLibraryItems = firmLibraryFilterCategory
    ? firmLibraryItems.filter((item) => item.category === firmLibraryFilterCategory)
    : firmLibraryItems;
  const selectedEngagement =
    engagements.find((engagement) => engagement.engagementId === selectedEngagementId) ??
    null;
  const selectedTemplateForUpdate =
    engagementTemplates.find(
      (template) => template.engagementTemplateId === templateUpdateId,
    ) ?? null;
  const compatibleMethodologyTemplates = selectedEngagement
    ? engagementTemplates.filter(
        (template) => template.serviceTypeId === selectedEngagement.serviceTypeId,
      )
    : [];
  const selectedLedgerImport =
    ledgerImports.find((item) => item.ledgerImportId === selectedLedgerImportId) ?? null;
  const ledgerControlledEvidenceVersions = ledgerEvidenceVersionHistory.filter(
    (entry) =>
      entry.controlledEvidenceVersionId &&
      entry.controlledVerificationState === "HASH_VERIFIED",
  );
  const selectedTrialBalanceImport =
    trialBalanceImports.find(
      (item) => item.trialBalanceImportId === selectedTrialBalanceImportId,
    ) ?? null;
  const selectedMappingLedgerImport =
    ledgerImports.find((item) => item.ledgerImportId === mappingLedgerImportId) ?? null;
  const compatibleMappingTrialBalanceImports = selectedMappingLedgerImport
    ? trialBalanceImports.filter(
        (item) => item.amountScale === selectedMappingLedgerImport.amountScale,
      )
    : [];
  const selectedMappingTrialBalanceImport =
    trialBalanceImports.find(
      (item) => item.trialBalanceImportId === mappingTrialBalanceImportId,
    ) ?? null;
  const selectedMappingLedgerAccount =
    ledgerAccountSummaries.find(
      (item) => item.accountKey === selectedMappingLedgerAccountKey,
    ) ?? null;
  const currentMappingForSelectedAccount =
    ledgerTbMappings.find(
      (item) => item.ledgerAccountKey === selectedMappingLedgerAccountKey,
    ) ?? null;
  const selectedScheduleMappingTrialBalanceImport =
    trialBalanceImports.find(
      (item) => item.trialBalanceImportId === scheduleMappingTrialBalanceImportId,
    ) ?? null;
  const selectedScheduleMappingTrialBalanceAccount =
    scheduleMappingTrialBalanceAccounts.find(
      (item) =>
        item.trialBalanceAccountId === selectedScheduleMappingTrialBalanceAccountId,
    ) ?? null;
  const currentScheduleMappingForSelectedAccount =
    trialBalanceScheduleMappings.find(
      (item) =>
        item.trialBalanceAccountId === selectedScheduleMappingTrialBalanceAccountId,
    ) ?? null;
  const trialBalanceControlledEvidenceVersions = trialBalanceEvidenceVersionHistory.filter(
    (entry) =>
      entry.controlledEvidenceVersionId &&
      entry.controlledVerificationState === "HASH_VERIFIED",
  );
  const selectedWorkpaper =
    workpapers.find((workpaper) => workpaper.workpaperId === selectedWorkpaperId) ??
    null;
  const visibleEngagements = selectedClientId
    ? engagements.filter((engagement) => engagement.clientId === selectedClientId)
    : engagements;
  const clientNameById = Object.fromEntries(
    clients.map((client) => [client.clientId, client.name]),
  );
  const serviceTypeNameById = Object.fromEntries(
    serviceTypes.map((serviceType) => [serviceType.serviceTypeId, serviceType.name]),
  );
  const areaNameById = Object.fromEntries(
    engagementAreas.map((area) => [area.engagementAreaId, area.name]),
  );
  const evidenceVersionOptions = workpaperEvidenceVersionOptions(evidenceVersionHistory);
  const pbcEvidenceVersionOptions =
    workpaperEvidenceVersionOptions(pbcEvidenceVersionHistory);
  const selectedPbcRequest =
    pbcRequests.find((request) => request.pbcRequestId === selectedPbcRequestId) ?? null;
  const selectedReviewNote =
    reviewNotes.find((note) => note.reviewNoteId === selectedReviewNoteId) ?? null;
  const revisionNumberById = Object.fromEntries(
    workpaperRevisions.map((revision) => [
      revision.workpaperRevisionId,
      revision.revisionNumber,
    ]),
  );
  const evidenceLinkNameById = Object.fromEntries(
    workpaperEvidenceLinks.map((link) => [
      link.evidenceLinkId,
      `${link.documentName} · ${link.relationshipType}`,
    ]),
  );
  const reviewLocationIncomplete =
    Boolean(newReviewLocationKind.trim()) !== Boolean(newReviewLocationValue.trim());
  const latestRevisionId = workpaperRevisions[0]?.workpaperRevisionId ?? null;
  const activeSignoffsForLatestRevision = workpaperSignoffs.filter(
    (signoff) =>
      signoff.workpaperRevisionId === latestRevisionId &&
      signoff.supersededAtMs === null,
  );
  const latestRevisionSigned = activeSignoffsForLatestRevision.length > 0;

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
          <button
            className={`nav-item${viewMode === "clients" ? " nav-item-active" : ""}`}
            type="button"
            onClick={() => showView("clients")}
          >
            Clients <span className="nav-count">{clients.length}</span>
          </button>
          <button
            className={`nav-item${viewMode === "engagements" ? " nav-item-active" : ""}`}
            type="button"
            onClick={() => showView("engagements")}
          >
            Engagements <span className="nav-count">{engagements.length}</span>
          </button>
          <button
            className={`nav-item${viewMode === "recent" ? " nav-item-active" : ""}`}
            type="button"
            onClick={() => showView("recent")}
          >
            Recent <span className="nav-count">{recentDocuments.length}</span>
          </button>
          <button
            className={`nav-item${viewMode === "searches" ? " nav-item-active" : ""}`}
            type="button"
            onClick={() => showView("searches")}
          >
            Searches <span className="nav-count">{recentSearches.length}</span>
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
          <div className="history-controls" aria-label="Navigation history">
            <button
              className="history-button"
              type="button"
              onClick={() => void navigateBack()}
              disabled={!canGoBack}
              aria-label="Go back"
              aria-keyshortcuts="Alt+ArrowLeft"
              title="Back (Alt+Left)"
            >
              ←
            </button>
            <button
              className="history-button"
              type="button"
              onClick={() => void navigateForward()}
              disabled={!canGoForward}
              aria-label="Go forward"
              aria-keyshortcuts="Alt+ArrowRight"
              title="Forward (Alt+Right)"
            >
              →
            </button>
          </div>
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

          {evidenceCaptureNotice ? (
            <div className="status-card evidence-success" role="status" aria-live="polite">
              <strong>
                Controlled evidence v{evidenceCaptureNotice.versionNumber} captured
              </strong>
              <span>{evidenceCaptureNotice.fileName}</span>
              <code title={evidenceCaptureNotice.sha256Hex}>
                SHA-256 {evidenceCaptureNotice.sha256Hex.slice(0, 16)}…
              </code>
            </div>
          ) : null}

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
                    : `${searchResults.length} result${searchResults.length === 1 ? "" : "s"}${searchElapsedMs === null ? "" : ` · ${searchElapsedMs} ms`}`}
                </h2>
              </div>
              <span>
                ↑↓ selects · Enter opens · Esc clears. Response time includes the 120 ms
                search-as-you-type debounce; the indexed-query engineering target is under 300 ms
                where feasible.
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
                      if (!sourceUnavailable(file.availabilityState)) {
                        void openFileInstance(file.fileInstanceId, query);
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
                        {stateLabel(file.availabilityState)}
                      </span>
                      <span className="file-size">{formatBytes(file.sizeBytes)}</span>
                      {renderPreviewAction(file, hasQuery ? query : undefined)}
                      {renderHistoryAction(file)}
                      {renderRelationshipAction(file)}
                      {renderCaptureAction(file)}
                      <button
                        className="file-action"
                        type="button"
                        onClick={() => void openFileInstance(file.fileInstanceId, query)}
                        disabled={sourceUnavailable(file.availabilityState)}
                      >
                        Open
                      </button>
                      <button
                        className="file-action"
                        type="button"
                        onClick={() => void revealFileInstance(file.fileInstanceId)}
                        disabled={sourceUnavailable(file.availabilityState)}
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

        {!hasQuery && !selectedRoot && viewMode === "clients" ? (
          <section className="results-panel professional-panel" aria-label="Clients">
            <div className="results-heading">
              <div>
                <p className="eyebrow">CLIENTS</p>
                <h2>Continuing client records</h2>
              </div>
              <span>
                Client identity is separate from engagements, periods, and storage locations.
              </span>
            </div>
            <div className="workspace-grid workspace-grid-two">
              <form className="workspace-card workspace-form" onSubmit={submitClient}>
                <div>
                  <span className="workspace-label">NEW CLIENT</span>
                  <h3>Create client</h3>
                </div>
                <label>
                  <span>Client name</span>
                  <input
                    value={newClientName}
                    onChange={(event) => setNewClientName(event.target.value)}
                    placeholder="Example Ltd"
                    maxLength={200}
                  />
                </label>
                <button
                  className="primary-button"
                  type="submit"
                  disabled={workspaceBusy || !newClientName.trim()}
                >
                  Create client
                </button>
              </form>

              <div className="workspace-card">
                <div className="workspace-card-heading">
                  <div>
                    <span className="workspace-label">CLIENT LIST</span>
                    <h3>{clients.length} active client{clients.length === 1 ? "" : "s"}</h3>
                  </div>
                </div>
                <div className="workspace-list">
                  {clients.length ? (
                    clients.map((client) => {
                      const engagementCount = engagements.filter(
                        (engagement) => engagement.clientId === client.clientId,
                      ).length;
                      return (
                        <button
                          className="workspace-list-row"
                          type="button"
                          key={client.clientId}
                          onClick={() => openClientEngagements(client.clientId)}
                        >
                          <span>
                            <strong>{client.name}</strong>
                            <small>
                              {engagementCount} engagement{engagementCount === 1 ? "" : "s"}
                            </small>
                          </span>
                          <span className="workspace-row-action">Open engagements →</span>
                        </button>
                      );
                    })
                  ) : (
                    <div className="empty-result">Create the first client to begin an engagement.</div>
                  )}
                </div>
              </div>
            </div>
          </section>
        ) : null}

        {!hasQuery && !selectedRoot && viewMode === "engagements" ? (
          <section className="results-panel professional-panel" aria-label="Engagements and workpapers">
            <div className="results-heading">
              <div>
                <p className="eyebrow">ENGAGEMENTS</p>
                <h2>
                  {selectedClient ? selectedClient.name : "Engagement workspace"}
                </h2>
              </div>
              <span>
                Service type and area hierarchy are configurable data, not fixed audit modules.
              </span>
            </div>

            <div className="workspace-toolbar">
              <label>
                <span>Client filter</span>
                <select
                  value={selectedClientId ?? ""}
                  onChange={(event) => {
                    const clientId = event.target.value || null;
                    setSelectedClientId(clientId);
                    setSelectedEngagementId(null);
                    setEngagementAreas([]);
                    setProcedures([]);
                    setWorkpapers([]);
                    setSelectedWorkpaperId(null);
                    setWorkpaperRevisions([]);
                    setWorkpaperEvidenceLinks([]);
                    setWorkpaperWorkflowEvents([]);
                    setReviewNotes([]);
                    setSelectedReviewNoteId(null);
                    setReviewNoteEvents([]);
                    setPbcRequests([]);
                    setSelectedPbcRequestId(null);
                    setPbcRequestEvents([]);
                    setPbcEvidenceLinks([]);
                    setPbcEvidenceSearchResults([]);
                    setSelectedPbcEvidenceDocument(null);
                    setPbcEvidenceVersionHistory([]);
                    setSelectedPbcEvidenceVersionKey("");
                  }}
                >
                  <option value="">All clients</option>
                  {clients.map((client) => (
                    <option key={client.clientId} value={client.clientId}>
                      {client.name}
                    </option>
                  ))}
                </select>
              </label>
              <form className="workspace-inline-form" onSubmit={submitServiceType}>
                <label>
                  <span>New service type</span>
                  <input
                    value={newServiceTypeName}
                    onChange={(event) => setNewServiceTypeName(event.target.value)}
                    placeholder="Internal Audit"
                    maxLength={120}
                  />
                </label>
                <button
                  className="secondary-button"
                  type="submit"
                  disabled={workspaceBusy || !newServiceTypeName.trim()}
                >
                  Add service type
                </button>
              </form>
            </div>

            <div className="workspace-grid workspace-grid-two">
              <form className="workspace-card workspace-form" onSubmit={submitFirmLibraryItem}>
                <div>
                  <span className="workspace-label">FIRM LIBRARY</span>
                  <h3>Create reusable methodology item</h3>
                </div>
                <div className="workspace-form-pair">
                  <label>
                    <span>Category</span>
                    <select
                      value={newFirmLibraryCategory}
                      onChange={(event) => setNewFirmLibraryCategory(event.target.value)}
                    >
                      {FIRM_LIBRARY_CATEGORIES.map((category) => (
                        <option key={category.key} value={category.key}>
                          {category.label}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label>
                    <span>Service scope</span>
                    <select
                      value={newFirmLibraryServiceTypeId}
                      onChange={(event) => setNewFirmLibraryServiceTypeId(event.target.value)}
                    >
                      <option value="">Firm-wide</option>
                      {serviceTypes.map((serviceType) => (
                        <option key={serviceType.serviceTypeId} value={serviceType.serviceTypeId}>
                          {serviceType.name}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
                <label>
                  <span>Name</span>
                  <input
                    value={newFirmLibraryName}
                    onChange={(event) => setNewFirmLibraryName(event.target.value)}
                    placeholder="Revenue completion checklist"
                    maxLength={200}
                  />
                </label>
                <label>
                  <span>Description</span>
                  <textarea
                    value={newFirmLibraryDescription}
                    onChange={(event) => setNewFirmLibraryDescription(event.target.value)}
                    rows={2}
                    maxLength={2000}
                    placeholder="When and how this firm methodology should be used."
                  />
                </label>
                <label>
                  <span>Methodology content</span>
                  <textarea
                    value={newFirmLibraryContent}
                    onChange={(event) => setNewFirmLibraryContent(event.target.value)}
                    rows={5}
                    maxLength={100000}
                    placeholder="Write the reusable checklist, query, risk/control guidance, test, report wording, or compliance requirement."
                  />
                </label>
                <p className="evidence-integrity-note">
                  Creation publishes immutable version 1. Later edits are new exact versions, never
                  silent rewrites.
                </p>
                <button
                  className="secondary-button"
                  type="submit"
                  disabled={
                    workspaceBusy ||
                    !newFirmLibraryName.trim() ||
                    !newFirmLibraryContent.trim()
                  }
                >
                  Publish library version 1
                </button>
              </form>

              <div className="workspace-card">
                <div className="workspace-card-heading">
                  <div>
                    <span className="workspace-label">REUSABLE LIBRARY</span>
                    <h3>{visibleFirmLibraryItems.length} item{visibleFirmLibraryItems.length === 1 ? "" : "s"}</h3>
                  </div>
                </div>
                <label>
                  <span>Category filter</span>
                  <select
                    value={firmLibraryFilterCategory}
                    onChange={(event) => setFirmLibraryFilterCategory(event.target.value)}
                  >
                    <option value="">All categories</option>
                    {FIRM_LIBRARY_CATEGORIES.map((category) => (
                      <option key={category.key} value={category.key}>
                        {category.label}
                      </option>
                    ))}
                  </select>
                </label>
                <div className="workspace-list">
                  {visibleFirmLibraryItems.length ? (
                    visibleFirmLibraryItems.map((item) => (
                      <button
                        className={`workspace-list-row${selectedFirmLibraryItemId === item.firmLibraryItemId ? " workspace-list-row-active" : ""}`}
                        type="button"
                        key={item.firmLibraryItemId}
                        onClick={() => void loadFirmLibraryItem(item.firmLibraryItemId)}
                      >
                        <span>
                          <strong>{item.name}</strong>
                          <small>
                            {firmLibraryCategoryLabel(item.category)} · v
                            {item.latestVersionNumber} ·{" "}
                            {item.serviceTypeId
                              ? serviceTypeNameById[item.serviceTypeId] ?? "Service"
                              : "Firm-wide"}
                          </small>
                        </span>
                        <span className="workspace-row-action">Open →</span>
                      </button>
                    ))
                  ) : (
                    <div className="empty-result">
                      No firm methodology item matches this category.
                    </div>
                  )}
                </div>
              </div>
            </div>

            {selectedFirmLibraryItem ? (
              <div className="workspace-grid workspace-grid-two">
                <form className="workspace-card workspace-form" onSubmit={submitFirmLibraryVersion}>
                  <div>
                    <span className="workspace-label">CONTROLLED UPDATE</span>
                    <h3>{selectedFirmLibraryItem.name}</h3>
                  </div>
                  <small>
                    {firmLibraryCategoryLabel(selectedFirmLibraryItem.category)} ·{" "}
                    {selectedFirmLibraryItem.serviceTypeId
                      ? serviceTypeNameById[selectedFirmLibraryItem.serviceTypeId] ?? "Service"
                      : "Firm-wide"}
                  </small>
                  <label>
                    <span>Next methodology content</span>
                    <textarea
                      value={firmLibraryDraftContent}
                      onChange={(event) => setFirmLibraryDraftContent(event.target.value)}
                      rows={6}
                      maxLength={100000}
                    />
                  </label>
                  <p className="evidence-integrity-note">
                    Publishing creates immutable version{" "}
                    {selectedFirmLibraryItem.latestVersionNumber + 1}. Version{" "}
                    {selectedFirmLibraryItem.latestVersionNumber} and its hash remain preserved.
                  </p>
                  <button
                    className="secondary-button"
                    type="submit"
                    disabled={workspaceBusy || !firmLibraryDraftContent.trim()}
                  >
                    Publish version {selectedFirmLibraryItem.latestVersionNumber + 1}
                  </button>
                </form>

                <div className="workspace-card">
                  <div className="workspace-card-heading">
                    <div>
                      <span className="workspace-label">VERSION HISTORY</span>
                      <h3>{firmLibraryVersions.length} exact version{firmLibraryVersions.length === 1 ? "" : "s"}</h3>
                    </div>
                  </div>
                  <div className="workspace-mini-list">
                    {firmLibraryVersions.map((version) => (
                      <span key={version.firmLibraryVersionId}>
                        <strong>Version {version.versionNumber}</strong>
                        <small>
                          SHA-256 {version.definitionHashHex.slice(0, 16)}… ·{" "}
                          {new Date(version.createdAtMs).toLocaleString()}
                        </small>
                      </span>
                    ))}
                  </div>
                </div>
              </div>
            ) : null}

            <div className="workspace-grid workspace-grid-two">
              <form className="workspace-card workspace-form" onSubmit={submitEngagement}>
                <div>
                  <span className="workspace-label">NEW ENGAGEMENT</span>
                  <h3>Create assignment</h3>
                </div>
                <label>
                  <span>Client</span>
                  <select
                    value={selectedClientId ?? ""}
                    onChange={(event) => setSelectedClientId(event.target.value || null)}
                  >
                    <option value="">Select client</option>
                    {clients.map((client) => (
                      <option key={client.clientId} value={client.clientId}>
                        {client.name}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  <span>Firm template</span>
                  <select
                    value={newEngagementTemplateVersionId}
                    onChange={(event) => {
                      const versionId = event.target.value;
                      setNewEngagementTemplateVersionId(versionId);
                      const template = engagementTemplates.find(
                        (entry) => entry.latestVersionId === versionId,
                      );
                      if (template) {
                        setNewEngagementServiceTypeId(template.serviceTypeId);
                      }
                    }}
                  >
                    <option value="">Blank engagement</option>
                    {engagementTemplates.map((template) => (
                      <option
                        key={template.engagementTemplateId}
                        value={template.latestVersionId}
                      >
                        {template.name} · v{template.latestVersionNumber}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  <span>Service type</span>
                  <select
                    value={newEngagementServiceTypeId}
                    onChange={(event) => setNewEngagementServiceTypeId(event.target.value)}
                    disabled={Boolean(newEngagementTemplateVersionId)}
                  >
                    <option value="">Select service type</option>
                    {serviceTypes.map((serviceType) => (
                      <option key={serviceType.serviceTypeId} value={serviceType.serviceTypeId}>
                        {serviceType.name}
                      </option>
                    ))}
                  </select>
                </label>
                {newEngagementTemplateVersionId ? (
                  <p className="evidence-integrity-note">
                    The selected exact template version is copied into the new engagement. Later
                    firm-methodology changes will not rewrite this engagement.
                  </p>
                ) : null}
                <label>
                  <span>Engagement name</span>
                  <input
                    value={newEngagementName}
                    onChange={(event) => setNewEngagementName(event.target.value)}
                    placeholder="Example Ltd — Review 2026"
                    maxLength={240}
                  />
                </label>
                <div className="workspace-form-pair">
                  <label>
                    <span>Period start</span>
                    <input
                      value={newEngagementPeriodStart}
                      onChange={(event) => setNewEngagementPeriodStart(event.target.value)}
                      placeholder="2026-04-01"
                    />
                  </label>
                  <label>
                    <span>Period end</span>
                    <input
                      value={newEngagementPeriodEnd}
                      onChange={(event) => setNewEngagementPeriodEnd(event.target.value)}
                      placeholder="2027-03-31"
                    />
                  </label>
                </div>
                <button
                  className="primary-button"
                  type="submit"
                  disabled={
                    workspaceBusy ||
                    !selectedClientId ||
                    (!newEngagementTemplateVersionId && !newEngagementServiceTypeId) ||
                    !newEngagementName.trim()
                  }
                >
                  {newEngagementTemplateVersionId
                    ? "Create from exact template"
                    : "Create engagement"}
                </button>
              </form>

              <div className="workspace-card">
                <div className="workspace-card-heading">
                  <div>
                    <span className="workspace-label">ENGAGEMENT LIST</span>
                    <h3>
                      {visibleEngagements.length} engagement
                      {visibleEngagements.length === 1 ? "" : "s"}
                    </h3>
                  </div>
                </div>
                <div className="workspace-list">
                  {visibleEngagements.length ? (
                    visibleEngagements.map((engagement) => (
                      <button
                        className={`workspace-list-row${selectedEngagementId === engagement.engagementId ? " workspace-list-row-active" : ""}`}
                        type="button"
                        key={engagement.engagementId}
                        onClick={() => void loadEngagementWorkspace(engagement.engagementId)}
                      >
                        <span>
                          <strong>{engagement.name}</strong>
                          <small>
                            {clientNameById[engagement.clientId] ?? "Unknown client"} ·{" "}
                            {serviceTypeNameById[engagement.serviceTypeId] ?? "Service"} ·{" "}
                            {engagement.status}
                          </small>
                          {engagement.periodStart || engagement.periodEnd ? (
                            <small>
                              {engagement.periodStart ?? "—"} → {engagement.periodEnd ?? "—"}
                            </small>
                          ) : null}
                        </span>
                        <span className="workspace-row-action">Open →</span>
                      </button>
                    ))
                  ) : (
                    <div className="empty-result">
                      {selectedClient
                        ? "No engagement exists for this client yet."
                        : "Create or select a client to begin."}
                    </div>
                  )}
                </div>
              </div>
            </div>

            {selectedEngagement ? (
              <div className="workspace-detail">
                <div className="workspace-detail-heading">
                  <div>
                    <span className="workspace-label">ACTIVE ENGAGEMENT</span>
                    <h3>{selectedEngagement.name}</h3>
                  </div>
                  <span>{workspaceBusy ? "Updating…" : "Ready"}</span>
                </div>

                <div className="workspace-grid workspace-grid-two">
                  <form className="workspace-card workspace-form" onSubmit={submitEngagementTemplate}>
                    <div>
                      <span className="workspace-label">FIRM METHODOLOGY</span>
                      <h3>
                        {selectedTemplateForUpdate
                          ? "Publish controlled methodology update"
                          : "Capture as reusable template"}
                      </h3>
                    </div>
                    <label>
                      <span>Publication target</span>
                      <select
                        value={templateUpdateId}
                        onChange={(event) => {
                          setTemplateUpdateId(event.target.value);
                          setNewTemplateName("");
                          setNewTemplateDescription("");
                        }}
                      >
                        <option value="">New firm template</option>
                        {compatibleMethodologyTemplates.map((template) => (
                          <option
                            key={template.engagementTemplateId}
                            value={template.engagementTemplateId}
                          >
                            {template.name} · publish after v{template.latestVersionNumber}
                          </option>
                        ))}
                      </select>
                    </label>
                    {selectedTemplateForUpdate ? (
                      <div className="evidence-integrity-note">
                        Publishing from this engagement creates immutable version{" "}
                        {selectedTemplateForUpdate.latestVersionNumber + 1} of{" "}
                        {selectedTemplateForUpdate.name}. Earlier versions and engagements created
                        from them remain unchanged.
                      </div>
                    ) : (
                      <>
                        <label>
                          <span>Template name</span>
                          <input
                            value={newTemplateName}
                            onChange={(event) => setNewTemplateName(event.target.value)}
                            placeholder="Core revenue methodology"
                            maxLength={200}
                          />
                        </label>
                        <label>
                          <span>Description</span>
                          <textarea
                            value={newTemplateDescription}
                            onChange={(event) => setNewTemplateDescription(event.target.value)}
                            rows={2}
                            placeholder="What this methodology template is intended to cover."
                          />
                        </label>
                        <p className="evidence-integrity-note">
                          Captures the current active areas, sub-areas, and procedures as immutable
                          version 1. Workpapers and client evidence are not copied into firm
                          methodology.
                        </p>
                      </>
                    )}
                    <button
                      className="secondary-button"
                      type="submit"
                      disabled={
                        workspaceBusy ||
                        (!selectedTemplateForUpdate && !newTemplateName.trim())
                      }
                    >
                      {selectedTemplateForUpdate
                        ? `Publish version ${selectedTemplateForUpdate.latestVersionNumber + 1}`
                        : "Save exact methodology version"}
                    </button>
                  </form>

                  <div className="workspace-card">
                    <div className="workspace-card-heading">
                      <div>
                        <span className="workspace-label">FIRM TEMPLATES</span>
                        <h3>{engagementTemplates.length} available</h3>
                      </div>
                    </div>
                    <div className="workspace-mini-list">
                      {engagementTemplates.length ? (
                        engagementTemplates.map((template) => (
                          <span key={template.engagementTemplateId}>
                            <strong>{template.name}</strong>
                            <small>
                              v{template.latestVersionNumber} ·{" "}
                              {serviceTypeNameById[template.serviceTypeId] ?? "Service"}
                            </small>
                          </span>
                        ))
                      ) : (
                        <div className="empty-result">
                          Capture a configured engagement to create the first firm template.
                        </div>
                      )}
                    </div>
                  </div>
                </div>

                <div className="workspace-grid workspace-grid-two">
                  <div className="workspace-card">
                    <div className="workspace-card-heading">
                      <div>
                        <span className="workspace-label">LEDGER SCRUTINY</span>
                        <h3>Import exact controlled workbook</h3>
                      </div>
                    </div>

                    <form className="workspace-form compact" onSubmit={searchLedgerEvidence}>
                      <label>
                        <span>Find workbook evidence</span>
                        <input
                          value={ledgerEvidenceSearchQuery}
                          onChange={(event) => setLedgerEvidenceSearchQuery(event.target.value)}
                          placeholder="Search indexed ledger workbook"
                        />
                      </label>
                      <button
                        className="secondary-button"
                        type="submit"
                        disabled={ledgerEvidenceSearchBusy || !ledgerEvidenceSearchQuery.trim()}
                      >
                        {ledgerEvidenceSearchBusy ? "Searching…" : "Search workbooks"}
                      </button>
                    </form>

                    {ledgerEvidenceSearchResults.length ? (
                      <div className="workspace-mini-list">
                        {ledgerEvidenceSearchResults.map((result) => (
                          <button
                            className={`workspace-list-row${selectedLedgerEvidenceDocument?.documentId === result.documentId ? " workspace-list-row-active" : ""}`}
                            type="button"
                            key={result.documentId}
                            onClick={() => void selectLedgerEvidenceDocument(result)}
                          >
                            <span>
                              <strong>{result.name}</strong>
                              <small>{result.path}</small>
                            </span>
                            <span className="workspace-row-action">Select →</span>
                          </button>
                        ))}
                      </div>
                    ) : null}

                    {selectedLedgerEvidenceDocument ? (
                      <form className="workspace-form compact" onSubmit={submitLedgerImport}>
                        <div className="evidence-integrity-note">
                          <strong>{selectedLedgerEvidenceDocument.name}</strong>
                          <br />
                          Ledger imports only use retained, hash-verified controlled evidence. The
                          original linked workbook path is never sent to the ledger parser.
                        </div>
                        <label>
                          <span>Exact controlled version</span>
                          <select
                            value={selectedLedgerControlledVersionId}
                            onChange={(event) =>
                              setSelectedLedgerControlledVersionId(event.target.value)
                            }
                          >
                            <option value="">Select controlled evidence</option>
                            {ledgerControlledEvidenceVersions.map((entry) => (
                              <option
                                key={entry.controlledEvidenceVersionId ?? entry.contentVersionId}
                                value={entry.controlledEvidenceVersionId ?? ""}
                              >
                                v{entry.controlledVersionNumber ?? "?"} · captured{" "}
                                {formatTimestamp(entry.capturedAtMs)}
                              </option>
                            ))}
                          </select>
                        </label>
                        {!ledgerControlledEvidenceVersions.length ? (
                          <p className="evidence-integrity-note">
                            This workbook has no hash-verified controlled evidence version yet.
                            Capture it as controlled evidence before ledger import.
                          </p>
                        ) : null}
                        <label>
                          <span>Worksheet</span>
                          <input
                            value={ledgerSheetName}
                            onChange={(event) => setLedgerSheetName(event.target.value)}
                            placeholder="Ledger"
                            maxLength={255}
                          />
                        </label>
                        <div className="workspace-grid workspace-grid-two">
                          <label>
                            <span>Header row</span>
                            <input
                              value={ledgerHeaderRowNumber}
                              onChange={(event) => setLedgerHeaderRowNumber(event.target.value)}
                              inputMode="numeric"
                              placeholder="1"
                            />
                          </label>
                          <label>
                            <span>Amount decimals</span>
                            <input
                              value={ledgerAmountScale}
                              onChange={(event) => setLedgerAmountScale(event.target.value)}
                              inputMode="numeric"
                              placeholder="2"
                            />
                          </label>
                        </div>
                        <div className="workspace-grid workspace-grid-two">
                          <label>
                            <span>Date column</span>
                            <input
                              value={ledgerDateColumn}
                              onChange={(event) => setLedgerDateColumn(event.target.value)}
                              placeholder="A"
                            />
                          </label>
                          <label>
                            <span>Account column</span>
                            <input
                              value={ledgerAccountColumn}
                              onChange={(event) => setLedgerAccountColumn(event.target.value)}
                              placeholder="B"
                            />
                          </label>
                          <label>
                            <span>Voucher column</span>
                            <input
                              value={ledgerVoucherColumn}
                              onChange={(event) => setLedgerVoucherColumn(event.target.value)}
                              placeholder="C"
                            />
                          </label>
                          <label>
                            <span>Narration column</span>
                            <input
                              value={ledgerNarrationColumn}
                              onChange={(event) => setLedgerNarrationColumn(event.target.value)}
                              placeholder="D"
                            />
                          </label>
                          <label>
                            <span>Amount column</span>
                            <input
                              value={ledgerAmountColumn}
                              onChange={(event) => setLedgerAmountColumn(event.target.value)}
                              placeholder="E"
                              required
                            />
                          </label>
                        </div>
                        <p className="evidence-integrity-note">
                          Column mapping accepts Excel letters (A, AA) or 1-based numbers. Imported
                          transactions become immutable and retain worksheet, source-row, row-hash,
                          controlled-version, and source-SHA provenance.
                        </p>
                        <button
                          className="secondary-button"
                          type="submit"
                          disabled={
                            workspaceBusy ||
                            !selectedLedgerControlledVersionId ||
                            !ledgerSheetName.trim() ||
                            !ledgerAmountColumn.trim()
                          }
                        >
                          Import immutable ledger
                        </button>
                      </form>
                    ) : null}
                  </div>

                  <div className="workspace-card">
                    <div className="workspace-card-heading">
                      <div>
                        <span className="workspace-label">DETERMINISTIC TEST</span>
                        <h3>High-value transactions</h3>
                      </div>
                    </div>

                    <form className="workspace-form compact" onSubmit={submitHighValueLedgerTest}>
                      <label>
                        <span>Imported ledger</span>
                        <select
                          value={selectedLedgerImportId ?? ""}
                          onChange={(event) =>
                            void selectLedgerImportForTesting(event.target.value || null)
                          }
                        >
                          <option value="">Select immutable import</option>
                          {ledgerImports.map((ledgerImport) => (
                            <option key={ledgerImport.ledgerImportId} value={ledgerImport.ledgerImportId}>
                              {ledgerImport.sheetName} · {ledgerImport.transactionCount.toLocaleString()} rows ·{" "}
                              {formatTimestamp(ledgerImport.importedAtMs)}
                            </option>
                          ))}
                        </select>
                      </label>
                      {selectedLedgerImport ? (
                        <div className="evidence-integrity-note">
                          Source SHA {selectedLedgerImport.sourceSha256Hex.slice(0, 20)}… ·
                          controlled ID{" "}
                          {selectedLedgerImport.controlledEvidenceVersionId.slice(0, 18)}… · sheet{" "}
                          {selectedLedgerImport.sheetName}
                        </div>
                      ) : null}
                      <label>
                        <span>High-value threshold</span>
                        <input
                          value={ledgerHighValueThreshold}
                          onChange={(event) => setLedgerHighValueThreshold(event.target.value)}
                          inputMode="decimal"
                          placeholder="100000.00"
                        />
                      </label>
                      <p className="evidence-integrity-note">
                        The test is inclusive and deterministic: amounts greater than or equal to
                        the threshold, including equally large negative amounts, are exceptions.
                      </p>
                      <button
                        className="secondary-button"
                        type="submit"
                        disabled={
                          workspaceBusy ||
                          !selectedLedgerImport ||
                          !ledgerHighValueThreshold.trim()
                        }
                      >
                        Run high-value test
                      </button>
                      {ledgerTestRuns.length ? (
                        <label>
                          <span>Completed runs</span>
                          <select
                            value={ledgerTestRun?.ledgerTestRunId ?? ""}
                            onChange={(event) => void openLedgerTestRun(event.target.value)}
                            disabled={workspaceBusy}
                          >
                            <option value="">Select completed run</option>
                            {ledgerTestRuns.map((run) => (
                              <option key={run.ledgerTestRunId} value={run.ledgerTestRunId}>
                                {formatTimestamp(run.ranAtMs)} · {run.exceptionCount} exception(s) ·{" "}
                                threshold{" "}
                                {selectedLedgerImport
                                  ? formatMinorUnitAmount(
                                      run.thresholdMinor,
                                      selectedLedgerImport.amountScale,
                                    )
                                  : run.thresholdMinor}
                              </option>
                            ))}
                          </select>
                        </label>
                      ) : null}
                    </form>

                    {ledgerTestRun ? (
                      <div className="workspace-mini-list">
                        <span>
                          <strong>{ledgerTestRun.exceptionCount} exception(s)</strong>
                          <small>
                            Threshold{" "}
                            {selectedLedgerImport
                              ? formatMinorUnitAmount(
                                  ledgerTestRun.thresholdMinor,
                                  selectedLedgerImport.amountScale,
                                )
                              : ledgerTestRun.thresholdMinor}{" "}
                            · {formatTimestamp(ledgerTestRun.ranAtMs)}
                          </small>
                        </span>
                        {ledgerExceptions.map((exception) => (
                          <span key={exception.ledgerExceptionId}>
                            <strong>
                              Row {exception.sourceRowNumber} ·{" "}
                              {selectedLedgerImport
                                ? formatMinorUnitAmount(
                                    exception.amountMinor,
                                    selectedLedgerImport.amountScale,
                                  )
                                : exception.amountMinor}
                            </strong>
                            <small>
                              {exception.transactionDateText ?? "No date"} ·{" "}
                              {exception.accountText ?? "No account"} ·{" "}
                              {exception.voucherText ?? "No voucher"}
                            </small>
                            <small>{exception.narrationText ?? "No narration"}</small>
                            <small>
                              {exception.sheetName}!row {exception.sourceRowNumber} · row SHA{" "}
                              {exception.sourceRowHashHex.slice(0, 16)}… · source SHA{" "}
                              {exception.sourceSha256Hex.slice(0, 16)}…
                            </small>
                          </span>
                        ))}
                      </div>
                    ) : ledgerImports.length ? (
                      <p className="evidence-integrity-note">
                        Select an immutable import and run the deterministic test to inspect
                        transaction-level exceptions.
                      </p>
                    ) : (
                      <div className="empty-result">
                        Import a controlled ledger workbook to begin deterministic scrutiny.
                      </div>
                    )}
                  </div>
                </div>

                <div className="workspace-grid workspace-grid-two">
                  <div className="workspace-card">
                    <div className="workspace-card-heading">
                      <div>
                        <span className="workspace-label">TRIAL BALANCE</span>
                        <h3>Import exact controlled workbook</h3>
                      </div>
                    </div>

                    <form className="workspace-form compact" onSubmit={searchTrialBalanceEvidence}>
                      <label>
                        <span>Find TB workbook evidence</span>
                        <input
                          value={trialBalanceEvidenceSearchQuery}
                          onChange={(event) => setTrialBalanceEvidenceSearchQuery(event.target.value)}
                          placeholder="Search indexed Trial Balance workbook"
                        />
                      </label>
                      <button
                        className="secondary-button"
                        type="submit"
                        disabled={
                          trialBalanceEvidenceSearchBusy ||
                          !trialBalanceEvidenceSearchQuery.trim()
                        }
                      >
                        {trialBalanceEvidenceSearchBusy ? "Searching…" : "Search workbooks"}
                      </button>
                    </form>

                    {trialBalanceEvidenceSearchResults.length ? (
                      <div className="workspace-mini-list">
                        {trialBalanceEvidenceSearchResults.map((result) => (
                          <button
                            className={`workspace-list-row${selectedTrialBalanceEvidenceDocument?.documentId === result.documentId ? " workspace-list-row-active" : ""}`}
                            type="button"
                            key={result.documentId}
                            onClick={() => void selectTrialBalanceEvidenceDocument(result)}
                          >
                            <span>
                              <strong>{result.name}</strong>
                              <small>{result.path}</small>
                            </span>
                            <span className="workspace-row-action">Select →</span>
                          </button>
                        ))}
                      </div>
                    ) : null}

                    {selectedTrialBalanceEvidenceDocument ? (
                      <form className="workspace-form compact" onSubmit={submitTrialBalanceImport}>
                        <div className="evidence-integrity-note">
                          <strong>{selectedTrialBalanceEvidenceDocument.name}</strong>
                          <br />
                          Trial Balance imports read only retained, hash-verified controlled
                          evidence. Opening/closing totals are computed in Rust from immutable
                          imported account rows.
                        </div>
                        <label>
                          <span>Exact controlled version</span>
                          <select
                            value={selectedTrialBalanceControlledVersionId}
                            onChange={(event) =>
                              setSelectedTrialBalanceControlledVersionId(event.target.value)
                            }
                          >
                            <option value="">Select controlled evidence</option>
                            {trialBalanceControlledEvidenceVersions.map((entry) => (
                              <option
                                key={entry.controlledEvidenceVersionId ?? entry.contentVersionId}
                                value={entry.controlledEvidenceVersionId ?? ""}
                              >
                                v{entry.controlledVersionNumber ?? "?"} · captured{" "}
                                {formatTimestamp(entry.capturedAtMs)}
                              </option>
                            ))}
                          </select>
                        </label>
                        {!trialBalanceControlledEvidenceVersions.length ? (
                          <p className="evidence-integrity-note">
                            This workbook has no hash-verified controlled evidence version yet.
                            Capture it as controlled evidence before TB import.
                          </p>
                        ) : null}
                        <label>
                          <span>Worksheet</span>
                          <input
                            value={trialBalanceSheetName}
                            onChange={(event) => setTrialBalanceSheetName(event.target.value)}
                            placeholder="Trial Balance"
                            maxLength={255}
                          />
                        </label>
                        <div className="workspace-grid workspace-grid-two">
                          <label>
                            <span>Header row</span>
                            <input
                              value={trialBalanceHeaderRowNumber}
                              onChange={(event) => setTrialBalanceHeaderRowNumber(event.target.value)}
                              inputMode="numeric"
                              placeholder="1"
                            />
                          </label>
                          <label>
                            <span>Amount decimals</span>
                            <input
                              value={trialBalanceAmountScale}
                              onChange={(event) => setTrialBalanceAmountScale(event.target.value)}
                              inputMode="numeric"
                              placeholder="2"
                            />
                          </label>
                          <label>
                            <span>Account code column</span>
                            <input
                              value={trialBalanceAccountCodeColumn}
                              onChange={(event) => setTrialBalanceAccountCodeColumn(event.target.value)}
                              placeholder="A (optional)"
                            />
                          </label>
                          <label>
                            <span>Account name column</span>
                            <input
                              value={trialBalanceAccountNameColumn}
                              onChange={(event) => setTrialBalanceAccountNameColumn(event.target.value)}
                              placeholder="B"
                              required
                            />
                          </label>
                          <label>
                            <span>Opening balance column</span>
                            <input
                              value={trialBalanceOpeningColumn}
                              onChange={(event) => setTrialBalanceOpeningColumn(event.target.value)}
                              placeholder="C (optional)"
                            />
                          </label>
                          <label>
                            <span>Closing balance column</span>
                            <input
                              value={trialBalanceClosingColumn}
                              onChange={(event) => setTrialBalanceClosingColumn(event.target.value)}
                              placeholder="D"
                              required
                            />
                          </label>
                        </div>
                        <p className="evidence-integrity-note">
                          Balances are signed fixed minor units. Blank opening/closing cells become
                          zero; account rows retain worksheet, source-row and row-hash provenance.
                        </p>
                        <button
                          className="secondary-button"
                          type="submit"
                          disabled={
                            workspaceBusy ||
                            !selectedTrialBalanceControlledVersionId ||
                            !trialBalanceSheetName.trim() ||
                            !trialBalanceAccountNameColumn.trim() ||
                            !trialBalanceClosingColumn.trim()
                          }
                        >
                          Import immutable Trial Balance
                        </button>
                      </form>
                    ) : null}
                  </div>

                  <div className="workspace-card">
                    <div className="workspace-card-heading">
                      <div>
                        <span className="workspace-label">TB REVIEW</span>
                        <h3>Imported balances & provenance</h3>
                      </div>
                    </div>

                    <label className="workspace-form compact">
                      <span>Imported Trial Balance</span>
                      <select
                        value={selectedTrialBalanceImportId ?? ""}
                        onChange={(event) =>
                          void selectTrialBalanceImport(event.target.value || null)
                        }
                      >
                        <option value="">Select immutable import</option>
                        {trialBalanceImports.map((trialBalanceImport) => (
                          <option
                            key={trialBalanceImport.trialBalanceImportId}
                            value={trialBalanceImport.trialBalanceImportId}
                          >
                            {trialBalanceImport.sheetName} ·{" "}
                            {trialBalanceImport.accountCount.toLocaleString()} accounts ·{" "}
                            {formatTimestamp(trialBalanceImport.importedAtMs)}
                          </option>
                        ))}
                      </select>
                    </label>

                    {selectedTrialBalanceImport ? (
                      <>
                        <div className="workspace-mini-list">
                          <span>
                            <strong>
                              {selectedTrialBalanceImport.accountCount.toLocaleString()} accounts
                            </strong>
                            <small>
                              Opening{" "}
                              {formatMinorUnitAmount(
                                trialBalanceComparison?.openingTotalMinor ??
                                  selectedTrialBalanceImport.openingTotalMinor,
                                selectedTrialBalanceImport.amountScale,
                              )}{" "}
                              · Closing{" "}
                              {formatMinorUnitAmount(
                                trialBalanceComparison?.closingTotalMinor ??
                                  selectedTrialBalanceImport.closingTotalMinor,
                                selectedTrialBalanceImport.amountScale,
                              )}{" "}
                              · Net movement{" "}
                              {trialBalanceComparison
                                ? formatMinorUnitAmount(
                                    trialBalanceComparison.netMovementMinor,
                                    selectedTrialBalanceImport.amountScale,
                                  )
                                : "Loading…"}
                            </small>
                            <small>
                              {selectedTrialBalanceImport.sheetName} · source SHA{" "}
                              {selectedTrialBalanceImport.sourceSha256Hex.slice(0, 16)}… ·
                              controlled ID{" "}
                              {selectedTrialBalanceImport.controlledEvidenceVersionId.slice(0, 18)}…
                            </small>
                          </span>
                          {trialBalanceComparison?.movements.slice(0, 100).map((account) => (
                            <span key={account.trialBalanceAccountId}>
                              <strong>
                                {account.accountCodeText ? `${account.accountCodeText} · ` : ""}
                                {account.accountNameText}
                              </strong>
                              <small>
                                Movement{" "}
                                {formatMinorUnitAmount(
                                  account.movementMinor,
                                  selectedTrialBalanceImport.amountScale,
                                )}{" "}
                                · Opening{" "}
                                {formatMinorUnitAmount(
                                  account.openingMinor,
                                  selectedTrialBalanceImport.amountScale,
                                )}{" "}
                                · Closing{" "}
                                {formatMinorUnitAmount(
                                  account.closingMinor,
                                  selectedTrialBalanceImport.amountScale,
                                )}
                              </small>
                              <small>
                                {selectedTrialBalanceImport.sheetName}!row{" "}
                                {account.sourceRowNumber} · row SHA{" "}
                                {account.sourceRowHashHex.slice(0, 16)}…
                              </small>
                            </span>
                          ))}
                        </div>
                        {trialBalanceComparison && trialBalanceComparison.movements.length > 100 ? (
                          <p className="evidence-integrity-note">
                            Showing the 100 largest absolute movements of{" "}
                            {trialBalanceComparison.movements.length.toLocaleString()} imported
                            accounts. The backend verifies that account movements reconcile to the
                            immutable opening and closing totals.
                          </p>
                        ) : trialBalanceComparison ? (
                          <p className="evidence-integrity-note">
                            Movements are sorted by absolute size and reconcile to the immutable
                            opening and closing totals in Rust.
                          </p>
                        ) : null}
                      </>
                    ) : trialBalanceImports.length ? (
                      <p className="evidence-integrity-note">
                        Select an immutable Trial Balance import to inspect backend-computed totals
                        and source-row provenance.
                      </p>
                    ) : (
                      <div className="empty-result">
                        Import a controlled Trial Balance workbook to establish accounting linkage.
                      </div>
                    )}
                  </div>
                </div>

                <div className="workspace-grid workspace-grid-two">
                  <div className="workspace-card">
                    <div className="workspace-card-heading">
                      <div>
                        <span className="workspace-label">ACCOUNT LINKAGE</span>
                        <h3>Ledger → Trial Balance mapping</h3>
                      </div>
                    </div>

                    <form className="workspace-form compact" onSubmit={submitLedgerTbMapping}>
                      <label>
                        <span>Immutable ledger import</span>
                        <select
                          value={mappingLedgerImportId ?? ""}
                          onChange={(event) =>
                            void selectMappingLedgerImport(event.target.value || null)
                          }
                        >
                          <option value="">Select ledger import</option>
                          {ledgerImports.map((ledgerImport) => (
                            <option
                              key={ledgerImport.ledgerImportId}
                              value={ledgerImport.ledgerImportId}
                            >
                              {ledgerImport.sheetName} ·{" "}
                              {ledgerImport.transactionCount.toLocaleString()} rows ·{" "}
                              {formatTimestamp(ledgerImport.importedAtMs)}
                            </option>
                          ))}
                        </select>
                      </label>
                      <label>
                        <span>Compatible Trial Balance import</span>
                        <select
                          value={mappingTrialBalanceImportId ?? ""}
                          onChange={(event) =>
                            void selectMappingTrialBalanceImport(event.target.value || null)
                          }
                          disabled={!selectedMappingLedgerImport}
                        >
                          <option value="">Select Trial Balance import</option>
                          {compatibleMappingTrialBalanceImports.map((trialBalanceImport) => (
                            <option
                              key={trialBalanceImport.trialBalanceImportId}
                              value={trialBalanceImport.trialBalanceImportId}
                            >
                              {trialBalanceImport.sheetName} ·{" "}
                              {trialBalanceImport.accountCount.toLocaleString()} accounts ·{" "}
                              {formatTimestamp(trialBalanceImport.importedAtMs)}
                            </option>
                          ))}
                        </select>
                      </label>
                      {selectedMappingLedgerImport &&
                      !compatibleMappingTrialBalanceImports.length ? (
                        <p className="evidence-integrity-note">
                          No Trial Balance import uses the same amount scale as this ledger import.
                          Import a compatible TB before mapping accounts.
                        </p>
                      ) : null}
                      <label>
                        <span>Ledger account</span>
                        <select
                          value={selectedMappingLedgerAccountKey}
                          onChange={(event) => selectMappingLedgerAccount(event.target.value)}
                          disabled={!mappingTrialBalanceImportId}
                        >
                          <option value="">Select ledger account</option>
                          {ledgerAccountSummaries.map((summary) => (
                            <option key={summary.accountKey} value={summary.accountKey}>
                              {summary.accountText} · {summary.transactionCount.toLocaleString()} tx
                            </option>
                          ))}
                        </select>
                      </label>
                      <label>
                        <span>Trial Balance account</span>
                        <select
                          value={selectedMappingTrialBalanceAccountId}
                          onChange={(event) =>
                            setSelectedMappingTrialBalanceAccountId(event.target.value)
                          }
                          disabled={!mappingTrialBalanceImportId}
                        >
                          <option value="">Select TB account</option>
                          {mappingTrialBalanceAccounts.map((account) => (
                            <option
                              key={account.trialBalanceAccountId}
                              value={account.trialBalanceAccountId}
                            >
                              {account.accountCodeText
                                ? account.accountCodeText + " · "
                                : ""}
                              {account.accountNameText}
                            </option>
                          ))}
                        </select>
                      </label>
                      {selectedMappingLedgerAccount && selectedMappingLedgerImport ? (
                        <p className="evidence-integrity-note">
                          Ledger total{" "}
                          {formatMinorUnitAmount(
                            selectedMappingLedgerAccount.totalMinor,
                            selectedMappingLedgerImport.amountScale,
                          )}{" "}
                          across {selectedMappingLedgerAccount.transactionCount.toLocaleString()}{" "}
                          transaction(s), source rows{" "}
                          {selectedMappingLedgerAccount.firstSourceRowNumber}–
                          {selectedMappingLedgerAccount.lastSourceRowNumber}.
                          {currentMappingForSelectedAccount
                            ? " Current mapping v" +
                              currentMappingForSelectedAccount.versionNumber +
                              ": " +
                              currentMappingForSelectedAccount.trialBalanceAccountNameText +
                              "."
                            : " Not yet mapped."}
                        </p>
                      ) : null}
                      <button
                        className="secondary-button"
                        type="submit"
                        disabled={
                          workspaceBusy ||
                          !mappingLedgerImportId ||
                          !mappingTrialBalanceImportId ||
                          !selectedMappingLedgerAccountKey ||
                          !selectedMappingTrialBalanceAccountId ||
                          currentMappingForSelectedAccount?.trialBalanceAccountId ===
                            selectedMappingTrialBalanceAccountId
                        }
                      >
                        {currentMappingForSelectedAccount?.trialBalanceAccountId ===
                        selectedMappingTrialBalanceAccountId
                          ? "Mapped"
                          : currentMappingForSelectedAccount
                            ? "Remap account"
                            : "Map account"}
                      </button>
                    </form>
                  </div>

                  <div className="workspace-card">
                    <div className="workspace-card-heading">
                      <div>
                        <span className="workspace-label">CURRENT MAP</span>
                        <h3>{ledgerTbMappings.length} mapped account(s)</h3>
                      </div>
                    </div>
                    <div className="workspace-mini-list">
                      {ledgerTbMappings.length ? (
                        ledgerTbMappings.map((mapping) => (
                          <span key={mapping.ledgerTbMappingId}>
                            <strong>
                              {mapping.ledgerAccountText} →{" "}
                              {mapping.trialBalanceAccountCodeText
                                ? mapping.trialBalanceAccountCodeText + " · "
                                : ""}
                              {mapping.trialBalanceAccountNameText}
                            </strong>
                            <small>
                              Mapping v{mapping.versionNumber} · TB row{" "}
                              {mapping.trialBalanceSourceRowNumber} · row SHA{" "}
                              {mapping.trialBalanceSourceRowHashHex.slice(0, 16)}…
                            </small>
                            <small>
                              {mapping.supersedesMappingId
                                ? "Supersedes " +
                                  mapping.supersedesMappingId.slice(0, 18) +
                                  "… · "
                                : ""}
                              {formatTimestamp(mapping.mappedAtMs)}
                            </small>
                          </span>
                        ))
                      ) : mappingLedgerImportId && mappingTrialBalanceImportId ? (
                        <div className="empty-result">
                          No ledger accounts are mapped to this exact Trial Balance import yet.
                        </div>
                      ) : (
                        <div className="empty-result">
                          Select compatible immutable ledger and Trial Balance imports to map
                          accounts.
                        </div>
                      )}
                    </div>
                    {selectedMappingTrialBalanceImport ? (
                      <p className="evidence-integrity-note">
                        Mapping target: {selectedMappingTrialBalanceImport.sheetName} · source SHA{" "}
                        {selectedMappingTrialBalanceImport.sourceSha256Hex.slice(0, 16)}… ·
                        controlled ID{" "}
                        {selectedMappingTrialBalanceImport.controlledEvidenceVersionId.slice(0, 18)}…
                      </p>
                    ) : null}
                  </div>
                </div>

                <div className="workspace-grid workspace-grid-two">
                  <div className="workspace-card">
                    <div className="workspace-card-heading">
                      <div>
                        <span className="workspace-label">FS SCHEDULES</span>
                        <h3>{financialStatementSchedules.length} immutable schedule(s)</h3>
                      </div>
                    </div>
                    <form
                      className="workspace-form compact"
                      onSubmit={submitFinancialStatementSchedule}
                    >
                      <label>
                        <span>Schedule reference</span>
                        <input
                          value={newFinancialStatementScheduleReference}
                          onChange={(event) =>
                            setNewFinancialStatementScheduleReference(event.target.value)
                          }
                          placeholder="SCH-REV"
                          maxLength={80}
                        />
                      </label>
                      <label>
                        <span>Schedule name</span>
                        <input
                          value={newFinancialStatementScheduleName}
                          onChange={(event) =>
                            setNewFinancialStatementScheduleName(event.target.value)
                          }
                          placeholder="Revenue"
                          maxLength={240}
                        />
                      </label>
                      <button
                        className="secondary-button"
                        type="submit"
                        disabled={
                          workspaceBusy ||
                          !newFinancialStatementScheduleReference.trim() ||
                          !newFinancialStatementScheduleName.trim()
                        }
                      >
                        Add immutable schedule
                      </button>
                    </form>
                    <div className="workspace-mini-list">
                      {financialStatementSchedules.length ? (
                        financialStatementSchedules.map((schedule) => (
                          <span key={schedule.financialStatementScheduleId}>
                            <strong>
                              {schedule.reference} · {schedule.name}
                            </strong>
                            <small>{formatTimestamp(schedule.createdAtMs)}</small>
                          </span>
                        ))
                      ) : (
                        <div className="empty-result">
                          Create the first engagement-level financial statement schedule.
                        </div>
                      )}
                    </div>
                  </div>

                  <div className="workspace-card">
                    <div className="workspace-card-heading">
                      <div>
                        <span className="workspace-label">SCHEDULE MAPPING</span>
                        <h3>Trial Balance → schedule</h3>
                      </div>
                    </div>
                    <form
                      className="workspace-form compact"
                      onSubmit={submitTrialBalanceScheduleMapping}
                    >
                      <label>
                        <span>Immutable Trial Balance import</span>
                        <select
                          value={scheduleMappingTrialBalanceImportId ?? ""}
                          onChange={(event) =>
                            void selectScheduleMappingTrialBalanceImport(
                              event.target.value || null,
                            )
                          }
                        >
                          <option value="">Select Trial Balance import</option>
                          {trialBalanceImports.map((trialBalanceImport) => (
                            <option
                              key={trialBalanceImport.trialBalanceImportId}
                              value={trialBalanceImport.trialBalanceImportId}
                            >
                              {trialBalanceImport.sheetName} ·{" "}
                              {trialBalanceImport.accountCount.toLocaleString()} accounts ·{" "}
                              {formatTimestamp(trialBalanceImport.importedAtMs)}
                            </option>
                          ))}
                        </select>
                      </label>
                      <label>
                        <span>Trial Balance account</span>
                        <select
                          value={selectedScheduleMappingTrialBalanceAccountId}
                          onChange={(event) =>
                            selectScheduleMappingTrialBalanceAccount(event.target.value)
                          }
                          disabled={!scheduleMappingTrialBalanceImportId}
                        >
                          <option value="">Select TB account</option>
                          {scheduleMappingTrialBalanceAccounts.map((account) => (
                            <option
                              key={account.trialBalanceAccountId}
                              value={account.trialBalanceAccountId}
                            >
                              {account.accountCodeText ? account.accountCodeText + " · " : ""}
                              {account.accountNameText}
                            </option>
                          ))}
                        </select>
                      </label>
                      <label>
                        <span>Financial statement schedule</span>
                        <select
                          value={selectedFinancialStatementScheduleId}
                          onChange={(event) =>
                            setSelectedFinancialStatementScheduleId(event.target.value)
                          }
                          disabled={!financialStatementSchedules.length}
                        >
                          <option value="">Select schedule</option>
                          {financialStatementSchedules.map((schedule) => (
                            <option
                              key={schedule.financialStatementScheduleId}
                              value={schedule.financialStatementScheduleId}
                            >
                              {schedule.reference} · {schedule.name}
                            </option>
                          ))}
                        </select>
                      </label>
                      {selectedScheduleMappingTrialBalanceAccount &&
                      selectedScheduleMappingTrialBalanceImport ? (
                        <p className="evidence-integrity-note">
                          {selectedScheduleMappingTrialBalanceImport.sheetName}!row{" "}
                          {selectedScheduleMappingTrialBalanceAccount.sourceRowNumber} · row SHA{" "}
                          {selectedScheduleMappingTrialBalanceAccount.sourceRowHashHex.slice(0, 16)}…
                          {currentScheduleMappingForSelectedAccount
                            ? " Current schedule mapping v" +
                              currentScheduleMappingForSelectedAccount.versionNumber +
                              ": " +
                              currentScheduleMappingForSelectedAccount.scheduleReference +
                              " · " +
                              currentScheduleMappingForSelectedAccount.scheduleName +
                              "."
                            : " Not yet mapped."}
                        </p>
                      ) : null}
                      <button
                        className="secondary-button"
                        type="submit"
                        disabled={
                          workspaceBusy ||
                          !scheduleMappingTrialBalanceImportId ||
                          !selectedScheduleMappingTrialBalanceAccountId ||
                          !selectedFinancialStatementScheduleId ||
                          currentScheduleMappingForSelectedAccount?.financialStatementScheduleId ===
                            selectedFinancialStatementScheduleId
                        }
                      >
                        {currentScheduleMappingForSelectedAccount?.financialStatementScheduleId ===
                        selectedFinancialStatementScheduleId
                          ? "Mapped"
                          : currentScheduleMappingForSelectedAccount
                            ? "Remap schedule"
                            : "Map to schedule"}
                      </button>
                    </form>

                    <div className="workspace-mini-list">
                      {trialBalanceScheduleMappings.length ? (
                        trialBalanceScheduleMappings.map((mapping) => (
                          <span key={mapping.trialBalanceScheduleMappingId}>
                            <strong>
                              {mapping.trialBalanceAccountCodeText
                                ? mapping.trialBalanceAccountCodeText + " · "
                                : ""}
                              {mapping.trialBalanceAccountNameText} →{" "}
                              {mapping.scheduleReference} · {mapping.scheduleName}
                            </strong>
                            <small>
                              Mapping v{mapping.versionNumber} · TB row{" "}
                              {mapping.trialBalanceSourceRowNumber} · row SHA{" "}
                              {mapping.trialBalanceSourceRowHashHex.slice(0, 16)}…
                            </small>
                            <small>
                              {mapping.supersedesMappingId
                                ? "Supersedes " +
                                  mapping.supersedesMappingId.slice(0, 18) +
                                  "… · "
                                : ""}
                              {formatTimestamp(mapping.mappedAtMs)}
                            </small>
                          </span>
                        ))
                      ) : scheduleMappingTrialBalanceImportId ? (
                        <div className="empty-result">
                          No Trial Balance accounts are mapped to financial statement schedules yet.
                        </div>
                      ) : (
                        <div className="empty-result">
                          Select an immutable Trial Balance import to map accounts to schedules.
                        </div>
                      )}
                    </div>
                  </div>
                </div>

                <div className="workspace-grid workspace-grid-three">
                  <div className="workspace-card">
                    <div className="workspace-card-heading">
                      <div>
                        <span className="workspace-label">AREAS</span>
                        <h3>{engagementAreas.length} configured</h3>
                      </div>
                    </div>
                    <form className="workspace-form compact" onSubmit={submitEngagementArea}>
                      <label>
                        <span>Parent area</span>
                        <select
                          value={newAreaParentId}
                          onChange={(event) => setNewAreaParentId(event.target.value)}
                        >
                          <option value="">Top level</option>
                          {engagementAreas.map((area) => (
                            <option key={area.engagementAreaId} value={area.engagementAreaId}>
                              {area.name}
                            </option>
                          ))}
                        </select>
                      </label>
                      <label>
                        <span>Area name</span>
                        <input
                          value={newAreaName}
                          onChange={(event) => setNewAreaName(event.target.value)}
                          placeholder="Revenue, GST, Procurement…"
                          maxLength={160}
                        />
                      </label>
                      <button
                        className="secondary-button"
                        type="submit"
                        disabled={workspaceBusy || !newAreaName.trim()}
                      >
                        Add area
                      </button>
                    </form>
                    <div className="workspace-mini-list">
                      {engagementAreas.map((area) => (
                        <span key={area.engagementAreaId}>
                          <strong>
                            {area.parentAreaId ? "↳ " : ""}
                            {area.name}
                          </strong>
                          <small>
                            {area.parentAreaId
                              ? `under ${areaNameById[area.parentAreaId] ?? "parent"}`
                              : "top level"}
                          </small>
                        </span>
                      ))}
                    </div>
                  </div>

                  <div className="workspace-card">
                    <div className="workspace-card-heading">
                      <div>
                        <span className="workspace-label">PROCEDURES</span>
                        <h3>{procedures.length} configured</h3>
                      </div>
                    </div>
                    <form className="workspace-form compact" onSubmit={submitProcedure}>
                      <label>
                        <span>Area</span>
                        <select
                          value={newProcedureAreaId}
                          onChange={(event) => setNewProcedureAreaId(event.target.value)}
                        >
                          <option value="">No area</option>
                          {engagementAreas.map((area) => (
                            <option key={area.engagementAreaId} value={area.engagementAreaId}>
                              {area.name}
                            </option>
                          ))}
                        </select>
                      </label>
                      <label>
                        <span>Procedure title</span>
                        <input
                          value={newProcedureTitle}
                          onChange={(event) => setNewProcedureTitle(event.target.value)}
                          placeholder="Inspect approval evidence"
                          maxLength={240}
                        />
                      </label>
                      <button
                        className="secondary-button"
                        type="submit"
                        disabled={workspaceBusy || !newProcedureTitle.trim()}
                      >
                        Add procedure
                      </button>
                    </form>
                    <div className="workspace-mini-list">
                      {procedures.map((procedure) => (
                        <span key={procedure.procedureId}>
                          <strong>{procedure.title}</strong>
                          <small>
                            {procedure.engagementAreaId
                              ? areaNameById[procedure.engagementAreaId] ?? "Area"
                              : "No area"}
                          </small>
                        </span>
                      ))}
                    </div>
                  </div>

                  <div className="workspace-card">
                    <div className="workspace-card-heading">
                      <div>
                        <span className="workspace-label">WORKPAPERS</span>
                        <h3>{workpapers.length} active</h3>
                      </div>
                    </div>
                    <form className="workspace-form compact" onSubmit={submitWorkpaper}>
                      <label>
                        <span>Area</span>
                        <select
                          value={newWorkpaperAreaId}
                          onChange={(event) => setNewWorkpaperAreaId(event.target.value)}
                        >
                          <option value="">No area</option>
                          {engagementAreas.map((area) => (
                            <option key={area.engagementAreaId} value={area.engagementAreaId}>
                              {area.name}
                            </option>
                          ))}
                        </select>
                      </label>
                      <div className="workspace-form-pair">
                        <label>
                          <span>Reference</span>
                          <input
                            value={newWorkpaperReference}
                            onChange={(event) => setNewWorkpaperReference(event.target.value)}
                            placeholder="REV-01"
                            maxLength={80}
                          />
                        </label>
                        <label>
                          <span>Title</span>
                          <input
                            value={newWorkpaperTitle}
                            onChange={(event) => setNewWorkpaperTitle(event.target.value)}
                            placeholder="Revenue testing"
                            maxLength={240}
                          />
                        </label>
                      </div>
                      <button
                        className="secondary-button"
                        type="submit"
                        disabled={
                          workspaceBusy ||
                          !newWorkpaperReference.trim() ||
                          !newWorkpaperTitle.trim()
                        }
                      >
                        Add workpaper
                      </button>
                    </form>
                    <div className="workspace-list compact-list">
                      {workpapers.map((workpaper) => (
                        <button
                          className={`workspace-list-row${selectedWorkpaperId === workpaper.workpaperId ? " workspace-list-row-active" : ""}`}
                          type="button"
                          key={workpaper.workpaperId}
                          onClick={() => void loadWorkpaperRevisions(workpaper.workpaperId)}
                        >
                          <span>
                            <strong>
                              {workpaper.reference} · {workpaper.title}
                            </strong>
                            <small>
                              {workpaper.workflowState.replaceAll("_", " ")} · Revision{" "}
                              {workpaper.latestRevisionNumber ?? "—"}
                            </small>
                          </span>
                        </button>
                      ))}
                    </div>
                  </div>
                </div>

                <div className="workspace-pbc-panel">
                  <div className="workspace-detail-heading">
                    <div>
                      <span className="workspace-label">PBC / CLIENT REQUESTS</span>
                      <h3>
                        {pbcRequests.length} request{pbcRequests.length === 1 ? "" : "s"}
                      </h3>
                    </div>
                    <span>Client-visible request content stays separate from internal notes.</span>
                  </div>

                  <div className="workspace-grid workspace-grid-two">
                    <form className="workspace-card workspace-form" onSubmit={submitPbcRequest}>
                      <div>
                        <span className="workspace-label">NEW REQUEST</span>
                        <h3>Create PBC item</h3>
                      </div>
                      <div className="workspace-form-pair">
                        <label>
                          <span>Request number</span>
                          <input
                            value={newPbcRequestNumber}
                            onChange={(event) => setNewPbcRequestNumber(event.target.value)}
                            placeholder="PBC-001"
                            maxLength={80}
                          />
                        </label>
                        <label>
                          <span>Area</span>
                          <select
                            value={newPbcAreaId}
                            onChange={(event) => setNewPbcAreaId(event.target.value)}
                          >
                            <option value="">No area</option>
                            {engagementAreas.map((area) => (
                              <option key={area.engagementAreaId} value={area.engagementAreaId}>
                                {area.name}
                              </option>
                            ))}
                          </select>
                        </label>
                      </div>
                      <label>
                        <span>Description</span>
                        <textarea
                          value={newPbcDescription}
                          onChange={(event) => setNewPbcDescription(event.target.value)}
                          rows={3}
                          placeholder="Describe what is requested."
                        />
                      </label>
                      <div className="workspace-form-pair">
                        <label>
                          <span>Requested from</span>
                          <input
                            value={newPbcRequestedFrom}
                            onChange={(event) => setNewPbcRequestedFrom(event.target.value)}
                            placeholder="Finance manager / client contact"
                            maxLength={200}
                          />
                        </label>
                        <label>
                          <span>Due</span>
                          <input
                            type="datetime-local"
                            value={newPbcDueLocal}
                            onChange={(event) => setNewPbcDueLocal(event.target.value)}
                          />
                        </label>
                      </div>
                      <label>
                        <span>Client-visible message</span>
                        <textarea
                          className="pbc-client-visible-input"
                          value={newPbcClientVisibleContent}
                          onChange={(event) =>
                            setNewPbcClientVisibleContent(event.target.value)
                          }
                          rows={3}
                          placeholder="Content intended to be shared with the client."
                        />
                      </label>
                      <label>
                        <span>Internal-only notes</span>
                        <textarea
                          className="pbc-internal-input"
                          value={newPbcInternalNotes}
                          onChange={(event) => setNewPbcInternalNotes(event.target.value)}
                          rows={3}
                          placeholder="Team-only context. Never client-visible."
                        />
                      </label>
                      <button
                        className="primary-button"
                        type="submit"
                        disabled={
                          workspaceBusy ||
                          !newPbcRequestNumber.trim() ||
                          !newPbcDescription.trim() ||
                          !newPbcRequestedFrom.trim()
                        }
                      >
                        Create request
                      </button>
                    </form>

                    <div className="workspace-card pbc-request-list">
                      <div className="workspace-card-heading">
                        <div>
                          <span className="workspace-label">REQUEST REGISTER</span>
                          <h3>Engagement PBC tracker</h3>
                        </div>
                      </div>
                      {pbcRequests.length ? (
                        pbcRequests.map((request) => (
                          <button
                            className={`pbc-request-row${
                              selectedPbcRequestId === request.pbcRequestId
                                ? " pbc-request-row-active"
                                : ""
                            }`}
                            type="button"
                            key={request.pbcRequestId}
                            onClick={() => void loadPbcRequestDetail(request.pbcRequestId)}
                          >
                            <span className="pbc-status-chip">
                              {request.status.replaceAll("_", " ")}
                            </span>
                            <strong>
                              {request.requestNumber} · {request.description}
                            </strong>
                            <small>
                              {request.engagementAreaId
                                ? areaNameById[request.engagementAreaId] ?? "Area"
                                : "No area"}{" "}
                              · Requested from {request.requestedFromParty}
                            </small>
                            <small>
                              {request.dueAtMs
                                ? `Due ${formatTimestamp(request.dueAtMs)}`
                                : "No due date"}
                              {request.latestAssessment ? " · Assessment recorded" : ""}
                            </small>
                          </button>
                        ))
                      ) : (
                        <div className="empty-result">
                          No PBC requests exist for this engagement yet.
                        </div>
                      )}
                    </div>
                  </div>

                  {selectedPbcRequest ? (
                    <div className="pbc-request-detail">
                      <div className="workspace-detail-heading">
                        <div>
                          <span className="workspace-label">SELECTED PBC REQUEST</span>
                          <h3>
                            {selectedPbcRequest.requestNumber} ·{" "}
                            {selectedPbcRequest.description}
                          </h3>
                        </div>
                        <span>{selectedPbcRequest.status.replaceAll("_", " ")}</span>
                      </div>

                      <div className="workspace-grid workspace-grid-two pbc-visibility-grid">
                        <div className="workspace-card pbc-client-visible-card">
                          <span className="workspace-label">CLIENT-VISIBLE CONTENT</span>
                          <p>
                            {selectedPbcRequest.clientVisibleContent ||
                              "No additional client-visible message was recorded."}
                          </p>
                          <small>
                            Requested from {selectedPbcRequest.requestedFromParty}
                            {selectedPbcRequest.dueAtMs
                              ? ` · Due ${formatTimestamp(selectedPbcRequest.dueAtMs)}`
                              : ""}
                          </small>
                        </div>
                        <div className="workspace-card pbc-internal-card">
                          <span className="workspace-label">INTERNAL ONLY</span>
                          <p>
                            {selectedPbcRequest.internalNotes ||
                              "No internal-only note was recorded."}
                          </p>
                          {selectedPbcRequest.latestAssessment ? (
                            <p>
                              <b>Latest assessment:</b>{" "}
                              {selectedPbcRequest.latestAssessment}
                            </p>
                          ) : null}
                        </div>
                      </div>

                      <div className="workspace-grid workspace-grid-two">
                        <div className="workspace-card pbc-action-stack">
                          <form className="workspace-form" onSubmit={submitPbcStatus}>
                            <div>
                              <span className="workspace-label">STATUS</span>
                              <h3>Record status change</h3>
                            </div>
                            <label>
                              <span>Next status</span>
                              <input
                                list="pbc-status-options"
                                value={pbcNextStatus}
                                onChange={(event) => setPbcNextStatus(event.target.value)}
                                placeholder="REQUESTED"
                                maxLength={80}
                              />
                              <datalist id="pbc-status-options">
                                <option value="DRAFT" />
                                <option value="REQUESTED" />
                                <option value="PARTIALLY_RECEIVED" />
                                <option value="RECEIVED" />
                                <option value="UNDER_REVIEW" />
                                <option value="FOLLOW_UP" />
                                <option value="ACCEPTED" />
                                <option value="CLOSED" />
                              </datalist>
                            </label>
                            <label>
                              <span>Actor identifier</span>
                              <input
                                value={pbcActorId}
                                onChange={(event) => setPbcActorId(event.target.value)}
                                placeholder="team.member@example.com"
                                maxLength={160}
                              />
                            </label>
                            <label>
                              <span>Status comment</span>
                              <textarea
                                value={pbcStatusComment}
                                onChange={(event) => setPbcStatusComment(event.target.value)}
                                rows={2}
                                placeholder="Optional reason or follow-up detail."
                              />
                            </label>
                            <button
                              className="secondary-button"
                              type="submit"
                              disabled={workspaceBusy || !pbcNextStatus.trim()}
                            >
                              Record status
                            </button>
                          </form>

                          <form className="workspace-form pbc-assessment-form" onSubmit={submitPbcAssessment}>
                            <div>
                              <span className="workspace-label">AUDITOR ASSESSMENT</span>
                              <h3>Add assessment</h3>
                            </div>
                            <label>
                              <span>Internal assessment</span>
                              <textarea
                                value={pbcAssessmentText}
                                onChange={(event) => setPbcAssessmentText(event.target.value)}
                                rows={3}
                                placeholder="Assess completeness, reliability, follow-up needed, or exceptions."
                              />
                            </label>
                            <button
                              className="secondary-button"
                              type="submit"
                              disabled={workspaceBusy || !pbcAssessmentText.trim()}
                            >
                              Record assessment
                            </button>
                          </form>
                        </div>

                        <div className="workspace-card pbc-event-list">
                          {pbcRequestEvents.length ? (
                            [...pbcRequestEvents].reverse().map((event) => (
                              <article key={event.pbcRequestEventId}>
                                <div>
                                  <strong>{event.eventType.replaceAll("_", " ")}</strong>
                                  <span>{formatTimestamp(event.occurredAtMs)}</span>
                                </div>
                                {event.fromStatus || event.toStatus ? (
                                  <p>
                                    {(event.fromStatus ?? "—").replaceAll("_", " ")} →{" "}
                                    {(event.toStatus ?? "—").replaceAll("_", " ")}
                                  </p>
                                ) : null}
                                {event.assessmentText ? (
                                  <p><b>Assessment:</b> {event.assessmentText}</p>
                                ) : null}
                                {event.actorId ? <p>Actor: {event.actorId}</p> : null}
                                {event.comment ? <p>{event.comment}</p> : null}
                              </article>
                            ))
                          ) : (
                            <div className="empty-result">
                              Request history will appear here.
                            </div>
                          )}
                        </div>
                      </div>

                      <div className="pbc-evidence-section">
                        <div className="workspace-detail-heading">
                          <div>
                            <span className="workspace-label">RECEIVED EVIDENCE</span>
                            <h3>
                              {pbcEvidenceLinks.length} exact version
                              {pbcEvidenceLinks.length === 1 ? "" : "s"} linked
                            </h3>
                          </div>
                          <span>
                            Prefer controlled evidence when the received item will support formal
                            review or sign-off.
                          </span>
                        </div>
                        <div className="workspace-grid workspace-grid-two">
                          <div className="workspace-card evidence-link-builder">
                            <form
                              className="workspace-inline-form evidence-search-form"
                              onSubmit={searchPbcEvidence}
                            >
                              <label>
                                <span>Find received document</span>
                                <input
                                  value={pbcEvidenceSearchQuery}
                                  onChange={(event) =>
                                    setPbcEvidenceSearchQuery(event.target.value)
                                  }
                                  placeholder="Search filename or path"
                                />
                              </label>
                              <button
                                className="secondary-button"
                                type="submit"
                                disabled={
                                  pbcEvidenceSearchBusy ||
                                  !pbcEvidenceSearchQuery.trim()
                                }
                              >
                                {pbcEvidenceSearchBusy ? "Searching…" : "Search"}
                              </button>
                            </form>

                            {pbcEvidenceSearchResults.length ? (
                              <div className="evidence-search-results">
                                {pbcEvidenceSearchResults.map((result) => (
                                  <button
                                    className={`workspace-list-row${
                                      selectedPbcEvidenceDocument?.documentId === result.documentId
                                        ? " workspace-list-row-active"
                                        : ""
                                    }`}
                                    type="button"
                                    key={result.fileInstanceId}
                                    onClick={() => void selectPbcEvidenceDocument(result)}
                                  >
                                    <span>
                                      <strong>{result.name}</strong>
                                      <small>{result.path}</small>
                                    </span>
                                    <span className="workspace-row-action">Versions →</span>
                                  </button>
                                ))}
                              </div>
                            ) : null}

                            {selectedPbcEvidenceDocument ? (
                              <form
                                className="workspace-form evidence-version-form"
                                onSubmit={submitPbcEvidenceLink}
                              >
                                <div className="evidence-selected-document">
                                  <span className="workspace-label">SELECTED DOCUMENT</span>
                                  <strong>{selectedPbcEvidenceDocument.name}</strong>
                                  <small>{selectedPbcEvidenceDocument.path}</small>
                                </div>
                                <label>
                                  <span>Exact received version</span>
                                  <select
                                    value={selectedPbcEvidenceVersionKey}
                                    onChange={(event) =>
                                      setSelectedPbcEvidenceVersionKey(event.target.value)
                                    }
                                  >
                                    <option value="">Select exact version</option>
                                    {pbcEvidenceVersionOptions.map((option) => (
                                      <option key={option.key} value={option.key}>
                                        {option.label}
                                      </option>
                                    ))}
                                  </select>
                                </label>
                                {selectedPbcEvidenceVersionKey.startsWith("content:") ? (
                                  <p className="evidence-integrity-note">
                                    This is an observed working-source version. Capture controlled
                                    evidence before relying on it for preserved sign-off evidence.
                                  </p>
                                ) : selectedPbcEvidenceVersionKey.startsWith("controlled:") ? (
                                  <p className="evidence-integrity-note evidence-integrity-strong">
                                    This received-evidence link will reference an immutable
                                    controlled-evidence version.
                                  </p>
                                ) : null}
                                <label>
                                  <span>Evidence description</span>
                                  <textarea
                                    value={pbcEvidenceDescription}
                                    onChange={(event) =>
                                      setPbcEvidenceDescription(event.target.value)
                                    }
                                    rows={2}
                                    placeholder="What was received and how does it answer the request?"
                                  />
                                </label>
                                <button
                                  className="primary-button"
                                  type="submit"
                                  disabled={
                                    workspaceBusy || !selectedPbcEvidenceVersionKey
                                  }
                                >
                                  Link received version
                                </button>
                              </form>
                            ) : null}
                          </div>

                          <div className="workspace-card workpaper-evidence-list">
                            {pbcEvidenceLinks.length ? (
                              pbcEvidenceLinks.map((link) => (
                                <article key={link.pbcRequestEvidenceLinkId}>
                                  <div>
                                    <strong>{link.documentName}</strong>
                                    <span>RECEIVED</span>
                                  </div>
                                  <p>
                                    {link.controlledEvidenceVersionId
                                      ? `Controlled evidence v${link.controlledVersionNumber ?? "?"} · captured ${formatTimestamp(link.controlledCapturedAtMs)}`
                                      : `Observed content · ${formatTimestamp(link.contentObservedAtMs)}`}
                                  </p>
                                  {link.description ? <p>{link.description}</p> : null}
                                  <code
                                    title={
                                      link.controlledEvidenceVersionId ??
                                      link.contentVersionId ??
                                      ""
                                    }
                                  >
                                    {link.controlledEvidenceVersionId
                                      ? `Controlled ID ${link.controlledEvidenceVersionId.slice(0, 18)}…`
                                      : `Content ID ${link.contentVersionId?.slice(0, 18) ?? "—"}…`}
                                  </code>
                                </article>
                              ))
                            ) : (
                              <div className="empty-result">
                                No received evidence version is linked yet.
                              </div>
                            )}
                          </div>
                        </div>
                      </div>
                    </div>
                  ) : null}
                </div>

                {selectedWorkpaper ? (
                  <div className="workspace-revision-panel">
                    <div className="workspace-detail-heading">
                      <div>
                        <span className="workspace-label">WORKPAPER REVISIONS</span>
                        <h3>
                          {selectedWorkpaper.reference} · {selectedWorkpaper.title}
                        </h3>
                      </div>
                      <span>
                        {workpaperRevisions.length} immutable revision
                        {workpaperRevisions.length === 1 ? "" : "s"}
                      </span>
                    </div>
                    <div className="workspace-grid workspace-grid-two">
                      <form className="workspace-card workspace-form" onSubmit={submitWorkpaperRevision}>
                        <label>
                          <span>Objective</span>
                          <textarea
                            value={newRevisionObjective}
                            onChange={(event) => setNewRevisionObjective(event.target.value)}
                            rows={3}
                            placeholder="What is this workpaper designed to establish?"
                          />
                        </label>
                        <label>
                          <span>Procedure performed</span>
                          <textarea
                            value={newRevisionProcedure}
                            onChange={(event) => setNewRevisionProcedure(event.target.value)}
                            rows={4}
                            placeholder="Document the work performed."
                          />
                        </label>
                        <label>
                          <span>Conclusion</span>
                          <textarea
                            value={newRevisionConclusion}
                            onChange={(event) => setNewRevisionConclusion(event.target.value)}
                            rows={3}
                            placeholder="Record the professional conclusion."
                          />
                        </label>
                        <button
                          className="primary-button"
                          type="submit"
                          disabled={
                            workspaceBusy ||
                            (!newRevisionObjective.trim() &&
                              !newRevisionProcedure.trim() &&
                              !newRevisionConclusion.trim())
                          }
                        >
                          Create next revision
                        </button>
                      </form>

                      <div className="workspace-card workspace-revision-list">
                        {workpaperRevisions.length ? (
                          workpaperRevisions.map((revision) => (
                            <article key={revision.workpaperRevisionId}>
                              <div>
                                <strong>Revision {revision.revisionNumber}</strong>
                                <span>{formatTimestamp(revision.createdAtMs)}</span>
                              </div>
                              {revision.revisionReason ? <p>{revision.revisionReason}</p> : null}
                              {revision.objective ? (
                                <p><b>Objective:</b> {revision.objective}</p>
                              ) : null}
                              {revision.procedurePerformed ? (
                                <p><b>Procedure:</b> {revision.procedurePerformed}</p>
                              ) : null}
                              {revision.conclusion ? (
                                <p><b>Conclusion:</b> {revision.conclusion}</p>
                              ) : null}
                              {revision.contentHashHex ? (
                                <code title={revision.contentHashHex}>
                                  SHA-256 {revision.contentHashHex.slice(0, 18)}…
                                </code>
                              ) : null}
                            </article>
                          ))
                        ) : (
                          <div className="empty-result">
                            Create the first immutable revision for this workpaper.
                          </div>
                        )}
                      </div>
                    </div>

                    <div className="workspace-evidence-panel">
                      <div className="workspace-detail-heading">
                        <div>
                          <span className="workspace-label">EXACT EVIDENCE LINKS</span>
                          <h3>
                            {workpaperRevisions.length
                              ? `Revision ${workpaperRevisions[0].revisionNumber} evidence`
                              : "Create a revision before linking evidence"}
                          </h3>
                        </div>
                        <span>
                          {workpaperEvidenceLinks.length} linked version
                          {workpaperEvidenceLinks.length === 1 ? "" : "s"}
                        </span>
                      </div>

                      <div className="workspace-grid workspace-grid-two">
                        <div className="workspace-card evidence-link-builder">
                          {latestRevisionSigned ? (
                            <p className="evidence-integrity-note evidence-integrity-strong signoff-evidence-lock">
                              Revision {workpaperRevisions[0]?.revisionNumber ?? "—"} has an active
                              sign-off. Its evidence set is locked; create a new revision to change
                              evidence.
                            </p>
                          ) : null}
                          <form className="workspace-inline-form evidence-search-form" onSubmit={searchWorkpaperEvidence}>
                            <label>
                              <span>Find indexed evidence</span>
                              <input
                                value={evidenceSearchQuery}
                                onChange={(event) => setEvidenceSearchQuery(event.target.value)}
                                placeholder="Search filename or path"
                              />
                            </label>
                            <button
                              className="secondary-button"
                              type="submit"
                              disabled={
                                evidenceSearchBusy ||
                                latestRevisionSigned ||
                                !evidenceSearchQuery.trim() ||
                                !workpaperRevisions.length
                              }
                            >
                              {evidenceSearchBusy ? "Searching…" : "Search"}
                            </button>
                          </form>

                          {evidenceSearchResults.length ? (
                            <div className="evidence-search-results">
                              {evidenceSearchResults.map((result) => (
                                <button
                                  className={`workspace-list-row${selectedEvidenceDocument?.documentId === result.documentId ? " workspace-list-row-active" : ""}`}
                                  type="button"
                                  key={result.fileInstanceId}
                                  onClick={() => void selectWorkpaperEvidenceDocument(result)}
                                >
                                  <span>
                                    <strong>{result.name}</strong>
                                    <small>{result.path}</small>
                                  </span>
                                  <span className="workspace-row-action">Versions →</span>
                                </button>
                              ))}
                            </div>
                          ) : null}

                          {selectedEvidenceDocument ? (
                            <form className="workspace-form evidence-version-form" onSubmit={submitWorkpaperEvidenceLink}>
                              <div className="evidence-selected-document">
                                <span className="workspace-label">SELECTED DOCUMENT</span>
                                <strong>{selectedEvidenceDocument.name}</strong>
                                <small>{selectedEvidenceDocument.path}</small>
                              </div>
                              <label>
                                <span>Exact version</span>
                                <select
                                  value={selectedEvidenceVersionKey}
                                  onChange={(event) =>
                                    setSelectedEvidenceVersionKey(event.target.value)
                                  }
                                >
                                  <option value="">Select exact version</option>
                                  {evidenceVersionOptions.map((option) => (
                                    <option key={option.key} value={option.key}>
                                      {option.label}
                                    </option>
                                  ))}
                                </select>
                              </label>
                              {selectedEvidenceVersionKey.startsWith("content:") ? (
                                <p className="evidence-integrity-note">
                                  This is a persisted working-source observation, not an immutable
                                  controlled copy. Capture controlled evidence before formal reviewer
                                  reliance.
                                </p>
                              ) : selectedEvidenceVersionKey.startsWith("controlled:") ? (
                                <p className="evidence-integrity-note evidence-integrity-strong">
                                  This link will bind the workpaper revision to an immutable
                                  controlled-evidence version.
                                </p>
                              ) : null}
                              <label>
                                <span>Relationship</span>
                                <input
                                  value={evidenceRelationshipType}
                                  onChange={(event) =>
                                    setEvidenceRelationshipType(event.target.value)
                                  }
                                  placeholder="SUPPORTS"
                                  maxLength={80}
                                />
                              </label>
                              <label>
                                <span>Description</span>
                                <textarea
                                  value={evidenceDescription}
                                  onChange={(event) => setEvidenceDescription(event.target.value)}
                                  rows={2}
                                  placeholder="What does this evidence support?"
                                />
                              </label>
                              <button
                                className="primary-button"
                                type="submit"
                                disabled={
                                  workspaceBusy ||
                                  latestRevisionSigned ||
                                  !selectedEvidenceVersionKey ||
                                  !evidenceRelationshipType.trim()
                                }
                              >
                                Link exact version
                              </button>
                            </form>
                          ) : null}
                        </div>

                        <div className="workspace-card workpaper-evidence-list">
                          {workpaperEvidenceLinks.length ? (
                            workpaperEvidenceLinks.map((link) => (
                              <article key={link.evidenceLinkId}>
                                <div>
                                  <strong>{link.documentName}</strong>
                                  <span>{link.relationshipType}</span>
                                </div>
                                <p>
                                  {link.controlledEvidenceVersionId
                                    ? `Controlled evidence v${link.controlledVersionNumber ?? "?"} · captured ${formatTimestamp(link.controlledCapturedAtMs)}`
                                    : `Observed content · ${formatTimestamp(link.contentObservedAtMs)}`}
                                </p>
                                {link.description ? <p>{link.description}</p> : null}
                                <code title={link.controlledEvidenceVersionId ?? link.contentVersionId ?? ""}>
                                  {link.controlledEvidenceVersionId
                                    ? `Controlled ID ${link.controlledEvidenceVersionId.slice(0, 18)}…`
                                    : `Content ID ${link.contentVersionId?.slice(0, 18) ?? "—"}…`}
                                </code>
                              </article>
                            ))
                          ) : (
                            <div className="empty-result">
                              {workpaperRevisions.length
                                ? "No evidence version is linked to the latest revision yet."
                                : "Create a workpaper revision before linking evidence."}
                            </div>
                          )}
                        </div>
                      </div>
                    </div>

                    <div className="workspace-review-panel">
                      <div className="workspace-detail-heading">
                        <div>
                          <span className="workspace-label">PREPARE / REVIEW WORKFLOW</span>
                          <h3>
                            {selectedWorkpaper.workflowState.replaceAll("_", " ")} · Revision{" "}
                            {workpaperRevisions[0]?.revisionNumber ?? "—"}
                          </h3>
                        </div>
                        <span>
                          {workpaperWorkflowEvents.length} transition
                          {workpaperWorkflowEvents.length === 1 ? "" : "s"}
                        </span>
                      </div>

                      <div className="workspace-grid workspace-grid-two">
                        <form className="workspace-card workspace-form" onSubmit={submitWorkflowTransition}>
                          <label>
                            <span>Next state</span>
                            <input
                              list="workpaper-workflow-state-options"
                              value={nextWorkflowState}
                              onChange={(event) => setNextWorkflowState(event.target.value)}
                              placeholder="PREPARED"
                              maxLength={80}
                            />
                            <datalist id="workpaper-workflow-state-options">
                              <option value="NOT_STARTED" />
                              <option value="IN_PROGRESS" />
                              <option value="PREPARED" />
                              <option value="SUBMITTED_FOR_REVIEW" />
                              <option value="REVIEW_POINT_RAISED" />
                              <option value="RESPONSE_SUBMITTED" />
                              <option value="CLEARED" />
                              <option value="FINALISED" />
                            </datalist>
                          </label>
                          <label>
                            <span>Actor identifier</span>
                            <input
                              value={workflowActorId}
                              onChange={(event) => setWorkflowActorId(event.target.value)}
                              placeholder="preparer@example.com"
                              maxLength={160}
                            />
                          </label>
                          <label>
                            <span>Transition comment</span>
                            <textarea
                              value={workflowComment}
                              onChange={(event) => setWorkflowComment(event.target.value)}
                              rows={2}
                              placeholder="Why is the workpaper moving to this state?"
                            />
                          </label>
                          {isFormalReviewState(nextWorkflowState) ? (
                            <p className="evidence-integrity-note">
                              Formal review/final states require a current revision. Any linked
                              evidence on that revision must be immutable hash-verified controlled
                              evidence; working observations will be rejected.
                            </p>
                          ) : null}
                          <button
                            className="primary-button"
                            type="submit"
                            disabled={workspaceBusy || !nextWorkflowState.trim()}
                          >
                            Record transition
                          </button>
                        </form>

                        <div className="workspace-card workflow-event-list">
                          {workpaperWorkflowEvents.length ? (
                            [...workpaperWorkflowEvents].reverse().map((event) => (
                              <article key={event.workpaperWorkflowEventId}>
                                <div>
                                  <strong>
                                    {event.fromState.replaceAll("_", " ")} →{" "}
                                    {event.toState.replaceAll("_", " ")}
                                  </strong>
                                  <span>{formatTimestamp(event.occurredAtMs)}</span>
                                </div>
                                <p>
                                  Revision{" "}
                                  {event.workpaperRevisionId
                                    ? revisionNumberById[event.workpaperRevisionId] ?? "historical"
                                    : "—"}
                                  {event.actorId ? ` · ${event.actorId}` : ""}
                                </p>
                                {event.comment ? <p>{event.comment}</p> : null}
                              </article>
                            ))
                          ) : (
                            <div className="empty-result">
                              State transitions will appear here as append-only history.
                            </div>
                          )}
                        </div>
                      </div>

                      <div className="workpaper-signoff-section">
                        <div className="workspace-detail-heading">
                          <div>
                            <span className="workspace-label">ROLE-AWARE SIGN-OFF</span>
                            <h3>
                              {workpaperSignoffs.length} historical sign-off
                              {workpaperSignoffs.length === 1 ? "" : "s"}
                            </h3>
                          </div>
                          <span>
                            {activeSignoffsForLatestRevision.length} active on revision{" "}
                            {workpaperRevisions[0]?.revisionNumber ?? "—"}
                          </span>
                        </div>

                        <div className="workspace-grid workspace-grid-two">
                          <form className="workspace-card workspace-form" onSubmit={submitWorkpaperSignoff}>
                            <label>
                              <span>Sign-off type</span>
                              <input
                                list="workpaper-signoff-type-options"
                                value={newSignoffType}
                                onChange={(event) => setNewSignoffType(event.target.value)}
                                placeholder="PREPARED"
                                maxLength={80}
                              />
                              <datalist id="workpaper-signoff-type-options">
                                <option value="PREPARED" />
                                <option value="REVIEWED" />
                                <option value="FINAL_APPROVAL" />
                              </datalist>
                            </label>
                            <div className="workspace-form-pair">
                              <label>
                                <span>Actor identifier</span>
                                <input
                                  value={newSignoffActorId}
                                  onChange={(event) => setNewSignoffActorId(event.target.value)}
                                  placeholder="manager@example.com"
                                  maxLength={160}
                                />
                              </label>
                              <label>
                                <span>Actor role</span>
                                <input
                                  value={newSignoffActorRole}
                                  onChange={(event) => setNewSignoffActorRole(event.target.value)}
                                  placeholder="Engagement Manager"
                                  maxLength={160}
                                />
                              </label>
                            </div>
                            <label>
                              <span>Comment</span>
                              <textarea
                                value={newSignoffComment}
                                onChange={(event) => setNewSignoffComment(event.target.value)}
                                rows={2}
                                placeholder="Optional sign-off comment."
                              />
                            </label>
                            {isFormalSignoffType(newSignoffType) ? (
                              <p className="evidence-integrity-note">
                                Reviewer/final sign-off requires every linked evidence item on the
                                latest revision to be hash-verified controlled evidence and all
                                review notes on that revision to be cleared.
                              </p>
                            ) : null}
                            <p className="signoff-authority-note">
                              Actor and role are recorded for professional history. Authentication
                              and role-authority enforcement are not claimed by this phase.
                            </p>
                            <button
                              className="primary-button"
                              type="submit"
                              disabled={
                                workspaceBusy ||
                                !workpaperRevisions.length ||
                                !newSignoffType.trim() ||
                                !newSignoffActorId.trim() ||
                                !newSignoffActorRole.trim()
                              }
                            >
                              Record sign-off
                            </button>
                          </form>

                          <div className="workspace-card workpaper-signoff-list">
                            {workpaperSignoffs.length ? (
                              workpaperSignoffs.map((signoff) => (
                                <article
                                  className={signoff.supersededAtMs ? "signoff-superseded" : ""}
                                  key={signoff.signoffId}
                                >
                                  <div>
                                    <strong>
                                      {signoff.signoffType.replaceAll("_", " ")} · Revision{" "}
                                      {signoff.revisionNumber}
                                    </strong>
                                    <span
                                      className={
                                        signoff.supersededAtMs
                                          ? "signoff-state signoff-state-superseded"
                                          : "signoff-state signoff-state-active"
                                      }
                                    >
                                      {signoff.supersededAtMs ? "SUPERSEDED" : "ACTIVE"}
                                    </span>
                                  </div>
                                  <p>
                                    {signoff.actorId} · {signoff.actorRole} ·{" "}
                                    {formatTimestamp(signoff.signedAtMs)}
                                  </p>
                                  <p>
                                    Evidence snapshot: {signoff.evidenceLinkIds.length} exact link
                                    {signoff.evidenceLinkIds.length === 1 ? "" : "s"}
                                  </p>
                                  {signoff.comment ? <p>{signoff.comment}</p> : null}
                                  {signoff.supersededAtMs ? (
                                    <p className="signoff-supersession-note">
                                      Superseded {formatTimestamp(signoff.supersededAtMs)}
                                      {signoff.supersededByRevisionId
                                        ? " by a later revision"
                                        : ""}
                                      {signoff.supersededReason ? " · " : ""}
                                      {signoff.supersededReason ?? ""}
                                    </p>
                                  ) : null}
                                </article>
                              ))
                            ) : (
                              <div className="empty-result">
                                No professional sign-offs have been recorded for this workpaper.
                              </div>
                            )}
                          </div>
                        </div>
                      </div>

                      <div className="review-notes-section">
                        <div className="workspace-detail-heading">
                          <div>
                            <span className="workspace-label">REVIEW NOTES</span>
                            <h3>
                              {reviewNotes.length} note{reviewNotes.length === 1 ? "" : "s"}
                            </h3>
                          </div>
                          <span>Raised against the latest exact workpaper revision</span>
                        </div>

                        <div className="workspace-grid workspace-grid-two">
                          <form className="workspace-card workspace-form" onSubmit={submitReviewNote}>
                            <label>
                              <span>Title</span>
                              <input
                                value={newReviewTitle}
                                onChange={(event) => setNewReviewTitle(event.target.value)}
                                placeholder="Explain exception treatment"
                                maxLength={240}
                              />
                            </label>
                            <label>
                              <span>Review point</span>
                              <textarea
                                value={newReviewBody}
                                onChange={(event) => setNewReviewBody(event.target.value)}
                                rows={4}
                                placeholder="Describe the review point and required action."
                              />
                            </label>
                            <div className="workspace-form-pair">
                              <label>
                                <span>Owner</span>
                                <input
                                  value={newReviewOwnerId}
                                  onChange={(event) => setNewReviewOwnerId(event.target.value)}
                                  placeholder="preparer@example.com"
                                  maxLength={160}
                                />
                              </label>
                              <label>
                                <span>Due</span>
                                <input
                                  type="datetime-local"
                                  value={newReviewDueLocal}
                                  onChange={(event) => setNewReviewDueLocal(event.target.value)}
                                />
                              </label>
                            </div>
                            <label>
                              <span>Exact evidence link (optional)</span>
                              <select
                                value={newReviewEvidenceLinkId}
                                onChange={(event) => {
                                  const evidenceLinkId = event.target.value;
                                  setNewReviewEvidenceLinkId(evidenceLinkId);
                                  if (!evidenceLinkId) {
                                    setNewReviewLocationKind("");
                                    setNewReviewLocationValue("");
                                  }
                                }}
                              >
                                <option value="">Workpaper revision only</option>
                                {workpaperEvidenceLinks.map((link) => (
                                  <option key={link.evidenceLinkId} value={link.evidenceLinkId}>
                                    {link.documentName} · {link.relationshipType}
                                  </option>
                                ))}
                              </select>
                            </label>
                            <div className="workspace-form-pair">
                              <label>
                                <span>Exact sublocation</span>
                                <select
                                  value={newReviewLocationKind}
                                  onChange={(event) => {
                                    setNewReviewLocationKind(event.target.value);
                                    setNewReviewLocationValue("");
                                  }}
                                  disabled={!newReviewEvidenceLinkId}
                                >
                                  <option value="">Document only / no sublocation</option>
                                  <option value="PAGE">Page</option>
                                  <option value="WORKSHEET">Worksheet</option>
                                  <option value="CELL">Cell</option>
                                  <option value="RANGE">Cell range</option>
                                </select>
                              </label>
                              <label>
                                <span>Anchor</span>
                                <input
                                  value={newReviewLocationValue}
                                  onChange={(event) => setNewReviewLocationValue(event.target.value)}
                                  placeholder={reviewLocationValuePlaceholder(newReviewLocationKind)}
                                  disabled={!newReviewEvidenceLinkId || !newReviewLocationKind}
                                />
                              </label>
                            </div>
                            <p className="evidence-integrity-note">
                              Select an exact evidence link before adding a page, worksheet, cell, or
                              range anchor. Cell and range anchors include the worksheet, for example
                              Trial Balance!B12.
                            </p>
                            {reviewLocationIncomplete ? (
                              <p className="evidence-integrity-note">
                                Location kind and location value must be supplied together.
                              </p>
                            ) : null}
                            <label>
                              <span>Raised by</span>
                              <input
                                value={newReviewRaisedBy}
                                onChange={(event) => setNewReviewRaisedBy(event.target.value)}
                                placeholder="reviewer@example.com"
                                maxLength={160}
                              />
                            </label>
                            <button
                              className="primary-button"
                              type="submit"
                              disabled={
                                workspaceBusy ||
                                !workpaperRevisions.length ||
                                !newReviewTitle.trim() ||
                                !newReviewBody.trim() ||
                                reviewLocationIncomplete
                              }
                            >
                              Raise review note
                            </button>
                          </form>

                          <div className="workspace-card review-note-list">
                            {reviewNotes.length ? (
                              reviewNotes.map((note) => (
                                <button
                                  className={`review-note-row${selectedReviewNoteId === note.reviewNoteId ? " review-note-row-active" : ""}`}
                                  type="button"
                                  key={note.reviewNoteId}
                                  onClick={() => void loadReviewNoteEvents(note.reviewNoteId)}
                                >
                                  <span className={`review-note-state review-note-state-${note.currentState.toLowerCase()}`}>
                                    {note.currentState.replaceAll("_", " ")}
                                  </span>
                                  <strong>{note.title}</strong>
                                  <small>
                                    Revision {revisionNumberById[note.workpaperRevisionId] ?? "historical"}
                                    {note.evidenceLinkId
                                      ? ` · ${evidenceLinkNameById[note.evidenceLinkId] ?? "exact evidence"}`
                                      : ""}
                                    {note.locationKind && note.locationValue
                                      ? ` · ${note.locationKind} ${note.locationValue}`
                                      : ""}
                                  </small>
                                  <small>
                                    {note.ownerId ? `Owner ${note.ownerId}` : "No owner"}
                                    {note.dueAtMs ? ` · Due ${formatTimestamp(note.dueAtMs)}` : ""}
                                  </small>
                                  <p>{note.body}</p>
                                </button>
                              ))
                            ) : (
                              <div className="empty-result">
                                No review notes have been raised for this workpaper.
                              </div>
                            )}
                          </div>
                        </div>

                        {selectedReviewNote ? (
                          <div className="review-note-history-panel">
                            <div className="workspace-detail-heading">
                              <div>
                                <span className="workspace-label">REVIEW NOTE HISTORY</span>
                                <h3>{selectedReviewNote.title}</h3>
                              </div>
                              <span>{selectedReviewNote.currentState.replaceAll("_", " ")}</span>
                            </div>
                            <div className="workspace-grid workspace-grid-two">
                              <form className="workspace-card workspace-form" onSubmit={submitReviewNoteResponse}>
                                <label>
                                  <span>Action actor</span>
                                  <input
                                    value={reviewActionActorId}
                                    onChange={(event) => setReviewActionActorId(event.target.value)}
                                    placeholder="preparer@example.com"
                                    maxLength={160}
                                  />
                                </label>
                                <label>
                                  <span>Response</span>
                                  <textarea
                                    value={reviewResponseText}
                                    onChange={(event) => setReviewResponseText(event.target.value)}
                                    rows={3}
                                    placeholder="Respond to the review point."
                                    disabled={selectedReviewNote.currentState === "CLEARED"}
                                  />
                                </label>
                                <label>
                                  <span>Action comment</span>
                                  <textarea
                                    value={reviewActionComment}
                                    onChange={(event) => setReviewActionComment(event.target.value)}
                                    rows={2}
                                    placeholder="Optional clearance/reopen comment."
                                  />
                                </label>
                                <button
                                  className="secondary-button"
                                  type="submit"
                                  disabled={
                                    workspaceBusy ||
                                    selectedReviewNote.currentState === "CLEARED" ||
                                    !reviewResponseText.trim()
                                  }
                                >
                                  Submit response
                                </button>
                                <div className="review-note-actions">
                                  {selectedReviewNote.currentState === "CLEARED" ? (
                                    <button
                                      className="secondary-button"
                                      type="button"
                                      onClick={() => void changeReviewNoteState("reopen_review_note")}
                                      disabled={workspaceBusy}
                                    >
                                      Reopen note
                                    </button>
                                  ) : (
                                    <button
                                      className="primary-button"
                                      type="button"
                                      onClick={() => void changeReviewNoteState("clear_review_note")}
                                      disabled={workspaceBusy}
                                    >
                                      Clear note
                                    </button>
                                  )}
                                </div>
                              </form>

                              <div className="workspace-card review-note-event-list">
                                {reviewNoteEvents.length ? (
                                  reviewNoteEvents.map((event) => (
                                    <article key={event.reviewNoteEventId}>
                                      <div>
                                        <strong>{event.eventType.replaceAll("_", " ")}</strong>
                                        <span>{formatTimestamp(event.occurredAtMs)}</span>
                                      </div>
                                      {event.actorId ? <p>Actor: {event.actorId}</p> : null}
                                      {event.responseText ? (
                                        <p><b>Response:</b> {event.responseText}</p>
                                      ) : null}
                                      {event.comment ? <p>{event.comment}</p> : null}
                                    </article>
                                  ))
                                ) : (
                                  <div className="empty-result">Loading review-note history…</div>
                                )}
                              </div>
                            </div>
                          </div>
                        ) : null}
                      </div>
                    </div>
                  </div>
                ) : null}
              </div>
            ) : null}
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
                        {stateLabel(file.availabilityState)}
                      </span>
                      {renderPreviewAction(file, hasQuery ? query : undefined)}
                      {renderHistoryAction(file)}
                      {renderRelationshipAction(file)}
                      {renderCaptureAction(file)}
                      <button
                        className="file-action"
                        type="button"
                        onClick={() => void openFileInstance(file.fileInstanceId)}
                        disabled={sourceUnavailable(file.availabilityState)}
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

        {!hasQuery && !selectedRoot && viewMode === "searches" ? (
          <section className="results-panel quick-access-panel" aria-label="Recent searches">
            <div className="results-heading">
              <div>
                <p className="eyebrow">RECENT SEARCHES</p>
                <h2>Searches that led to an opened document</h2>
              </div>
              <span>
                Search-as-you-type fragments are not saved. A query appears here only
                after you use it to open a result.
              </span>
            </div>
            <div className="recent-search-list">
              {recentSearches.length ? (
                recentSearches.map((search) => (
                  <button
                    className="recent-search-row"
                    type="button"
                    key={search.normalizedQuery}
                    onClick={() => void reuseRecentSearch(search)}
                  >
                    <span className="recent-search-icon" aria-hidden="true">⌕</span>
                    <span className="recent-search-main">
                      <strong>{search.queryText}</strong>
                      <span>
                        Used {search.useCount} time{search.useCount === 1 ? "" : "s"}
                      </span>
                    </span>
                    <span className="recent-search-action">Search again</span>
                  </button>
                ))
              ) : (
                <div className="empty-result">
                  Open a document from universal search and that useful query will appear here.
                </div>
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
                        {stateLabel(file.availabilityState)}
                      </span>
                      {renderPreviewAction(file, hasQuery ? query : undefined)}
                      {renderHistoryAction(file)}
                      {renderRelationshipAction(file)}
                      {renderCaptureAction(file)}
                      <button
                        className="file-action"
                        type="button"
                        onClick={() => void openFileInstance(file.fileInstanceId)}
                        disabled={sourceUnavailable(file.availabilityState)}
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

        {viewMode === "home" && roots.length ? (
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
                        {stateLabel(file.availabilityState)}
                      </span>
                      <span className="file-size">{formatBytes(file.sizeBytes)}</span>
                      {renderPreviewAction(file, hasQuery ? query : undefined)}
                      {renderHistoryAction(file)}
                      {renderRelationshipAction(file)}
                      {renderCaptureAction(file)}
                      <button
                        className="file-action"
                        type="button"
                        onClick={() => void openFileInstance(file.fileInstanceId)}
                        disabled={sourceUnavailable(file.availabilityState)}
                      >
                        Open
                      </button>
                      <button
                        className="file-action"
                        type="button"
                        onClick={() => void revealFileInstance(file.fileInstanceId)}
                        disabled={sourceUnavailable(file.availabilityState)}
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
              <h2>Recent searches</h2>
              <p>
                {recentSearches.length
                  ? `${recentSearches.length} useful search${recentSearches.length === 1 ? "" : "es"} ready to repeat from the sidebar.`
                  : "Searches are remembered only after they successfully open a document."}
              </p>
            </article>
          </section>
        ) : null}
        {activeVersionHistory ? (
          <div
            className="text-preview-backdrop"
            role="presentation"
            onMouseDown={(event) => {
              if (event.currentTarget === event.target) {
                setActiveVersionHistory(null);
              }
            }}
          >
            <section
              className="version-history-dialog"
              role="dialog"
              aria-modal="true"
              aria-labelledby="version-history-title"
            >
              <header className="text-preview-header">
                <div>
                  <p className="eyebrow">DOCUMENT VERSION HISTORY</p>
                  <h2 id="version-history-title">{activeVersionHistory.file.name}</h2>
                  <span title={activeVersionHistory.file.path}>
                    {activeVersionHistory.file.path}
                  </span>
                </div>
                <div className="text-preview-actions">
                  <button
                    className="file-action"
                    type="button"
                    onClick={() => setActiveVersionHistory(null)}
                  >
                    Close
                  </button>
                </div>
              </header>

              <div className="version-history-body">
                {activeVersionHistory.entries.length ? (
                  activeVersionHistory.entries.map((entry) => {
                    const isControlled = entry.controlledVersionNumber !== null;
                    const verification =
                      entry.controlledVerificationState ?? entry.verificationState;

                    return (
                      <article
                        className={
                          isControlled
                            ? "version-history-entry version-history-entry-controlled"
                            : "version-history-entry"
                        }
                        key={entry.contentVersionId}
                      >
                        <div className="version-history-entry-heading">
                          <div>
                            <strong>
                              {isControlled
                                ? "Controlled evidence v" + entry.controlledVersionNumber
                                : "Linked source observation"}
                            </strong>
                            <span>
                              {formatTimestamp(entry.observedAtMs)} ·{" "}
                              {formatBytes(entry.sizeBytes)}
                            </span>
                          </div>
                          <span className="version-history-verification">
                            {verification.replaceAll("_", " ")}
                          </span>
                        </div>

                        <dl className="version-history-details">
                          <div>
                            <dt>Source modified</dt>
                            <dd>{formatTimestamp(entry.lastWriteTimeMs)}</dd>
                          </div>
                          {isControlled ? (
                            <>
                              <div>
                                <dt>Captured</dt>
                                <dd>{formatTimestamp(entry.capturedAtMs)}</dd>
                              </div>
                              <div>
                                <dt>Capture reason</dt>
                                <dd>
                                  {entry.captureReason?.replaceAll("_", " ") ??
                                    "Not recorded"}
                                </dd>
                              </div>
                              <div>
                                <dt>Capture policy</dt>
                                <dd>
                                  {entry.capturePolicy?.replaceAll("_", " ") ??
                                    "Not recorded"}
                                </dd>
                              </div>
                              <div>
                                <dt>Captured by</dt>
                                <dd>{entry.capturedBy ?? "Not recorded"}</dd>
                              </div>
                              <div>
                                <dt>Stable during read</dt>
                                <dd>
                                  {entry.sourceStableDuringRead === true
                                    ? "Yes"
                                    : entry.sourceStableDuringRead === false
                                      ? "No"
                                      : "Not recorded"}
                                </dd>
                              </div>
                            </>
                          ) : null}
                        </dl>

                        {entry.sha256Hex ? (
                          <code
                            className="version-history-hash"
                            title={entry.sha256Hex}
                          >
                            SHA-256 {entry.sha256Hex}
                          </code>
                        ) : (
                          <span className="version-history-no-hash">
                            Metadata observation · cryptographic hash not recorded
                          </span>
                        )}
                      </article>
                    );
                  })
                ) : (
                  <div className="empty-result">
                    No persisted content-version records exist for this document.
                  </div>
                )}
              </div>

              <footer className="text-preview-footer">
                <span>Newest persisted record first.</span>
                <span>
                  Controlled evidence entries are immutable captures; linked-source
                  observations describe the external working file at scan/capture time.
                </span>
              </footer>
            </section>
          </div>
        ) : null}
        {activeTextPreview ? (
          <div
            className="text-preview-backdrop"
            role="presentation"
            onMouseDown={(event) => {
              if (event.currentTarget === event.target) {
                setActiveTextPreview(null);
              }
            }}
          >
            <section
              className="text-preview-dialog"
              role="dialog"
              aria-modal="true"
              aria-labelledby="text-preview-title"
            >
              <header className="text-preview-header">
                <div>
                  <p className="eyebrow">
                    IN-APP PREVIEW · {activeTextPreview.preview.extension.toUpperCase()}
                  </p>
                  <h2 id="text-preview-title">{activeTextPreview.file.name}</h2>
                  <span>
                    {stateLabel(activeTextPreview.file.availabilityState)} ·{" "}
                    {formatBytes(activeTextPreview.preview.totalSizeBytes)}
                  </span>
                </div>
                <div className="text-preview-actions">
                  <button
                    className="secondary-button"
                    type="button"
                    onClick={() =>
                      void openFileInstance(activeTextPreview.file.fileInstanceId)
                    }
                  >
                    Open original
                  </button>
                  <button
                    className="file-action file-action-related"
                    type="button"
                    onClick={() => void loadRelationshipContext(activeTextPreview.file)}
                    disabled={relationshipContextLoadingDocumentId !== null}
                  >
                    {relationshipContextLoadingDocumentId === activeTextPreview.file.documentId
                      ? "Loading links…"
                      : "Related"}
                  </button>
                  <button
                    className="file-action"
                    type="button"
                    onClick={() => {
                      setActiveTextPreview(null);
                      resetViewerSearch();
                    }}
                  >
                    Close
                  </button>
                </div>
              </header>

              {renderViewerSearchBar("local")}

              <div className="text-preview-body">
                <pre>
                  {activeTextPreview.preview.content
                    ? renderHighlightedText(
                        activeTextPreview.preview.content,
                        "text",
                        localViewerMatches,
                        safeViewerSearchIndex,
                      )
                    : "This file is empty."}
                </pre>
              </div>

              <footer className="text-preview-footer">
                <span>
                  Previewed {formatBytes(activeTextPreview.preview.previewedBytes)}
                  {activeTextPreview.preview.truncated
                    ? " of " + formatBytes(activeTextPreview.preview.totalSizeBytes)
                    : ""}
                </span>
                {activeTextPreview.preview.truncated ? (
                  <strong>
                    Preview limited to the first 256 KB. Open the original for the complete file.
                  </strong>
                ) : (
                  <span>Original file remains at its approved source location.</span>
                )}
              </footer>
            </section>
          </div>
        ) : null}
        {activePdfPreview ? (
          <div
            className="text-preview-backdrop"
            role="presentation"
            onMouseDown={(event) => {
              if (event.currentTarget === event.target) {
                closePdfPreview();
              }
            }}
          >
            <section
              className="pdf-preview-dialog"
              role="dialog"
              aria-modal="true"
              aria-labelledby="pdf-preview-title"
            >
              <header className="text-preview-header">
                <div>
                  <p className="eyebrow">IN-APP PREVIEW · PDF</p>
                  <h2 id="pdf-preview-title">{activePdfPreview.file.name}</h2>
                  <span>
                    {stateLabel(activePdfPreview.file.availabilityState)} ·{" "}
                    {formatBytes(activePdfPreview.file.sizeBytes)}
                  </span>
                </div>
                <div className="text-preview-actions">
                  <button
                    className="secondary-button"
                    type="button"
                    onClick={() =>
                      void openFileInstance(activePdfPreview.file.fileInstanceId)
                    }
                  >
                    Open original
                  </button>
                  <button
                    className="file-action file-action-related"
                    type="button"
                    onClick={() => void loadRelationshipContext(activePdfPreview.file)}
                    disabled={relationshipContextLoadingDocumentId !== null}
                  >
                    {relationshipContextLoadingDocumentId === activePdfPreview.file.documentId
                      ? "Loading links…"
                      : "Related"}
                  </button>
                  <button
                    className="file-action"
                    type="button"
                    onClick={() => {
                      closePdfPreview();
                      resetViewerSearch();
                    }}
                  >
                    Close
                  </button>
                </div>
              </header>

              <div className="viewer-search-bar viewer-search-native">
                <strong>Find in PDF</strong>
                <span>
                  Click inside the PDF, then press Ctrl+F to use the native PDF find controls.
                </span>
              </div>

              <div className="pdf-preview-body">
                <iframe
                  src={activePdfPreview.url}
                  title={`PDF preview: ${activePdfPreview.file.name}`}
                  referrerPolicy="no-referrer"
                />
              </div>

              <footer className="text-preview-footer">
                <span>
                  PDF content is served only from the validated indexed source.
                </span>
                <span>Preview limit: 64 MB · Open original for larger files.</span>
              </footer>
            </section>
          </div>
        ) : null}
        {activeImagePreview ? (
          <div
            className="text-preview-backdrop"
            role="presentation"
            onMouseDown={(event) => {
              if (event.currentTarget === event.target) {
                closeImagePreview();
              }
            }}
          >
            <section
              className="image-preview-dialog"
              role="dialog"
              aria-modal="true"
              aria-labelledby="image-preview-title"
            >
              <header className="text-preview-header">
                <div>
                  <p className="eyebrow">
                    IN-APP PREVIEW · {activeImagePreview.file.extension.toUpperCase()}
                  </p>
                  <h2 id="image-preview-title">{activeImagePreview.file.name}</h2>
                  <span>
                    {stateLabel(activeImagePreview.file.availabilityState)} ·{" "}
                    {formatBytes(activeImagePreview.file.sizeBytes)}
                  </span>
                </div>
                <div className="text-preview-actions">
                  <button
                    className="secondary-button"
                    type="button"
                    onClick={() =>
                      void openFileInstance(activeImagePreview.file.fileInstanceId)
                    }
                  >
                    Open original
                  </button>
                  <button
                    className="file-action file-action-related"
                    type="button"
                    onClick={() => void loadRelationshipContext(activeImagePreview.file)}
                    disabled={relationshipContextLoadingDocumentId !== null}
                  >
                    {relationshipContextLoadingDocumentId ===
                    activeImagePreview.file.documentId
                      ? "Loading links…"
                      : "Related"}
                  </button>
                  <button
                    className="file-action"
                    type="button"
                    onClick={closeImagePreview}
                  >
                    Close
                  </button>
                </div>
              </header>

              <div className="image-preview-body">
                <img
                  src={activeImagePreview.url}
                  alt={activeImagePreview.file.name}
                  draggable={false}
                  referrerPolicy="no-referrer"
                />
              </div>

              <footer className="text-preview-footer">
                <span>
                  Raster image bytes are served only from the validated indexed source.
                </span>
                <span>
                  PNG, JPEG, GIF, WebP, BMP · Preview limit: 32 MB · SVG is intentionally excluded.
                </span>
              </footer>
            </section>
          </div>
        ) : null}
        {activeWorkbookPreview ? (
          <div
            className="text-preview-backdrop"
            role="presentation"
            onMouseDown={(event) => {
              if (event.currentTarget === event.target) {
                closeWorkbookPreview();
              }
            }}
          >
            <section
              className="workbook-preview-dialog"
              role="dialog"
              aria-modal="true"
              aria-labelledby="workbook-preview-title"
            >
              <header className="text-preview-header workbook-preview-header">
                <div>
                  <p className="eyebrow">
                    IN-APP PREVIEW · {activeWorkbookPreview.preview.extension.toUpperCase()}
                  </p>
                  <h2 id="workbook-preview-title">{activeWorkbookPreview.file.name}</h2>
                  <span>
                    {stateLabel(activeWorkbookPreview.file.availabilityState)} ·{" "}
                    {formatBytes(activeWorkbookPreview.preview.totalSizeBytes)}
                  </span>
                </div>
                <div className="text-preview-actions">
                  <button
                    className="secondary-button"
                    type="button"
                    onClick={() =>
                      void openFileInstance(activeWorkbookPreview.file.fileInstanceId)
                    }
                  >
                    Open original
                  </button>
                  <button
                    className="file-action file-action-related"
                    type="button"
                    onClick={() => void loadRelationshipContext(activeWorkbookPreview.file)}
                    disabled={relationshipContextLoadingDocumentId !== null}
                  >
                    {relationshipContextLoadingDocumentId === activeWorkbookPreview.file.documentId
                      ? "Loading links…"
                      : "Related"}
                  </button>
                  <button
                    className="file-action"
                    type="button"
                    onClick={() => {
                      closeWorkbookPreview();
                      resetViewerSearch();
                    }}
                  >
                    Close
                  </button>
                </div>
              </header>

              {renderViewerSearchBar("workbook")}

              <div className="workbook-sheet-tabs" aria-label="Workbook sheets">
                {activeWorkbookPreview.preview.sheets.map((sheet) => (
                  <button
                    type="button"
                    key={sheet.name}
                    className={
                      sheet.name === activeWorkbookPreview.preview.selectedSheet
                        ? "workbook-sheet-tab workbook-sheet-tab-active"
                        : "workbook-sheet-tab"
                    }
                    disabled={!sheet.previewable || previewingFileInstanceId !== null}
                    onClick={() =>
                      void loadWorkbookPreview(
                        activeWorkbookPreview.file,
                        sheet.name,
                        undefined,
                        undefined,
                      )
                    }
                    title={
                      sheet.previewable
                        ? `${sheet.sheetType} · ${sheet.visibility}`
                        : `${sheet.sheetType} cannot be rendered as worksheet cells`
                    }
                  >
                    <span>{sheet.name}</span>
                    <small>{sheet.visibility.replaceAll("_", " ")}</small>
                  </button>
                ))}
              </div>

              <div className="workbook-preview-toolbar">
                <div>
                  <strong>{activeWorkbookPreview.preview.selectedSheet}</strong>
                  <span>
                    Used range{" "}
                    {columnLabel(activeWorkbookPreview.preview.usedStartColumn)}
                    {activeWorkbookPreview.preview.usedStartRow + 1}:
                    {columnLabel(activeWorkbookPreview.preview.usedEndColumn)}
                    {activeWorkbookPreview.preview.usedEndRow + 1}
                  </span>
                </div>
                <div className="workbook-page-actions">
                  <button
                    className="file-action"
                    type="button"
                    disabled={
                      activeWorkbookPreview.preview.rowOffset <=
                        activeWorkbookPreview.preview.usedStartRow ||
                      previewingFileInstanceId !== null
                    }
                    onClick={() =>
                      void loadWorkbookPreview(
                        activeWorkbookPreview.file,
                        activeWorkbookPreview.preview.selectedSheet,
                        Math.max(
                          activeWorkbookPreview.preview.usedStartRow,
                          activeWorkbookPreview.preview.rowOffset - WORKBOOK_PAGE_ROWS,
                        ),
                        activeWorkbookPreview.preview.columnOffset,
                      )
                    }
                  >
                    ↑ Rows
                  </button>
                  <button
                    className="file-action"
                    type="button"
                    disabled={
                      activeWorkbookPreview.preview.rowOffset +
                        activeWorkbookPreview.preview.rowCount >
                        activeWorkbookPreview.preview.usedEndRow ||
                      previewingFileInstanceId !== null
                    }
                    onClick={() =>
                      void loadWorkbookPreview(
                        activeWorkbookPreview.file,
                        activeWorkbookPreview.preview.selectedSheet,
                        activeWorkbookPreview.preview.rowOffset + WORKBOOK_PAGE_ROWS,
                        activeWorkbookPreview.preview.columnOffset,
                      )
                    }
                  >
                    ↓ Rows
                  </button>
                  <button
                    className="file-action"
                    type="button"
                    disabled={
                      activeWorkbookPreview.preview.columnOffset <=
                        activeWorkbookPreview.preview.usedStartColumn ||
                      previewingFileInstanceId !== null
                    }
                    onClick={() =>
                      void loadWorkbookPreview(
                        activeWorkbookPreview.file,
                        activeWorkbookPreview.preview.selectedSheet,
                        activeWorkbookPreview.preview.rowOffset,
                        Math.max(
                          activeWorkbookPreview.preview.usedStartColumn,
                          activeWorkbookPreview.preview.columnOffset - WORKBOOK_PAGE_COLUMNS,
                        ),
                      )
                    }
                  >
                    ← Columns
                  </button>
                  <button
                    className="file-action"
                    type="button"
                    disabled={
                      activeWorkbookPreview.preview.columnOffset +
                        activeWorkbookPreview.preview.columnCount >
                        activeWorkbookPreview.preview.usedEndColumn ||
                      previewingFileInstanceId !== null
                    }
                    onClick={() =>
                      void loadWorkbookPreview(
                        activeWorkbookPreview.file,
                        activeWorkbookPreview.preview.selectedSheet,
                        activeWorkbookPreview.preview.rowOffset,
                        activeWorkbookPreview.preview.columnOffset + WORKBOOK_PAGE_COLUMNS,
                      )
                    }
                  >
                    Columns →
                  </button>
                </div>
              </div>

              <div className="workbook-grid-wrap">
                {activeWorkbookPreview.preview.rowCount &&
                activeWorkbookPreview.preview.columnCount ? (
                  <table className="workbook-grid">
                    <thead>
                      <tr>
                        <th className="workbook-corner" aria-label="Row number" />
                        {Array.from(
                          { length: activeWorkbookPreview.preview.columnCount },
                          (_, index) =>
                            activeWorkbookPreview.preview.columnOffset + index,
                        ).map((column) => (
                          <th
                            className={
                              workbookHiddenColumns.has(column)
                                ? "workbook-dimension-hidden"
                                : undefined
                            }
                            key={column}
                            title={
                              workbookHiddenColumns.has(column)
                                ? "This column is hidden in the source workbook."
                                : undefined
                            }
                          >
                            {columnLabel(column)}
                          </th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      {Array.from(
                        { length: activeWorkbookPreview.preview.rowCount },
                        (_, index) => activeWorkbookPreview.preview.rowOffset + index,
                      ).map((row) => (
                        <tr key={row}>
                          <th
                            className={
                              workbookHiddenRows.has(row)
                                ? "workbook-dimension-hidden"
                                : undefined
                            }
                            title={
                              workbookHiddenRows.has(row)
                                ? "This row is hidden in the source workbook."
                                : undefined
                            }
                          >
                            {row + 1}
                          </th>
                          {Array.from(
                            { length: activeWorkbookPreview.preview.columnCount },
                            (_, index) =>
                              activeWorkbookPreview.preview.columnOffset + index,
                          ).map((column) => {
                            const positionKey = workbookCellKey(row, column);
                            const cell =
                              activeWorkbookPreview.cellsByPosition[positionKey];
                            const comment =
                              activeWorkbookPreview.commentsByPosition[positionKey];
                            const hiddenDimension =
                              workbookHiddenRows.has(row) ||
                              workbookHiddenColumns.has(column);

                            const searchKey =
                              `${activeWorkbookPreview.preview.selectedSheet}:${row}:${column}`;
                            const isSearchHit = workbookSearchHitKeys.has(searchKey);
                            const isActiveSearchHit =
                              activeWorkbookSearchHit?.sheetName ===
                                activeWorkbookPreview.preview.selectedSheet &&
                              activeWorkbookSearchHit.row === row &&
                              activeWorkbookSearchHit.column === column;

                            return (
                              <td
                                key={column}
                                id={isActiveSearchHit ? "workbook-search-active" : undefined}
                                className={[
                                  "workbook-cell",
                                  cell?.formula ? "workbook-cell-formula" : "",
                                  comment ? "workbook-cell-commented" : "",
                                  hiddenDimension ? "workbook-cell-hidden-source" : "",
                                  isSearchHit ? "workbook-cell-search-hit" : "",
                                  isActiveSearchHit ? "workbook-cell-search-active" : "",
                                ]
                                  .filter(Boolean)
                                  .join(" ")}
                                title={
                                  comment
                                    ? `${comment.address} · Note${comment.author ? ` by ${comment.author}` : ""}: ${comment.text || "(empty note)"}`
                                    : cell?.address ?? `${columnLabel(column)}${row + 1}`
                                }
                              >
                                {cell ? (
                                  <>
                                    <span>
                                      {isSearchHit
                                        ? renderInlineQueryHighlight(
                                            cell.value || " ",
                                            viewerSearchQuery,
                                          )
                                        : cell.value || " "}
                                    </span>
                                    {cell.formula ? (
                                      <code>
                                        {isSearchHit
                                          ? renderInlineQueryHighlight(
                                              cell.formula.startsWith("=")
                                                ? cell.formula
                                                : `=${cell.formula}`,
                                              viewerSearchQuery,
                                            )
                                          : cell.formula.startsWith("=")
                                            ? cell.formula
                                            : `=${cell.formula}`}
                                      </code>
                                    ) : null}
                                  </>
                                ) : null}
                                {comment ? (
                                  <span
                                    className="workbook-comment-indicator"
                                    aria-label={`Note on ${comment.address}`}
                                  >
                                    NOTE
                                  </span>
                                ) : null}
                              </td>
                            );
                          })}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                ) : (
                  <div className="empty-result">This worksheet has no used cells.</div>
                )}
              </div>

              <div className="workbook-metadata">
                <div>
                  <strong>
                    Merged ranges ({activeWorkbookPreview.preview.mergedRanges.length})
                  </strong>
                  <span>
                    {activeWorkbookPreview.preview.mergedRanges.length
                      ? activeWorkbookPreview.preview.mergedRanges
                          .slice(0, 12)
                          .map((range) => range.address)
                          .join(", ")
                      : "None detected for this sheet."}
                  </span>
                </div>
                <div>
                  <strong>
                    Hyperlinks ({activeWorkbookPreview.preview.hyperlinks.length})
                  </strong>
                  <span>
                    {activeWorkbookPreview.preview.hyperlinks.length
                      ? activeWorkbookPreview.preview.hyperlinks
                          .slice(0, 8)
                          .map(
                            (link) =>
                              `${link.range.address}: ${link.displayedText || link.target || link.location || "link"}`,
                          )
                          .join(" · ")
                      : "None surfaced for this sheet/format."}
                  </span>
                </div>
                <div>
                  <strong>
                    Notes / legacy comments ({activeWorkbookPreview.preview.comments.length})
                  </strong>
                  <span>
                    {!activeWorkbookPreview.preview.ooxmlMetadataAvailable
                      ? "OOXML note metadata is unavailable for this workbook format."
                      : activeWorkbookPreview.preview.comments.length
                        ? activeWorkbookPreview.preview.comments
                            .slice(0, 6)
                            .map(
                              (comment) =>
                                `${comment.address}${comment.author ? ` · ${comment.author}` : ""}: ${comment.text || "(empty note)"}`,
                            )
                            .join(" · ")
                        : "None detected for this sheet."}
                  </span>
                </div>
                <div>
                  <strong>
                    Hidden rows ({activeWorkbookPreview.preview.hiddenRows.length})
                  </strong>
                  <span>
                    {!activeWorkbookPreview.preview.ooxmlMetadataAvailable
                      ? "OOXML row visibility metadata is unavailable for this workbook format."
                      : activeWorkbookPreview.preview.hiddenRows.length
                        ? activeWorkbookPreview.preview.hiddenRows
                            .slice(0, 20)
                            .map((row) => row + 1)
                            .join(", ")
                        : "None detected for this sheet."}
                  </span>
                </div>
                <div>
                  <strong>
                    Hidden columns ({activeWorkbookPreview.preview.hiddenColumns.length})
                  </strong>
                  <span>
                    {!activeWorkbookPreview.preview.ooxmlMetadataAvailable
                      ? "OOXML column visibility metadata is unavailable for this workbook format."
                      : activeWorkbookPreview.preview.hiddenColumns.length
                        ? activeWorkbookPreview.preview.hiddenColumns
                            .slice(0, 20)
                            .map(columnLabel)
                            .join(", ")
                        : "None detected for this sheet."}
                  </span>
                </div>
              </div>

              <footer className="text-preview-footer workbook-preview-footer">
                <span>
                  Values and formulas are read-only. Sheet visibility is preserved.
                </span>
                <span>
                  {activeWorkbookPreview.preview.metadataTruncated
                    ? "Sheet metadata list truncated for safety. "
                    : ""}
                  {activeWorkbookPreview.preview.ooxmlMetadataAvailable
                    ? "OOXML notes and hidden row/column indicators are preserved for this sheet; threaded comments are not yet surfaced."
                    : "Notes and hidden row/column indicators are not available for this workbook format."}
                </span>
              </footer>
            </section>
          </div>
        ) : null}
        {activeWordPreview ? (
          <div
            className="text-preview-backdrop"
            role="presentation"
            onMouseDown={(event) => {
              if (event.currentTarget === event.target) {
                closeWordPreview();
              }
            }}
          >
            <section
              className="word-preview-dialog"
              role="dialog"
              aria-modal="true"
              aria-labelledby="word-preview-title"
            >
              <header className="text-preview-header">
                <div>
                  <p className="eyebrow">IN-APP PREVIEW · DOCX</p>
                  <h2 id="word-preview-title">{activeWordPreview.file.name}</h2>
                  <span>
                    {stateLabel(activeWordPreview.file.availabilityState)} ·{" "}
                    {formatBytes(activeWordPreview.preview.totalSizeBytes)} ·{" "}
                    {activeWordPreview.preview.blockCount} block
                    {activeWordPreview.preview.blockCount === 1 ? "" : "s"}
                  </span>
                </div>
                <div className="text-preview-actions">
                  <button
                    className="secondary-button"
                    type="button"
                    onClick={() =>
                      void openFileInstance(activeWordPreview.file.fileInstanceId)
                    }
                  >
                    Open original
                  </button>
                  <button
                    className="file-action file-action-related"
                    type="button"
                    onClick={() => void loadRelationshipContext(activeWordPreview.file)}
                    disabled={relationshipContextLoadingDocumentId !== null}
                  >
                    {relationshipContextLoadingDocumentId === activeWordPreview.file.documentId
                      ? "Loading links…"
                      : "Related"}
                  </button>
                  <button
                    className="file-action"
                    type="button"
                    onClick={() => {
                      closeWordPreview();
                      resetViewerSearch();
                    }}
                  >
                    Close
                  </button>
                </div>
              </header>

              {renderViewerSearchBar("local")}

              <div className="word-preview-notice">
                Structural preview of the main DOCX document body. Exact Word pagination,
                floating objects, headers/footers, comments, tracked changes, and typography
                are not reproduced in this foundation.
              </div>

              <div className="word-preview-body">
                <article className="word-document">
                  {activeWordPreview.preview.blocks.map((block, blockIndex) =>
                    block.kind === "paragraph" ? (
                      <div
                        className={wordParagraphClass(block.style)}
                        key={`paragraph-${blockIndex}`}
                      >
                        {block.style ? (
                          <span className="word-style-label">{block.style}</span>
                        ) : null}
                        <span>
                          {renderHighlightedText(
                            block.text,
                            `p:${blockIndex}`,
                            localViewerMatches,
                            safeViewerSearchIndex,
                          )}
                        </span>
                      </div>
                    ) : (
                      <div
                        className="word-table-wrap"
                        key={`table-${blockIndex}`}
                      >
                        <table className="word-table">
                          <tbody>
                            {block.rows.map((row, rowIndex) => (
                              <tr key={rowIndex}>
                                {row.map((cell, cellIndex) => (
                                  <td key={cellIndex}>
                                    {renderHighlightedText(
                                      cell || " ",
                                      `t:${blockIndex}:${rowIndex}:${cellIndex}`,
                                      localViewerMatches,
                                      safeViewerSearchIndex,
                                    )}
                                  </td>
                                ))}
                              </tr>
                            ))}
                          </tbody>
                        </table>
                        {block.truncated ? (
                          <span className="word-truncation-note">
                            Table preview was truncated for safety.
                          </span>
                        ) : null}
                      </div>
                    ),
                  )}
                  {!activeWordPreview.preview.blocks.length ? (
                    <div className="empty-result">
                      No previewable paragraphs or tables were found in the main document body.
                    </div>
                  ) : null}
                </article>
              </div>

              <footer className="text-preview-footer">
                <span>Read-only preview · approved indexed source only.</span>
                <span>
                  {activeWordPreview.preview.truncated
                    ? "Preview truncated at the safety limit. Open the original for the complete document."
                    : "Open original for full Word rendering fidelity."}
                </span>
              </footer>
            </section>
          </div>
        ) : null}
        {activeRelationshipContext ? (
          <div
            className="text-preview-backdrop relationship-context-backdrop"
            role="presentation"
            onMouseDown={(event) => {
              if (event.currentTarget === event.target) {
                setActiveRelationshipContext(null);
              }
            }}
          >
            <section
              className="relationship-context-dialog"
              role="dialog"
              aria-modal="true"
              aria-labelledby="relationship-context-title"
            >
              <header className="text-preview-header">
                <div>
                  <p className="eyebrow">RELATED DOCUMENT CONTEXT</p>
                  <h2 id="relationship-context-title">
                    {activeRelationshipContext.file.name}
                  </h2>
                  <span>
                    {activeRelationshipContext.relationships.length} active relationship
                    {activeRelationshipContext.relationships.length === 1 ? "" : "s"}
                  </span>
                </div>
                <div className="text-preview-actions">
                  <button
                    className="file-action"
                    type="button"
                    onClick={() => setActiveRelationshipContext(null)}
                  >
                    Close
                  </button>
                </div>
              </header>

              <div className="relationship-context-body">
                <section className="relationship-existing" aria-label="Existing relationships">
                  <div className="relationship-section-heading">
                    <div>
                      <strong>Current links</strong>
                      <span>
                        Incoming and outgoing links are shown together so supporting context
                        is reachable without another global search.
                      </span>
                    </div>
                  </div>

                  <div className="relationship-list">
                    {activeRelationshipContext.relationships.length ? (
                      activeRelationshipContext.relationships.map((relationship) => {
                        const relatedFile = relationship.relatedFile;
                        const unavailable =
                          relatedFile === null ||
                          sourceUnavailable(relatedFile.availabilityState);

                        return (
                          <article
                            className="relationship-row"
                            key={relationship.documentRelationshipId}
                          >
                            <div className="relationship-direction">
                              <span>
                                {relationship.direction === "OUTGOING" ? "OUTGOING" : "INCOMING"}
                              </span>
                              <strong>{relationship.relationshipType}</strong>
                            </div>
                            <div className="relationship-main">
                              <strong>{relationship.relatedDocumentName}</strong>
                              <span>
                                {relatedFile
                                  ? relatedFile.path
                                  : "No current file instance is available for this document."}
                              </span>
                              <small>
                                Linked {formatTimestamp(relationship.createdAtMs)}
                              </small>
                            </div>
                            <div className="relationship-actions">
                              {relatedFile ? (
                                <button
                                  className="file-action"
                                  type="button"
                                  disabled={unavailable}
                                  onClick={() =>
                                    void openFileInstance(relatedFile.fileInstanceId)
                                  }
                                >
                                  Open
                                </button>
                              ) : null}
                              <button
                                className="file-action file-action-remove-link"
                                type="button"
                                disabled={relationshipMutationId !== null}
                                onClick={() =>
                                  void removeDocumentRelationship(relationship)
                                }
                              >
                                {relationshipMutationId ===
                                relationship.documentRelationshipId
                                  ? "Removing…"
                                  : "Remove"}
                              </button>
                            </div>
                          </article>
                        );
                      })
                    ) : (
                      <div className="empty-result">
                        No explicit relationships have been recorded for this document yet.
                      </div>
                    )}
                  </div>
                </section>

                <section className="relationship-add" aria-label="Add relationship">
                  <div className="relationship-section-heading">
                    <div>
                      <strong>Add a related document</strong>
                      <span>
                        Relationship labels are free-form so the same model can represent
                        support, source, response, workpaper, or future firm-specific links.
                      </span>
                    </div>
                  </div>

                  <form
                    className="relationship-link-form"
                    onSubmit={(event) => {
                      event.preventDefault();
                      void searchRelationshipCandidates();
                    }}
                  >
                    <label>
                      <span>Relationship label</span>
                      <input
                        value={relationshipType}
                        maxLength={80}
                        onChange={(event) => setRelationshipType(event.target.value)}
                        placeholder="e.g. SUPPORTS"
                      />
                    </label>
                    <label className="relationship-search-field">
                      <span>Find document</span>
                      <div>
                        <input
                          value={relationshipSearchQuery}
                          onChange={(event) =>
                            setRelationshipSearchQuery(event.target.value)
                          }
                          placeholder="Type a filename or meaningful fragment"
                        />
                        <button
                          className="file-action"
                          type="submit"
                          disabled={isRelationshipSearching}
                        >
                          {isRelationshipSearching ? "Searching…" : "Find"}
                        </button>
                      </div>
                    </label>
                  </form>

                  <div className="relationship-candidates">
                    {relationshipSearchResults.map((candidate) => (
                      <div className="relationship-candidate" key={candidate.documentId}>
                        <div>
                          <strong>{candidate.name}</strong>
                          <span>{candidate.path}</span>
                          <small>{stateLabel(candidate.availabilityState)}</small>
                        </div>
                        <button
                          className="file-action file-action-related"
                          type="button"
                          disabled={relationshipMutationId !== null}
                          onClick={() => void addDocumentRelationship(candidate)}
                        >
                          {relationshipMutationId === candidate.documentId
                            ? "Linking…"
                            : "Link"}
                        </button>
                      </div>
                    ))}
                    {!isRelationshipSearching &&
                    relationshipSearchQuery.trim() &&
                    !relationshipSearchResults.length ? (
                      <div className="empty-result">
                        No candidate documents matched this relationship search.
                      </div>
                    ) : null}
                  </div>
                </section>
              </div>

              <footer className="text-preview-footer">
                <span>
                  Relationship creation and removal are recorded in the audit trail.
                </span>
                <span>
                  Closing this panel returns to the current viewer without changing its context.
                </span>
              </footer>
            </section>
          </div>
        ) : null}
      </main>
    </div>
  );
}
