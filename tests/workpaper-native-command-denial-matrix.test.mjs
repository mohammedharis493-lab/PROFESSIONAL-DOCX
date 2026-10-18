/**
 * SOURCE-ONLY INVENTORY TRIPWIRE for the deferred direct-Tauri authorization
 * acceptance matrix. Passing this suite is NOT a Tauri IPC invocation, an
 * authorization denial test, a security approval or proof of safe exposure.
 *
 * An approved L1-L4 decision MUST update the ledger and replace the unrun
 * cases with real denial/atomicity tests against registered native handlers.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const lib = readFileSync("src-tauri/src/lib.rs", "utf8");
const app = readFileSync("src/App.tsx", "utf8");
const matrix = JSON.parse(readFileSync(
  "docs/architecture/WORKPAPER_DIRECT_TAURI_DENIAL_ACCEPTANCE_MATRIX_DRAFT.json",
  "utf8",
));
const expected = [
  "create_workpaper_revision",
  "create_workpaper_evidence_link",
  "create_workpaper_signoff",
  "transition_workpaper_state",
  "create_review_note",
  "respond_to_review_note",
  "clear_review_note",
  "reopen_review_note",
];
const universalScenarios = [
  "MISSING_TRUSTED_PRINCIPAL",
  "WRONG_WORKPAPER_GRANT",
  "REVOKED_OR_EXPIRED_GRANT",
];

function commandRegistry(source) {
  const match = source.match(/tauri::generate_handler!\s*\[([\s\S]*?)\]/);
  assert.ok(match, "NO_NATIVE_COMMAND_REGISTRY");
  return new Set(match[1]
    .replace(/\/\/[^\n]*/g, "")
    .split(",")
    .map(name => name.trim())
    .filter(Boolean));
}

function assertExpectedRegistered(source, names = expected) {
  const routes = commandRegistry(source);
  for (const name of names) {
    assert.ok(routes.has(name), "LEGACY_NATIVE_ROUTE_DRIFT_REQUIRES_REVIEW: " + name);
    assert.match(source,
      new RegExp("#\\[tauri::command\\]\\s*fn\\s+" + name + "\\s*\\("),
      "NO_DECORATED_HANDLER: " + name);
  }
  assert.ok(!routes.has("candidate_successor_transaction"),
    "UNREVIEWED_TEST_ONLY_SUCCESSOR_ROUTE");
  assert.ok(!routes.has("test_only_checked_successor"),
    "UNREVIEWED_TEST_ONLY_HELPER_ROUTE");
  return routes;
}

function assertUnexecutedMatrix(plan) {
  assert.equal(plan.schema, "professional-docx-native-command-denial-plan/v1");
  assert.equal(plan.status, "DRAFT_NO_SECURITY_APPROVAL");
  assert.equal(plan.selectedContainmentOption, null);
  assert.equal(plan.independentHumanReview, "UNAVAILABLE_NOT_PERFORMED");
  assert.equal(plan.liveTauriCommandDenialHarness, "NOT_IMPLEMENTED");
  assert.equal(plan.principalIssuer, "NOT_IMPLEMENTED");
  assert.equal(plan.workpaperGrants, "NOT_IMPLEMENTED");
  assert.deepEqual(plan.governanceDecisions,
    Array.from({ length: 8 }, (_, i) => "DEC-0" + (i + 1)));
  assert.ok(plan.globalAtomicityInvariants.length >= 4);
  assert.deepEqual(plan.routes.map(route => route.name), expected);
  assert.equal(new Set(plan.cases.map(item => item.id)).size, plan.cases.length,
    "DUPLICATED_DENIAL_VECTOR");
  for (const route of plan.routes) {
    assert.ok(route.requiredGrant, "MISSING_UNDECIDED_GRANT_CATEGORY: " + route.name);
    assert.ok(Array.isArray(route.actorInputs));
    const vectors = plan.cases.filter(item => item.command === route.name);
    for (const scenario of universalScenarios) {
      assert.equal(vectors.filter(item => item.scenario === scenario).length, 1,
        "MISSING_DEFERRED_DENIAL_VECTOR: " + route.name + "/" + scenario);
    }
    const spoof = vectors.filter(item =>
      item.scenario === "SPOOFED_PAYLOAD_ATTRIBUTION");
    assert.equal(spoof.length, route.actorInputs.length ? 1 : 0,
      "ATTRIBUTION_VECTOR_SCOPE_DRIFT: " + route.name);
  }
  for (const item of plan.cases) {
    assert.ok(expected.includes(item.command), "UNKNOWN_COMMAND: " + item.command);
    assert.ok(item.expected.startsWith("DENY"), "NON_DENIAL_EXPECTATION: " + item.id);
    assert.equal(item.execution, "NOT_IMPLEMENTED",
      "FALSE_CLAIM_DIRECT_NATIVE_TEST_IMPLEMENTED: " + item.id);
    assert.equal(item.result, "NOT_RUN",
      "FALSE_CLAIM_DIRECT_NATIVE_TEST_VERIFIED: " + item.id);
    assert.ok(item.precondition, "MISSING_TEST_PRECONDITION: " + item.id);
  }
  assert.ok(plan.cases.length >= 30);
}

