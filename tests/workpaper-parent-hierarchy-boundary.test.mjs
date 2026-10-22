/**
 * Source-only review tripwire for adjacent parent-hierarchy creators.
 * This does NOT call installed Tauri commands or prove a principal was denied.
 * The three commands are intentionally OUTSIDE the 34-case workpaper ledger.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const lib = readFileSync("src-tauri/src/lib.rs", "utf8");
const persistence = readFileSync("src-tauri/src/persistence.rs", "utf8");
const contract = readFileSync(
  "docs/architecture/WORKPAPER_PARENT_HIERARCHY_AUTHORITY_DEPENDENCY_DRAFT.md",
  "utf8",
);
const matrix = JSON.parse(readFileSync(
  "docs/architecture/WORKPAPER_DIRECT_TAURI_DENIAL_ACCEPTANCE_MATRIX_DRAFT.json",
  "utf8",
));
const parents = [
  "create_engagement",
  "create_engagement_area",
  "create_procedure",
];

function functionSource(source, name, exported) {
  const prefix = exported ? "pub fn " : "fn ";
  const start = source.indexOf(prefix + name + "(");
  assert.ok(start >= 0, "HIERARCHY_NATIVE_FUNCTION_MISSING: " + name);
  const next = source.indexOf(exported ? "\npub fn " : "\n#[tauri::command]", start + 1);
  return source.slice(start, next < 0 ? undefined : next);
}

function assertParentScopeChecks(source) {
  const area = functionSource(source, "create_engagement_area", true);
  const procedure = functionSource(source, "create_procedure", true);
  assert.match(area, /parent_engagement\.as_deref\(\) != Some\(engagement_id\)/,
    "HIERARCHY_AREA_CROSS_ENGAGEMENT_CHECK_MISSING");
  assert.match(procedure, /area_engagement\.as_deref\(\) != Some\(engagement_id\)/,
    "HIERARCHY_PROCEDURE_CROSS_ENGAGEMENT_CHECK_MISSING");
  for (const name of parents) {
    const fn = functionSource(source, name, true);
    assert.match(fn, /TransactionBehavior::Immediate/,
      "HIERARCHY_TRANSACTION_BOUNDARY_DRIFT: " + name);
    assert.match(fn, /archived_at_ms IS NULL/,
      "HIERARCHY_LIVE_PARENT_CHECK_DRIFT: " + name);
  }
}

test("three creator handlers remain registered with no verified principal argument", () => {
  const registry = lib.match(/tauri::generate_handler!\s*\[([\s\S]*?)\]/);
  assert.ok(registry, "TAURI_REGISTRY_MISSING");
  const names = new Set(registry[1].split(",").map(x => x.trim()));
  for (const name of parents) {
    assert.ok(names.has(name), "PARENT_HANDLER_REGISTERED_STATE_DRIFT: " + name);
    assert.match(lib, new RegExp("#\\[tauri::command\\]\\s*fn\\s+" + name + "\\s*\\("));
    const fn = functionSource(lib, name, false);
    assert.match(fn, /persistence::create_/);
    assert.doesNotMatch(fn, /verified_session|trusted_principal|authenticated_grant/,
      "PARENT_AUTHORITY_CHANGE_NEEDS_INDEPENDENT_REVIEW: " + name);
  }
});

test("parent lineage and SQLite writer lock source controls require review on drift", () => {
  assertParentScopeChecks(persistence);
  const mutation = persistence.replace(
    "parent_engagement.as_deref() != Some(engagement_id)",
    "false",
  );
  assert.notEqual(mutation, persistence, "LINEAGE_TRIPWIRE_MUTATION_MISSING");
  assert.throws(() => assertParentScopeChecks(mutation),
    /HIERARCHY_AREA_CROSS_ENGAGEMENT_CHECK_MISSING/);
});

test("native hierarchy regressions remain test-only; deferred security is unrun", () => {
  assert.match(lib,
    /#\[cfg\(test\)\]\s*mod workpaper_parent_hierarchy_native_tests;/);
  const native = readFileSync(
    "src-tauri/src/workpaper_parent_hierarchy_native_tests.rs", "utf8",
  );
  assert.match(native, /initialize_database\(&path\)/);
  assert.match(native, /legacy_area_creation_rejects_cross_engagement_parent_without_writes/);
  assert.match(native, /legacy_procedure_creation_rejects_foreign_area_without_writes/);
  assert.match(native, /legacy_hierarchy_creation_rejects_archived_ancestors_atomically/);
  assert.match(contract, /NOT IMPLEMENTED \/ NOT RUN/);
  assert.ok(parents.every(name => !matrix.routes.some(route => route.name === name)),
    "PARENT_HANDLER_PLAN_MUST_BE_REVIEWED_SEPARATELY");
  assert.equal(matrix.routes.length, 9);
  assert.equal(matrix.cases.length, 34);
  assert.ok(matrix.cases.every(c => c.execution === "NOT_IMPLEMENTED" && c.result === "NOT_RUN"));
  assert.equal(matrix.liveTauriCommandDenialHarness, "NOT_IMPLEMENTED");
});

const alternativeCreators = [
  "create_client",
  "create_service_type",
  "create_engagement_template_from_engagement",
  "create_engagement_template_version_from_engagement",
  "create_engagement_from_template",
];

test("alternative template-derived parent routes remain registered and unauthenticated", () => {
  const registry = lib.match(/tauri::generate_handler!\s*\[([\s\S]*?)\]/);
  assert.ok(registry, "TAURI_REGISTRY_MISSING");
  const names = new Set(registry[1].split(",").map(x => x.trim()));
  for (const name of alternativeCreators) {
    assert.ok(names.has(name), "ALTERNATIVE_PARENT_ROUTE_REGISTRATION_DRIFT: " + name);
    const fn = functionSource(lib, name, false);
    assert.match(fn, /persistence::create_/,
      "ALTERNATIVE_PARENT_PERSISTENCE_FORWARDING_DRIFT: " + name);
    assert.doesNotMatch(fn, /verified_session|trusted_principal|authenticated_grant/,
      "ALTERNATIVE_PARENT_AUTHORIZATION_REVIEW_REQUIRED: " + name);
  }
});

test("template instantiation retains writer lock, live parent checks and orphan rollback", () => {
  const fn = functionSource(persistence, "create_engagement_from_template", true);
  assert.match(fn, /TransactionBehavior::Immediate/,
    "TEMPLATE_INSTANTIATION_WRITER_LOCK_DRIFT");
  assert.match(fn, /FROM clients[\s\S]*archived_at_ms IS NULL/,
    "TEMPLATE_DESTINATION_CLIENT_LIVE_CHECK_MISSING");
  assert.match(fn, /FROM engagement_template_versions v[\s\S]*t\.archived_at_ms IS NULL/,
    "TEMPLATE_VERSION_LIVE_CHECK_MISSING");
  assert.match(fn, /FROM service_types[\s\S]*archived_at_ms IS NULL/,
    "TEMPLATE_SERVICE_LIVE_CHECK_MISSING");
  assert.match(fn, /next_pending\.len\(\) == pending_count/,
    "TEMPLATE_PARENT_CYCLE_REJECTION_MISSING");
  assert.match(fn, /engagement template procedure area is unavailable/,
    "TEMPLATE_PROCEDURE_PARENT_REJECTION_MISSING");
  assert.match(fn, /transaction\.commit\(\)\?/,
    "TEMPLATE_COMMIT_BOUNDARY_DRIFT");

  const mutated = persistence.replace(
    "if next_pending.len() == pending_count",
    "if false",
  );
  assert.notEqual(mutated, persistence, "TEMPLATE_MUTATION_ANCHOR_MISSING");
  assert.doesNotMatch(
    functionSource(mutated, "create_engagement_from_template", true),
    /next_pending\.len\(\) == pending_count/,
    "TEMPLATE_MUTATION_TRIPWIRE_DID_NOT_DETECT_REMOVAL",
  );
});

test("client/service + template real-migration tests are nonshipping; Tauri denials remain unrun", () => {
  assert.match(lib,
    /#\[cfg\(test\)\]\s*mod workpaper_template_hierarchy_native_tests;/);
  const native = readFileSync(
    "src-tauri/src/workpaper_template_hierarchy_native_tests.rs", "utf8",
  );
  for (const name of [
    "legacy_client_and_service_provisioning_has_no_issuer_input",
    "legacy_template_clone_reparents_nested_areas_across_clients_without_copying_workpapers",
    "legacy_template_clone_rolls_back_staged_engagement_on_invalid_definition",
    "legacy_template_version_rejects_foreign_service_type_without_new_version",
  ]) {
    assert.ok(native.includes("fn " + name + "()"),
      "ALTERNATIVE_PARENT_NATIVE_TEST_MISSING: " + name);
  }
  assert.match(native, /initialize_database\(&path\)/);
  assert.ok(alternativeCreators.every(name => !matrix.routes.some(r => r.name === name)),
    "NEW_HIERARCHY_MUTATIONS_ARE_SEPARATE_FROM_WORKPAPER_MATRIX");
  assert.equal(matrix.routes.length, 9);
  assert.equal(matrix.cases.length, 34);
  assert.ok(matrix.cases.every(c =>
    c.execution === "NOT_IMPLEMENTED" && c.result === "NOT_RUN"));
  assert.equal(matrix.liveTauriCommandDenialHarness, "NOT_IMPLEMENTED");
  assert.match(contract, /NOT IMPLEMENTED \/ NOT RUN/);
});
