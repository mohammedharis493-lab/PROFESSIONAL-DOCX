/**
 * Source-only tripwires for additional firm-library publication boundaries.
 * NOT direct Tauri authorization tests, authenticated approver evidence, or
 * proof an adversary can directly modify a migrated database.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const lib = readFileSync("src-tauri/src/lib.rs", "utf8");
const persistence = readFileSync("src-tauri/src/persistence.rs", "utf8");
const app = readFileSync("src/App.tsx", "utf8");
const contract = readFileSync(
  "docs/architecture/WORKPAPER_FIRM_LIBRARY_PUBLICATION_DEPENDENCY_DRAFT.md", "utf8",
);
const matrix = JSON.parse(readFileSync(
  "docs/architecture/WORKPAPER_DIRECT_TAURI_DENIAL_ACCEPTANCE_MATRIX_DRAFT.json", "utf8",
));

function implementation(name, source = persistence) {
  const prefix = "pub fn " + name + "(";
  const start = source.indexOf(prefix);
  assert.ok(start >= 0, "NATIVE_FIRM_LIBRARY_ENTRY_MISSING: " + name);
  const end = source.indexOf("\npub fn ", start + prefix.length);
  return source.slice(start, end < 0 ? undefined : end);
}

test("creation, publication and statutory consumption are registered Tauri calls", () => {
  const registry = lib.match(/tauri::generate_handler!\s*\[([\s\S]*?)\]/);
  assert.ok(registry, "TAURI_REGISTRY_MISSING");
  const handlers = new Set(registry[1].split(",").map(name => name.trim()));
  for (const name of [
    "create_firm_library_item",
    "publish_firm_library_version",
    "create_statutory_compliance_requirement",
  ]) {
    assert.ok(handlers.has(name), "FIRM_LIBRARY_REGISTERED_ROUTE_DRIFT: " + name);
    assert.match(lib, new RegExp("#\\[tauri::command\\]\\s*fn\\s+" + name + "\\s*\\("));
    assert.doesNotMatch(
      lib.slice(lib.indexOf("fn " + name + "("), lib.indexOf("\n#[tauri::command]", lib.indexOf("fn " + name + "(") + 1)),
      /verified_session|trusted_principal|authenticated_grant/,
      "FIRM_LIBRARY_AUTHORITY_CHANGE_REQUIRES_GOVERNANCE: " + name,
    );
  }
});

test("existing publication transaction, digest, immutable snapshots and audit remain inspected", () => {
  const create = implementation("create_firm_library_item");
  const publish = implementation("publish_firm_library_version");
  for (const [name, fn] of [["create", create], ["publish", publish]]) {
    assert.match(fn, /TransactionBehavior::Immediate/,
      "FIRM_LIBRARY_WRITER_LOCK_DRIFT: " + name);
    assert.match(fn, /insert_domain_audit_event/,
      "FIRM_LIBRARY_AUDIT_INSERT_DRIFT: " + name);
    assert.match(fn, /transaction\.commit\(\)\?/,
      "FIRM_LIBRARY_COMMIT_BOUNDARY_DRIFT: " + name);
  }
  assert.match(persistence, /fn normalize_firm_library_definition\(/);
  assert.match(persistence, /Sha256::digest\(canonical\.as_bytes\(\)\)/);
  assert.match(persistence, /definition\.is_object\(\)/);
  assert.match(publish, /archived_at_ms IS NULL/);
  assert.match(publish, /COALESCE\(MAX\(version_number\), 0\) \+ 1/);
  assert.doesNotMatch(publish, /expected_latest_revision|expected_latest_version|trusted_principal/,
    "PUBLISHER_CAS_OR_IDENTITY_REQUIRES_GOVERNANCE_REVIEW");
  assert.match(app, /invoke<FirmLibraryItem>\("create_firm_library_item"/);
  assert.match(app, /invoke<FirmLibraryItem>\("publish_firm_library_version"/);
  assert.match(app, /definitionJson: JSON\.stringify\(\{ content \}\)/);
});

test("statutory consumer is distinct from engagement-template clone; claimed hash needs review", () => {
  const consumer = implementation("create_statutory_compliance_requirement");
  assert.match(consumer, /FROM firm_library_versions v/);
  assert.match(consumer, /i\.category = 'STATUTORY_COMPLIANCE_REQUIREMENT'/);
  assert.match(consumer, /value != engagement_service_type_id/);
  assert.match(consumer, /definition_hash\.len\(\) != 32/);
  assert.doesNotMatch(consumer, /Sha256::digest|Sha256::new/,
    "RECOMPUTED_DIGEST_BEHAVIOR_CHANGED_REQUIRES_REVIEW");
  assert.match(consumer, /STATUTORY_COMPLIANCE_REQUIREMENT_ADDED/);
  const clone = implementation("create_engagement_from_template");
  assert.doesNotMatch(clone, /firm_library_version_id|firm_library_item_id/,
    "TEMPLATE_AND_FIRM_LIBRARY_LINKAGE_CHANGED_REQUIRES_REVIEW");
});

test("new test-only regressions do not claim 34 workpaper denials are implemented", () => {
  assert.match(lib, /#\[cfg\(test\)\]\s*mod workpaper_firm_library_native_tests;/);
  const native = readFileSync(
    "src-tauri/src/workpaper_firm_library_native_tests.rs", "utf8",
  );
  for (const name of [
    "legacy_firm_library_create_preserves_canonical_hash_audit_and_immutable_version",
    "legacy_firm_library_rejects_invalid_definition_and_foreign_service_without_writes",
    "legacy_firm_library_publish_appends_version_without_issuer_or_compare_and_swap",
    "legacy_statutory_consumer_pins_exact_library_version_and_rejects_wrong_scopes",
    "legacy_statutory_consumer_only_checks_claimed_hash_length_not_recomputed_digest",
  ]) {
    assert.ok(native.includes("fn " + name + "()"), "NATIVE_TEST_MISSING: " + name);
  }
  assert.match(native, /initialize_database\(&path\)/);
  assert.match(contract, /NOT IMPLEMENTED \/ NOT RUN/);
  assert.equal(matrix.routes.length, 9);
  assert.equal(matrix.cases.length, 34);
  assert.equal(matrix.liveTauriCommandDenialHarness, "NOT_IMPLEMENTED");
  assert.ok(matrix.cases.every(c =>
    c.execution === "NOT_IMPLEMENTED" && c.result === "NOT_RUN"));
});


test("statutory assessment writer remains registered but unbound to a verified principal", () => {
  const registry = lib.match(/tauri::generate_handler!\s*\[([\s\S]*?)\]/);
  assert.ok(registry, "TAURI_REGISTRY_MISSING");
  assert.ok(
    registry[1].split(",").map(name => name.trim())
      .includes("create_statutory_compliance_assessment"),
    "STATUTORY_ASSESSMENT_REGISTERED_ROUTE_DRIFT",
  );
  const handlerStart = lib.indexOf("fn create_statutory_compliance_assessment(");
  assert.ok(handlerStart >= 0, "STATUTORY_ASSESSMENT_HANDLER_MISSING");
  const handlerEnd = lib.indexOf("\n#[tauri::command]", handlerStart + 1);
  const handler = lib.slice(handlerStart, handlerEnd);
  assert.match(handler, /persistence::create_statutory_compliance_assessment/);
  assert.doesNotMatch(handler, /verified_session|trusted_principal|authenticated_grant/,
    "STATUTORY_ASSESSMENT_AUTHORITY_CHANGE_REQUIRES_REVIEW");
});

test("assessment snapshots evidence metadata and audit inside IMMEDIATE but not principal grants", () => {
  const assess = implementation("create_statutory_compliance_assessment");
  assert.match(assess, /TransactionBehavior::Immediate/,
    "STATUTORY_ASSESSMENT_LOCK_DRIFT");
  assert.match(assess, /FROM statutory_compliance_requirements/,
    "STATUTORY_REQUIREMENT_CHECK_MISSING");
  assert.match(assess, /FROM controlled_evidence_versions/,
    "STATUTORY_EXACT_EVIDENCE_LOOKUP_MISSING");
  assert.match(assess, /retention_state != "RETAINED"/,
    "STATUTORY_EVIDENCE_RETENTION_CHECK_MISSING");
  assert.match(assess, /verification_state != "HASH_VERIFIED"/,
    "STATUTORY_EVIDENCE_VERIFICATION_CHECK_MISSING");
  assert.match(assess, /sha256\.len\(\) != 32/,
    "STATUTORY_EVIDENCE_HASH_LENGTH_CHECK_MISSING");
  assert.match(assess, /INSERT INTO statutory_compliance_evidence_links/,
    "STATUTORY_EXACT_EVIDENCE_SNAPSHOT_MISSING");
  assert.match(assess, /STATUTORY_COMPLIANCE_ASSESSMENT_RECORDED/,
    "STATUTORY_AUDIT_EVENT_MISSING");
  assert.match(assess, /transaction\.commit\(\)\?/,
    "STATUTORY_COMMIT_DRIFT");
  assert.doesNotMatch(assess, /archived_at_ms IS NULL/,
    "STATUTORY_ARCHIVED_PARENT_BEHAVIOR_CHANGED_REQUIRES_REVIEW");
  assert.doesNotMatch(assess, /Sha256::digest|Sha256::new/,
    "STATUTORY_BYTE_HASH_RECOMPUTE_CHANGED_REQUIRES_REVIEW");
  const auditor = persistence.slice(
    persistence.indexOf("fn insert_domain_audit_event("),
    persistence.indexOf("\npub fn create_client(", persistence.indexOf("fn insert_domain_audit_event(")),
  );
  assert.match(auditor, /occurred_at_ms,\s*actor_id/);
  assert.match(auditor, /event\.occurred_at_ms,\s*event\.details\.to_string\(\)/);
  assert.match(auditor, /NULL, \?8/,
    "CURRENT_NULL_AUDIT_ACTOR_BEHAVIOR_CHANGED_REQUIRES_REVIEW");
});

test("assessment status validation and native real-migration tests stay separate from IPC denials", () => {
  const validationStart = persistence.indexOf("fn normalize_statutory_compliance_assessment(");
  const validationEnd = persistence.indexOf("\nfn ", validationStart + 1);
  assert.ok(validationStart >= 0 && validationEnd > validationStart);
  const validation = persistence.slice(validationStart, validationEnd);
  assert.match(validation, /statutory compliance evidence versions must be unique/);
  assert.match(validation, /"COMPLIANT"/);
  assert.match(validation, /"EXCEPTION"/);
  assert.match(validation, /"NOT_APPLICABLE"/);
  assert.match(lib,
    /#\[cfg\(test\)\]\s*mod workpaper_statutory_assessment_native_tests;/);
  const native = readFileSync(
    "src-tauri/src/workpaper_statutory_assessment_native_tests.rs", "utf8",
  );
  for (const name of [
    "legacy_assessment_appends_caller_claimed_exception_without_evidence_or_verified_actor",
    "legacy_assessment_invalid_state_duplicate_evidence_and_missing_evidence_do_not_write",
    "legacy_assessment_allows_new_version_after_archiving_parent_engagement",
    "legacy_assessment_version_numbers_advance_with_immutable_prior_state",
  ]) {
    assert.ok(native.includes("fn " + name + "()"), "STATUTORY_NATIVE_TEST_MISSING: " + name);
  }
  assert.match(native, /initialize_database\(&path\)/);
  assert.equal(matrix.routes.length, 9);
  assert.equal(matrix.cases.length, 34);
  assert.equal(matrix.liveTauriCommandDenialHarness, "NOT_IMPLEMENTED");
  assert.ok(matrix.cases.every(c => c.execution === "NOT_IMPLEMENTED" && c.result === "NOT_RUN"));
});


test("controlled-evidence read/archive guard diverges from statutory metadata attachment", () => {
  const read = implementation("get_controlled_evidence_read_record");
  const assess = implementation("create_statutory_compliance_assessment");
  assert.match(read, /JOIN documents d ON d\.document_id = cev\.document_id/,
    "CONTROLLED_READ_DOCUMENT_JOIN_DRIFT");
  assert.match(read, /d\.archived_at_ms IS NULL/,
    "CONTROLLED_READ_ARCHIVE_GUARD_DRIFT");
  assert.match(assess, /FROM controlled_evidence_versions/,
    "STATUTORY_EVIDENCE_METADATA_LOOKUP_DRIFT");
  assert.doesNotMatch(assess, /JOIN documents|FROM documents/,
    "STATUTORY_EVIDENCE_SOURCE_ARCHIVE_BEHAVIOR_CHANGED_REQUIRES_REVIEW");
  assert.doesNotMatch(assess, /controlled_storage_locator|controlled_bytes|read_controlled_evidence_bytes/,
    "STATUTORY_BYTE_CUSTODY_BOUNDARY_CHANGED_REQUIRES_REVIEW");
  const simulatedRegression = read.replace("AND d.archived_at_ms IS NULL", "");
  assert.notEqual(simulatedRegression, read, "READ_ARCHIVE_GUARD_MUTATION_MISSING");
  assert.doesNotMatch(simulatedRegression, /d\.archived_at_ms IS NULL/,
    "READ_ARCHIVE_GUARD_MUTATION_NOT_DETECTED");
});

test("synthetic exact-evidence regressions are native persistence tests, not captured bytes or IPC grants", () => {
  const native = readFileSync(
    "src-tauri/src/workpaper_statutory_assessment_native_tests.rs", "utf8",
  );
  for (const name of [
    "legacy_assessment_uses_controlled_metadata_snapshot_without_document_scope_grant",
    "legacy_assessment_can_attach_archived_document_hidden_by_evidence_read_path",
    "legacy_assessment_rejects_quarantined_metadata_without_partial_evidence_or_audit",
  ]) {
    assert.ok(native.includes("fn " + name + "()"),
      "STATUTORY_EXACT_EVIDENCE_NATIVE_TEST_MISSING: " + name);
  }
  assert.match(native, /fn seed_synthetic_evidence\(/);
  assert.match(native, /INSERT INTO controlled_evidence_versions/);
  assert.match(native, /INSERT INTO evidence_capture_jobs/);
  assert.match(native, /get_controlled_evidence_read_record/);
  assert.equal(matrix.routes.length, 9);
  assert.equal(matrix.cases.length, 34);
  assert.ok(matrix.cases.every(c => c.execution === "NOT_IMPLEMENTED" && c.result === "NOT_RUN"));
});
