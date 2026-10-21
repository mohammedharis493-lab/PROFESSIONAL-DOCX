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
