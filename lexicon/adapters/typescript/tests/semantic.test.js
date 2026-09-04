const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const { buildFacts } = require("../dist/orchestration");

test("emits normalized error-handling capabilities and actions", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "lexicon-ts-semantic-"));
  fs.writeFileSync(path.join(root, "tsconfig.json"), JSON.stringify({ compilerOptions: { target: "ES2022" } }));
  fs.mkdirSync(path.join(root, "src"));
  fs.writeFileSync(path.join(root, "src", "main.ts"), [
    "declare function work(): void;",
    "export function swallowed(): void { try { work(); } catch (error) {} }",
    "export function propagated(): void { try { work(); } catch (error) { throw error; } }",
    "export function recorded(): void { try { work(); } catch (error) { console.error(error); } }",
    "export function recovered(): number { try { work(); return 1; } catch (error) { return 0; } }",
    "",
  ].join("\n"));

  const records = buildFacts(root);
  const nodes = records.filter((record) => record.record === "node");
  const edges = records.filter((record) => record.record === "edge");
  const capabilities = nodes.filter((node) => node.kind === "protocol" && String(node.name).startsWith("semantic-capabilities:typescript:"));
  const handlers = nodes.filter((node) => node.kind === "protocol" && node.name === "error-handler:typescript");
  const actions = nodes.filter((node) => node.kind === "protocol" && String(node.name).startsWith("error-action:"));

  assert.equal(capabilities.length, 1);
  assert.equal(capabilities[0].name, "semantic-capabilities:typescript:control-flow,error-handling,calls,source-spans,outcome-obligations");
  assert.equal(handlers.length, 4);
  assert.deepEqual(new Set(actions.map((node) => node.name)), new Set(["error-action:propagate", "error-action:record", "error-action:recover"]));
  const actionIds = new Set(actions.map((node) => node.id));
  assert.equal(edges.filter((edge) => edge.relation === "contains" && actionIds.has(edge.target)).length, 3);
});