test("denial matrix is explicit, unapproved and all planned vectors are unrun", () => {
  assertUnexecutedMatrix(matrix);
});

test("all eight current registered native writers match the documented risk inventory", () => {
  const registered = assertExpectedRegistered(lib);
  assert.ok(registered.has("list_workpaper_revisions"));
  assert.ok(registered.has("list_review_notes"));
  assert.ok(registered.has("list_review_note_events"));
});

test("native DTOs and installed frontend still expose the documented caller-attribution gap", () => {
  assert.match(lib, /struct WorkpaperRevisionInputDto\s*\{/);
  assert.match(lib, /#\[serde\(default\)\]\s*initial_only:\s*bool/);
  assert.match(lib, /if revision\.initial_only/);
  assert.match(lib, /persistence::create_initial_workpaper_revision/);
  assert.match(lib, /persistence::create_workpaper_revision/);
  assert.match(lib, /struct WorkpaperSignoffInputDto\s*\{[^}]*actor_id:\s*String,[^}]*actor_role:\s*String/s);
  assert.match(lib, /struct ReviewNoteInputDto\s*\{[^}]*owner_id:\s*Option<String>,[^}]*raised_by:\s*Option<String>/s);
  assert.match(lib, /struct ReviewNoteActionInputDto\s*\{[^}]*actor_id:\s*Option<String>/s);
  assert.match(lib, /fn review_note_action<'a>\(/);
  assert.match(app, /invoke<WorkpaperRevision>\("create_workpaper_revision"/);
  assert.match(app, /initialOnly:\s*true/);
  assert.match(app, /invoke<ReviewNoteEvent>\("respond_to_review_note"/);
  assert.match(app, /changeReviewNoteState\("clear_review_note"\)/);
  assert.match(app, /changeReviewNoteState\("reopen_review_note"\)/);
});

test("source-tripwire detects a renamed or unregistered legacy native entry", () => {
  const mutated = lib.replace(
    /(?<=\s)clear_review_note,(?=\s)/,
    "clear_review_note_REMOVED,",
  );
  assert.notEqual(mutated, lib, "TEST_MUTATION_DID_NOT_REMOVE_ROUTE");
  assert.throws(() => assertExpectedRegistered(mutated),
    /LEGACY_NATIVE_ROUTE_DRIFT_REQUIRES_REVIEW: clear_review_note/);
});

test("source-tripwire detects accidental registration of a fixture-only successor", () => {
  const needle = "list_workpaper_revisions,";
  assert.ok(lib.includes(needle), "TEST_MUTATION_MISSING_INSERTION_POINT");
  const mutated = lib.replace(
    needle, needle + "\n            candidate_successor_transaction,");
  assert.throws(() => assertExpectedRegistered(mutated),
    /UNREVIEWED_TEST_ONLY_SUCCESSOR_ROUTE/);
});

test("matrix review-tripwire rejects fictitious security approval or fake passing tests", () => {
  assert.throws(() => assertUnexecutedMatrix({
    ...matrix,
    selectedContainmentOption: "L4",
  }));
  assert.throws(() => assertUnexecutedMatrix({
    ...matrix,
    cases: matrix.cases.map((item, i) => i === 0
      ? { ...item, execution: "IMPLEMENTED", result: "PASSED" } : item),
  }), /FALSE_CLAIM_DIRECT_NATIVE_TEST_IMPLEMENTED/);
});
