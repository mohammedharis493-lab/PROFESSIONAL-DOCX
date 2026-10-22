/**
 * SOURCE-BOUNDARY REGRESSION ONLY: do not accidentally mount the untrusted
 * seven-field editor or connect it to legacy Tauri writing/sign-off commands.
 *
 * This tests literal JS/TS/TSX module imports from the frontend entrypoint
 * and from the editor modules. It is NOT a security sandbox, trusted signer,
 * authorization policy, runtime proof or a review of arbitrary native calls.
 * A deliberate future activation MUST replace this guard with a separately
 * reviewed, identity- and permission-aware integration contract.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, extname, isAbsolute, join, relative, resolve } from "node:path";
import ts from "typescript";

const repoRoot = resolve(".");
const frontendEntry = "src/main.tsx";
const detachedEditor = [
  "src/WorkpaperRevisionDraftPanel.tsx",
  "src/workpaper-revision-edit-draft.ts",
  "src/workpaper-revision-read-snapshot.ts",
  "src/workpaper-revision-conflict-inspection.ts",
];
const executableExts = [".ts", ".tsx", ".js", ".jsx", ".mjs", ".mts"];
const assetExts = new Set([".css", ".scss", ".svg", ".png", ".jpg", ".jpeg", ".webp", ".woff", ".woff2"]);
function relativeUnix(root, absolutePath) {
  return relative(root, absolutePath).split("\\").join("/");
}
function scriptKind(filename) {
  return filename.endsWith(".tsx") || filename.endsWith(".jsx")
    ? ts.ScriptKind.TSX
    : filename.endsWith(".js") || filename.endsWith(".mjs")
      ? ts.ScriptKind.JS
      : ts.ScriptKind.TS;
}
function importSpecs(text, filename) {
  const ast = ts.createSourceFile(filename, text, ts.ScriptTarget.Latest, true, scriptKind(filename));
  if (ast.parseDiagnostics.length > 0) {
    throw new Error("INVALID_FRONTEND_MODULE_SYNTAX: " + filename);
  }
  const specs = [];
  function visit(node) {
    if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier) {
      if (!ts.isStringLiteralLike(node.moduleSpecifier)) {
        throw new Error("NON_LITERAL_IMPORT: " + filename);
      }
      specs.push(node.moduleSpecifier.text);
    }
    if (ts.isImportEqualsDeclaration(node) && ts.isExternalModuleReference(node.moduleReference)) {
      const expression = node.moduleReference.expression;
      if (!expression || !ts.isStringLiteralLike(expression)) {
        throw new Error("NON_LITERAL_IMPORT_EQUALS: " + filename);
      }
      specs.push(expression.text);
    }
    if (ts.isCallExpression(node)) {
      const dynamicImport = node.expression.kind === ts.SyntaxKind.ImportKeyword;
      const requireCall = ts.isIdentifier(node.expression) && node.expression.text === "require";
      const metaGlob = ts.isPropertyAccessExpression(node.expression) &&
        (node.expression.name.text === "glob" || node.expression.name.text === "globEager") &&
        ts.isMetaProperty(node.expression.expression);
      if (dynamicImport || requireCall || metaGlob) {
        if (metaGlob || node.arguments.length !== 1 ||
            !ts.isStringLiteralLike(node.arguments[0])) {
          throw new Error("UNREVIEWED_DYNAMIC_MODULE_DISCOVERY: " + filename);
        }
        specs.push(node.arguments[0].text);
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(ast);
  return { specs, ast };
}
function resolveRelative(root, fromFile, spec) {
  if (!spec.startsWith(".")) return null;
  const base = resolve(dirname(fromFile), spec);
  const rel = relativeUnix(root, base);
  if (rel === ".." || rel.startsWith("../") || isAbsolute(rel)) {
    throw new Error("IMPORT_OUTSIDE_REVIEWED_FRONTEND_ROOT: " + spec);
  }
  if (assetExts.has(extname(base))) return null;
  const choices = extname(base) && executableExts.includes(extname(base))
    ? [base]
    : [base, ...executableExts.map(ext => base + ext),
      ...executableExts.map(ext => join(base, "index" + ext))];
  const path = choices.find(existsSync);
  if (!path) {
    throw new Error("UNRESOLVED_FRONTEND_MODULE: " + spec);
  }
  return path;
}
function analyzeGraph(root, entry) {
  const absoluteRoot = resolve(root);
  const pending = [resolve(absoluteRoot, entry)];
  const reachable = new Set();
  const external = new Set();
  const sourceByFile = new Map();
  for (; pending.length > 0;) {
    const path = pending.pop();
    const name = relativeUnix(absoluteRoot, path);
    if (reachable.has(name)) continue;
    if (!existsSync(path)) throw new Error("MISSING_FRONTEND_ENTRY: " + name);
    reachable.add(name);
    const source = readFileSync(path, "utf8");
    sourceByFile.set(name, source);
    for (const spec of importSpecs(source, name).specs) {
      const dep = resolveRelative(absoluteRoot, path, spec);
      if (dep) pending.push(dep);
      else if (!spec.startsWith(".")) external.add(spec);
    }
  }
  return { reachable, external, sourceByFile };
}
function assertUnmounted(root) {
  const reachable = analyzeGraph(root, frontendEntry).reachable;
  for (const name of detachedEditor) {
    assert.equal(reachable.has(name), false, "UNREVIEWED_EDITOR_MOUNT: " + name);
  }
  return reachable;
}
function assertEditorCannotWrite(root) {
  for (const name of detachedEditor) {
    const graph = analyzeGraph(root, name);
    assert.equal(graph.reachable.has("src/App.tsx"), false, "EDITOR_IMPORTS_LEGACY_APP: " + name);
    for (const spec of graph.external) {
      assert.equal(spec.startsWith("@tauri-apps/"), false,
        "EDITOR_IMPORTS_TAURI: " + name + " -> " + spec);
    }
    for (const [file, text] of graph.sourceByFile) {
      const ast = importSpecs(text, file).ast;
      function scan(node) {
        if (ts.isCallExpression(node) && (
          ts.isIdentifier(node.expression) && node.expression.text === "invoke" ||
          ts.isPropertyAccessExpression(node.expression) &&
            ["invoke", "execute", "query"].includes(node.expression.name.text)
        )) {
          throw new Error("EDITOR_CALLS_WRITE_OR_NATIVE_API: " + file);
        }
        if (ts.isIdentifier(node) && ["__TAURI__", "__TAURI_INTERNALS__"].includes(node.text)) {
          throw new Error("EDITOR_REFERENCES_TAURI_GLOBAL: " + file);
        }
        ts.forEachChild(node, scan);
      }
      scan(ast);
    }
  }
}
function temporaryProject(t, entries) {
  const root = mkdtempSync(join(tmpdir(), "pdx-editor-isolation-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const [filename, content] of Object.entries(entries)) {
    const full = join(root, filename);
    mkdirSync(dirname(full), { recursive: true });
    writeFileSync(full, content);
  }
  return root;
}
function detachedStubs() {
  return Object.fromEntries(detachedEditor.map(path => [path, "export const harmless = true;\n"]));
}

test("live frontend entrypoint cannot reach the isolated seven-field revision editor", () => {
  const live = assertUnmounted(repoRoot);
  assert.ok(live.has("src/main.tsx"));
  assert.ok(live.has("src/App.tsx"));
  assert.ok(live.size >= 2);
});

test("every isolated editor module is free from reachable Tauri native writing/imports", () => {
  assertEditorCannotWrite(repoRoot);
});

test("guard rejects a direct import of the detached editor into the running app", t => {
  const root = temporaryProject(t, {
    ...detachedStubs(),
    "src/main.tsx": "import './WorkpaperRevisionDraftPanel';\n",
  });
  assert.throws(() => assertUnmounted(root), /UNREVIEWED_EDITOR_MOUNT/);
});

test("guard rejects transitive import through another local frontend component", t => {
  const root = temporaryProject(t, {
    ...detachedStubs(),
    "src/main.tsx": "import './screen';\n",
    "src/screen.tsx": "import './WorkpaperRevisionDraftPanel';\n",
  });
  assert.throws(() => assertUnmounted(root), /UNREVIEWED_EDITOR_MOUNT/);
});

test("guard rejects re-exporting detached editor through a barrel", t => {
  const root = temporaryProject(t, {
    ...detachedStubs(),
    "src/main.tsx": "import './reexports';\n",
    "src/reexports.ts": "export { harmless } from './workpaper-revision-edit-draft';\n",
  });
  assert.throws(() => assertUnmounted(root), /UNREVIEWED_EDITOR_MOUNT/);
});

test("guard rejects dynamic import of the prototype from production", t => {
  const root = temporaryProject(t, {
    ...detachedStubs(),
    "src/main.tsx": "void import('./WorkpaperRevisionDraftPanel');\n",
  });
  assert.throws(() => assertUnmounted(root), /UNREVIEWED_EDITOR_MOUNT/);
});

test("guard rejects unreviewable variable module loading rather than guessing it safe", t => {
  const root = temporaryProject(t, {
    ...detachedStubs(),
    "src/main.tsx": "const spec = './WorkpaperRevisionDraftPanel'; void import(spec);\n",
  });
  assert.throws(() => assertUnmounted(root), /UNREVIEWED_DYNAMIC_MODULE_DISCOVERY/);
});

test("guard still allows unrelated frontend imports and harmless asset CSS", t => {
  const root = temporaryProject(t, {
    ...detachedStubs(),
    "src/main.tsx": "import './App'; import './style.css';\n",
    "src/App.tsx": "export const page = 'normal';\n",
    "src/style.css": "body { display: block; }\n",
  });
  assert.deepEqual([...assertUnmounted(root)].sort(), ["src/App.tsx", "src/main.tsx"]);
  assertEditorCannotWrite(root);
});

test("guard rejects a native Tauri import inside the detached panel", t => {
  const root = temporaryProject(t, {
    ...detachedStubs(),
    "src/WorkpaperRevisionDraftPanel.tsx":
      "import { invoke } from '@tauri-apps/api/core'; export const go = () => invoke('create_workpaper_revision');\n",
  });
  assert.throws(() => assertEditorCannotWrite(root), /EDITOR_IMPORTS_TAURI/);
});

test("guard rejects indirect Tauri import from a nominally pure editor helper", t => {
  const root = temporaryProject(t, {
    ...detachedStubs(),
    "src/workpaper-revision-edit-draft.ts": "export { go } from './bridge';\n",
    "src/bridge.ts": "import { invoke } from '@tauri-apps/api/core'; export const go = invoke;\n",
  });
  assert.throws(() => assertEditorCannotWrite(root), /EDITOR_IMPORTS_TAURI/);
});

test("guard rejects a hidden global Tauri invoke without an explicit import", t => {
  const root = temporaryProject(t, {
    ...detachedStubs(),
    "src/workpaper-revision-read-snapshot.ts":
      "const globalNative = __TAURI_INTERNALS__; export const leak = globalNative;\n",
  });
  assert.throws(() => assertEditorCannotWrite(root), /EDITOR_REFERENCES_TAURI_GLOBAL/);
});

test("guard rejects an aliased/native invoke call added without imports", t => {
  const root = temporaryProject(t, {
    ...detachedStubs(),
    "src/workpaper-revision-conflict-inspection.ts":
      "export const unsafe = () => invoke('create_workpaper_signoff');\n",
  });
  assert.throws(() => assertEditorCannotWrite(root), /EDITOR_CALLS_WRITE_OR_NATIVE_API/);
});
