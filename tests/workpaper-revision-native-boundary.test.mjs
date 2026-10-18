/**
 * Temporary native-source tripwire: the speculative authoring preflight
 * MUST NOT compile into installed Rust binaries, be referenced by other
 * native modules or become a callable Tauri route without DEC-03/04/06/08.
 *
 * This is not a Rust parser, threat-model proof, sandbox, identity test, or
 * security approval. Future reviewed implementation must deliberately
 * replace this guard, not simply bypass it to mount a draft Save.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";

const repoRoot = resolve(".");
const nativeDirectory = "src-tauri/src";
const moduleName = "workpaper_revision_authorization_contract";
const moduleFile = moduleName + ".rs";
const testOnlyRegistration = "#[cfg(test)]\nmod " + moduleName + ";";

function rustSources(root) {
  const dir = join(root, nativeDirectory);
  const work = [dir];
  const paths = [];
  while (work.length > 0) {
    const next = work.pop();
    for (const entry of readdirSync(next, { withFileTypes: true })) {
      const file = join(next, entry.name);
      if (entry.isDirectory()) work.push(file);
      if (entry.isFile() && entry.name.endsWith(".rs")) paths.push(file);
    }
  }
  return paths;
}

function assertNativeAuthorizationIsTestOnly(root) {
  const libPath = join(root, nativeDirectory, "lib.rs");
  const declaration = readFileSync(libPath, "utf8");
  const prototypePath = join(root, nativeDirectory, moduleFile);
  const prototype = readFileSync(prototypePath, "utf8");
  assert.ok(prototype.includes("DenyAllWorkpaperAuthoringPolicy"),
    "MISSING_NATIVE_DENY_ALL_PROTOTYPE");
  assert.ok(prototype.includes("#[cfg(test)]"),
    "MISSING_TEST_ONLY_NATIVE_FIXTURES");
  assert.equal(
    declaration.split(testOnlyRegistration).length - 1,
    1,
    "UNREVIEWED_PRODUCTION_AUTHORING_MODULE_REGISTRATION",
  );
  // Reject any other named reference anywhere in the shipping Rust source.
  // This intentionally denies even a commented reference until reviewed;
  // only the exact cfg(test) declaration is an exception.
  for (const file of rustSources(root)) {
    if (file === prototypePath) continue;
    const source = readFileSync(file, "utf8");
    const withoutAllowedDeclaration = file === libPath
      ? source.replace(testOnlyRegistration, "")
      : source;
    assert.equal(
      withoutAllowedDeclaration.includes(moduleName),
      false,
      "UNREVIEWED_NATIVE_AUTHORING_REFERENCE: " + relative(root, file),
    );
  }
  return rustSources(root).length;
}

function fixture(t, lib, extra = {}) {
  const root = mkdtempSync(join(tmpdir(), "pdox-native-auth-boundary-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const files = {
    "src-tauri/src/lib.rs": lib,
    ["src-tauri/src/" + moduleFile]:
      "#[cfg(test)]\nstruct Fixture;\nstruct DenyAllWorkpaperAuthoringPolicy;\n",
    ...extra,
  };
  for (const [name, data] of Object.entries(files)) {
    const target = join(root, name);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, data);
  }
  return root;
}

test("live Rust prototype is test-only, absent from ordinary native builds", () => {
  assert.ok(assertNativeAuthorizationIsTestOnly(repoRoot) > 2);
});

test("tripwire rejects unguarded module registration", t => {
  const root = fixture(t, "mod " + moduleName + ";\n");
  assert.throws(() => assertNativeAuthorizationIsTestOnly(root),
    /UNREVIEWED_PRODUCTION_AUTHORING_MODULE_REGISTRATION/);
});

test("tripwire rejects feature-flagged rather than test-only registration", t => {
  const root = fixture(t, "#[cfg(feature = \"experimental\")]\nmod " + moduleName + ";\n");
  assert.throws(() => assertNativeAuthorizationIsTestOnly(root),
    /UNREVIEWED_PRODUCTION_AUTHORING_MODULE_REGISTRATION/);
});

test("tripwire rejects a second production module reference in lib.rs", t => {
  const root = fixture(t, testOnlyRegistration +
    "\nuse crate::" + moduleName + "::check_workpaper_revision_authoring_preflight;\n");
  assert.throws(() => assertNativeAuthorizationIsTestOnly(root),
    /UNREVIEWED_NATIVE_AUTHORING_REFERENCE/);
});

test("tripwire rejects transitive native reference from another Rust module", t => {
  const root = fixture(t, testOnlyRegistration + "\nmod workaround;\n", {
    "src-tauri/src/workaround.rs":
      "use crate::" + moduleName + "::VerifiedNativeWorkpaperSession;\n",
  });
  assert.throws(() => assertNativeAuthorizationIsTestOnly(root),
    /UNREVIEWED_NATIVE_AUTHORING_REFERENCE/);
});

test("tripwire permits unrelated native code and the single cfg(test) declaration", t => {
  const root = fixture(t, "mod standalone;\n" + testOnlyRegistration + "\n", {
    "src-tauri/src/standalone.rs": "pub(crate) fn unrelated() {}\n",
  });
  assert.equal(assertNativeAuthorizationIsTestOnly(root), 3);
});

test("native successor candidate never compiles into shipping binaries", () => {
  const candidate = "workpaper_revision_successor_candidate";
  const lib = readFileSync(join(repoRoot, nativeDirectory, "lib.rs"), "utf8");
  const declaration = "#[cfg(test)]\\nmod " + candidate + ";";
  assert.equal(lib.split(declaration).length - 1, 1, "UNREVIEWED_CANDIDATE_REGISTRATION");
  for (const file of rustSources(repoRoot)) {
    const path = relative(repoRoot, file).replaceAll("\\\\", "/");
    if (path === "src-tauri/src/workpaper_revision_successor_candidate.rs" ||
        path === "src-tauri/src/workpaper_revision_native_text_contract_tests.rs") continue;
    const source = readFileSync(file, "utf8");
    const remainder = path === "src-tauri/src/lib.rs"
      ? source.replace(declaration, "") : source;
    assert.ok(!remainder.includes(candidate),
      "UNREVIEWED_PRODUCTION_CANDIDATE_REFERENCE: " + path);
  }
  const moduleText = readFileSync(join(repoRoot, nativeDirectory, candidate + ".rs"), "utf8");
  assert.match(moduleText, /CANDIDATE_NATIVE_AUTHORITY_UNAVAILABLE/);
  assert.match(moduleText, /authority\\.check_under_write_lock\\(&tx, workpaper_id\\)/);
});
