const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const { buildFacts } = require("../dist/orchestration");

function semanticRecords(records, language) {
  const nodes = records.filter((record) => record.record === "node");
  const edges = records.filter((record) => record.record === "edge");
  const operations = nodes.filter((node) => node.name === `outcome-operation:${language}:async`);
  const actions = nodes.filter((node) => node.name === "outcome-action:consume");
  const actionIds = new Set(actions.map((node) => node.id));
  const consumed = new Set(
    edges
      .filter((edge) => edge.relation === "contains" && actionIds.has(edge.target))
      .map((edge) => edge.source),
  );
  return { nodes, operations, consumed };
}

test("emits async outcome obligations for TypeScript promise calls", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "lexicon-ts-outcome-"));
  fs.writeFileSync(path.join(root, "tsconfig.json"), JSON.stringify({ compilerOptions: { target: "ES2022" } }));
  fs.writeFileSync(path.join(root, "main.ts"), [
    "async function fetchValue(): Promise<number> { return 1; }",
    "export async function use(): Promise<number> {",
    "  fetchValue();",
    "  await fetchValue();",
    "  const pending = fetchValue();",
    "  return fetchValue();",
    "}",
    "",
  ].join("\n"));

  const { operations, consumed } = semanticRecords(buildFacts(root), "typescript");
  assert.equal(operations.length, 4);
  assert.equal(consumed.size, 3);
  assert.equal(operations.filter((operation) => !consumed.has(operation.id)).length, 1);
});

test("uses JavaScript semantic language and finds floating promises", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "lexicon-js-outcome-"));
  fs.writeFileSync(path.join(root, "jsconfig.json"), JSON.stringify({ compilerOptions: { target: "ES2022", checkJs: true } }));
  fs.writeFileSync(path.join(root, "main.js"), [
    "async function load() { return 1; }",
    "function swallowed() { try { return 1; } catch (error) {} }",
    "function use() {",
    "  load();",
    "  return load();",
    "}",
    "",
  ].join("\n"));

  const { nodes, operations, consumed } = semanticRecords(buildFacts(root), "javascript");
  assert.equal(operations.length, 2);
  assert.equal(consumed.size, 1);
  assert.equal(operations.filter((operation) => !consumed.has(operation.id)).length, 1);
  assert.ok(nodes.some((node) => node.name === "error-handler:javascript"));
  assert.ok(nodes.some((node) => String(node.name).startsWith("semantic-capabilities:javascript:")));
});
