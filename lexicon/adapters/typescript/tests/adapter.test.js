const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");

const ADAPTER_ROOT = path.resolve(__dirname, "..");
const REPO_ROOT = path.resolve(ADAPTER_ROOT, "../..");
const CLI = path.join(ADAPTER_ROOT, "dist", "cli.js");

function makeFixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "lexicon-typescript-"));
  const files = {
    "tsconfig.json": JSON.stringify({
      compilerOptions: {
        baseUrl: ".",
        paths: {
          "@/*": ["src/*"],
          "@exact": ["src/exact-target"],
          "@ambiguous": ["src/ambiguous-a", "src/ambiguous-b"],
        },
      },
    }),
    "src/base.ts": [
      "export interface Named { id: string }",
      "export class Base { run(): void {} }",
      "export const BASE = 1;",
      "let mutable = 2;",
      "",
    ].join("\n"),
    "src/dataflow.ts": [
      "export const FLOW_CONST = 2;",
      "export class DataBox { field = 0; }",
      "export function flow({ value }: { value: number }, box: DataBox): number {",
      "  let local = value;",
      "  local += FLOW_CONST;",
      "  local++;",
      "  box.field = local;",
      "  return local + value;",
      "}",
      "export function inner(value: number): number { let local = value; return local; }",
      "",
    ].join("\n"),
    "src/child.ts": [
      'import { AliasTarget } from "@/alias-target";',
      'import { ExactTarget } from "@exact";',
      'import { Ambiguous } from "@ambiguous";',
      'import { Missing } from "@/missing";',
      'import { Base, Named, BASE as VALUE } from "./base";',
      "export interface Child extends Named {}",
      "export class Child extends Base implements Named {",
      "  value = VALUE;",
      "  ambiguous: Ambiguous | Missing;",
      "  method(): void { void import(\"./dynamic\"); }",
      "}",
      "",
    ].join("\n"),
    "src/barrel.ts": 'export { Base as RenamedBase, Named as RenamedNamed } from "./base";\n',
    "src/alias-target.ts": "export class AliasTarget { value = 1; }\n",
    "src/exact-target.ts": "export class ExactTarget { value = 2; }\n",
    "src/ambiguous-a.ts": "export class Ambiguous {}\n",
    "src/ambiguous-b.ts": "export class Ambiguous {}\n",
    "src/alias-barrel.ts": 'export { AliasTarget as RenamedAliasTarget } from "@/alias-target";\n',
    "src/alias-consumer.ts": [
      'import { RenamedAliasTarget } from "./alias-barrel";',
      "export class AliasConsumer extends RenamedAliasTarget {}",
      "",
    ].join("\n"),
    "src/call-targets.ts": [
      "export function importedHelper(): void {}",
      "export class ImportedWorker {}",
      "",
    ].join("\n"),
    "src/calls.ts": [
      'import { importedHelper, ImportedWorker } from "./call-targets";',
      'import { AliasTarget } from "@/alias-target";',
      'import { externalHelper } from "external-package";',
      'import { MissingHelper } from "./missing";',
      "function localHelper(): void {}",
      "function overloaded(value: string): void;",
      "function overloaded(value: number): void;",
      "function overloaded(value: string | number): void {}",
      "class LocalWorker {}",
      "const callable = () => 1;",
      "export function caller(): void {",
      "  localHelper();",
      "  importedHelper();",
      "  new LocalWorker();",
      "  new ImportedWorker();",
      "  new AliasTarget();",
      "  externalHelper();",
      "  MissingHelper();",
      "  worker.run();",
      "  overloaded(1);",
      "  callable();",
      "}",
      "export class CallSite {",
      "  run(): void { localHelper(); }",
      "}",
      "",
    ].join("\n"),
    "src/semantics.ts": [
      "export interface Runner { run(): string }",
      "export class BaseRunner implements Runner { run(): string { return 'base'; } }",
      "export class ChildRunner extends BaseRunner { run(): string { return 'child'; } }",
      "export function mixin<T extends new (...args: any[]) => {}>(Base: T) { return class extends Base {}; }",
      "export class MixedRunner extends mixin(BaseRunner) {}",
      "export class Worker { run(): void {} }",
      "export function localCallback(): void {}",
      "export function invokeRunner(runner: Runner): string { return runner.run(); }",
      "export function exactRunner(): string { return new ChildRunner().run(); }",
      "export function invokeCallback(callback: () => void): void { callback(); }",
      "export function higherOrder(): void { invokeCallback(localCallback); }",
      "export function methodCalls(worker: Worker): void { worker.run(); worker?.run(); }",
      "export const arrowCallable = () => 1;",
      "export function arrowCaller(): number { return arrowCallable(); }",
      "export function tag(strings: TemplateStringsArray): string { return strings[0]; }",
      "export function taggedCaller(): string { return tag`value`; }",
      "",
    ].join("\n"),
    "src/default-component.tsx": [
      "export function InnerComponent(): null { return null; }",
      "export const AssignedComponent = Object.assign(InnerComponent, {});",
      "export default AssignedComponent;",
      "",
    ].join("\n"),
    "src/default-consumer.tsx": [
      'import DefaultComponent from "./default-component";',
      "export function RenderDefault() { return <DefaultComponent />; }",
      "",
    ].join("\n"),
    "src/through-barrel.ts": [
      'import { RenamedBase, RenamedNamed } from "./barrel";',
      "export class Through extends RenamedBase implements RenamedNamed {}",
      "",
    ].join("\n"),
    "src/index.tsx": [
      'import React from "react";',
      'export { Child } from "./child";',
      "export default function App() { return <Child />; }",
      "",
    ].join("\n"),
    "src/dynamic.ts": "export const loaded = true;\n",
    "src/computed.ts": 'const suffix = "dynamic";\nimport("./" + suffix);\n',
    "node_modules/ignored.ts": "export class Ignored {}\n",
    "build/ignored.ts": "export class IgnoredBuild {}\n",
    ".git/ignored.ts": "export class IgnoredGit {}\n",
    ".ddocs/ignored.ts": "export class IgnoredState {}\n",
    ".lexicon/ignored.ts": "export class IgnoredState {}\n",
    ".arcana/ignored.ts": "export class IgnoredState {}\n",
    ".grimoire/ignored.ts": "export class IgnoredState {}\n",
    ".pitlord/ignored.ts": "export class IgnoredState {}\n",
    ".cantrip/ignored.ts": "export class IgnoredState {}\n",
    ".homunculus/ignored.ts": "export class IgnoredState {}\n",
    ".incubus/ignored.ts": "export class IgnoredState {}\n",
    ".ritual/ignored.ts": "export class IgnoredState {}\n",
    ".warlock/ignored.ts": "export class IgnoredState {}\n",
  };
  for (const [relative, content] of Object.entries(files)) {
    const target = path.join(root, relative);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, content);
  }
  return root;
}

function runAdapter(repo, output) {
  const result = spawnSync(process.execPath, [CLI, "--repo", repo, "--output", output], { encoding: "utf8" });
  assert.equal(result.status, 0, `${result.stderr}\n${result.stdout}`);
  assert.equal(result.stderr, "");
  return fs.readFileSync(output, "utf8").trimEnd().split("\n").map(JSON.parse);
}

function recordsOf(records, kind) {
  return records.filter((record) => record.record === kind);
}

function spanKey(record) {
  const span = record.span || {};
  return [span.path || "", span.start_line || 0, span.start_column || 0, span.end_line || 0, span.end_column || 0];
}

function recordKey(record) {
  if (record.record === "node") return [0, record.id, record.kind, record.path, record.qualified_name];
  if (record.record === "edge") return [1, record.source, record.target, record.relation, ...spanKey(record)];
  return [2, record.source, record.relation, record.expression, record.reason, ...spanKey(record)];
}

function compare(left, right) {
  for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
    const a = String(left[index] ?? "");
    const b = String(right[index] ?? "");
    if (a < b) return -1;
    if (a > b) return 1;
  }
  return 0;
}

function assertSortedObject(value) {
  if (Array.isArray(value)) {
    value.forEach(assertSortedObject);
    return;
  }
  if (value && typeof value === "object") {
    const keys = Object.keys(value);
    assert.deepEqual(keys, [...keys].sort());
    Object.values(value).forEach(assertSortedObject);
  }
}

test("extracts TypeScript declarations, imports, exports, inheritance, and exclusions", () => {
  const repo = makeFixture();
  const output = path.join(repo, "facts.jsonl");
  const records = runAdapter(repo, output);
  const nodes = recordsOf(records, "node");
  const edges = recordsOf(records, "edge");
  const unresolved = recordsOf(records, "unresolved");

  assert.deepEqual(records[0], {
    adapter_version: "0.5.0",
    language: "typescript",
    record: "lexicon",
    repository: path.basename(repo),
    schema_version: 1,
  });
  const kinds = new Set(nodes.map((record) => record.kind));
  for (const kind of ["repository", "directory", "file", "module", "type", "interface", "function", "method", "variable", "constant", "import", "export"]) assert.ok(kinds.has(kind), kind);
  const paths = new Set(nodes.map((record) => record.path));
  assert.ok(![...paths].some((item) => item.includes("node_modules") || item.includes("build") || item.includes(".git")));
  for (const directory of [".ddocs", ".lexicon", ".arcana", ".grimoire", ".pitlord", ".cantrip", ".homunculus", ".incubus", ".ritual", ".warlock"]) {
    assert.ok(![...paths].some((item) => item.startsWith(`${directory}/`)), directory);
  }
  for (const relation of ["contains", "defines", "imports", "extends", "implements"]) assert.ok(edges.some((record) => record.relation === relation), relation);
  assert.ok(unresolved.some((record) => record.relation === "imports" && record.reason === "external-target"));
  assert.ok(unresolved.some((record) => record.relation === "imports" && record.reason === "dynamic-target"));
  assert.ok(nodes.some((record) => record.kind === "interface" && record.qualified_name === "src/child.Child"));
  assert.ok(nodes.some((record) => record.kind === "method" && record.qualified_name === "src/child.Child.method"));
  assert.ok(nodes.some((record) => record.kind === "constant" && record.qualified_name === "src/base.BASE"));
});

test("resolves exact and wildcard tsconfig aliases without guessing missing or ambiguous targets", () => {
  const repo = makeFixture();
  const records = runAdapter(repo, path.join(repo, "facts.jsonl"));
  const nodes = recordsOf(records, "node");
  const edges = recordsOf(records, "edge");
  const unresolved = recordsOf(records, "unresolved");
  const child = nodes.find((record) => record.kind === "module" && record.qualified_name === "src/child");
  const aliasTargetModule = nodes.find((record) => record.kind === "module" && record.qualified_name === "src/alias-target");
  const aliasTarget = nodes.find((record) => record.qualified_name === "src/alias-target.AliasTarget");
  const exactTarget = nodes.find((record) => record.qualified_name === "src/exact-target.ExactTarget");
  const aliasBarrel = nodes.find((record) => record.kind === "module" && record.qualified_name === "src/alias-barrel");
  const consumer = nodes.find((record) => record.qualified_name === "src/alias-consumer.AliasConsumer");
  assert.ok(child && aliasTargetModule && aliasTarget && exactTarget && aliasBarrel && consumer);
  assert.ok(edges.some((record) => record.source === child.id && record.target === aliasTarget.id && record.relation === "imports"));
  assert.ok(edges.some((record) => record.source === child.id && record.target === exactTarget.id && record.relation === "imports"));
  assert.ok(edges.some((record) => record.source === aliasBarrel.id && record.target === aliasTargetModule.id && record.relation === "imports"));
  assert.ok(edges.some((record) => record.source === consumer.id && record.target === aliasTarget.id && record.relation === "extends"));
  assert.ok(unresolved.some((record) => record.source === child.id && record.reason === "missing-target" && record.candidate_name === "@/missing:Missing"));
  assert.ok(unresolved.some((record) => record.source === child.id && record.reason === "ambiguous-target" && record.candidate_name === "@ambiguous:Ambiguous"));
});

test("resolves heritage through named barrel re-exports", () => {
  const repo = makeFixture();
  const records = runAdapter(repo, path.join(repo, "facts.jsonl"));
  const nodes = recordsOf(records, "node");
  const edges = recordsOf(records, "edge");
  const unresolved = recordsOf(records, "unresolved");
  const through = nodes.find((record) => record.qualified_name === "src/through-barrel.Through");
  const base = nodes.find((record) => record.qualified_name === "src/base.Base");
  const named = nodes.find((record) => record.qualified_name === "src/base.Named");
  assert.ok(through && base && named);
  assert.ok(edges.some((record) => record.source === through.id && record.target === base.id && record.relation === "extends"));
  assert.ok(edges.some((record) => record.source === through.id && record.target === named.id && record.relation === "implements"));
  assert.ok(!unresolved.some((record) => record.source === through.id && ["extends", "implements"].includes(record.relation)));
});

test("emits conservative direct function and constructor call facts", () => {
  const repo = makeFixture();
  const records = runAdapter(repo, path.join(repo, "facts.jsonl"));
  const nodes = recordsOf(records, "node");
  const edges = recordsOf(records, "edge");
  const unresolved = recordsOf(records, "unresolved");
  const caller = nodes.find((record) => record.qualified_name === "src/calls.caller");
  const method = nodes.find((record) => record.qualified_name === "src/calls.CallSite.run");
  const localHelper = nodes.find((record) => record.qualified_name === "src/calls.localHelper");
  const importedHelper = nodes.find((record) => record.qualified_name === "src/call-targets.importedHelper");
  const localWorker = nodes.find((record) => record.qualified_name === "src/calls.LocalWorker");
  const importedWorker = nodes.find((record) => record.qualified_name === "src/call-targets.ImportedWorker");
  const aliasTarget = nodes.find((record) => record.qualified_name === "src/alias-target.AliasTarget");
  const overloaded = nodes.find((record) => record.qualified_name === "src/calls.overloaded");
  const callable = nodes.find((record) => record.qualified_name === "src/calls.callable");
  assert.ok(caller && method && localHelper && importedHelper && localWorker && importedWorker && aliasTarget && overloaded && callable);
  for (const target of [localHelper, importedHelper, overloaded, callable]) assert.ok(edges.some((record) => record.source === caller.id && record.target === target.id && record.relation === "calls"));
  for (const target of [localWorker, importedWorker, aliasTarget]) assert.ok(edges.some((record) => record.source === caller.id && record.target === target.id && record.relation === "calls"));
  assert.ok(edges.some((record) => record.source === method.id && record.target === localHelper.id && record.relation === "calls"));
  assert.ok(unresolved.some((record) => record.source === caller.id && record.relation === "calls" && record.reason === "external-target" && record.candidate_name === "externalHelper"));
  assert.ok(unresolved.some((record) => record.source === caller.id && record.relation === "calls" && record.reason === "missing-target" && record.candidate_name === "MissingHelper"));
  assert.ok(unresolved.some((record) => record.source === caller.id && record.relation === "calls" && record.reason === "dynamic-target" && record.expression === "worker.run()"));
  assert.ok(!unresolved.some((record) => record.source === caller.id && ["overloaded", "callable"].includes(record.candidate_name)));
});

test("resolves compiler-backed methods, dispatch, callbacks, JSX, wrappers, and tagged templates", () => {
  const repo = makeFixture();
  const records = runAdapter(repo, path.join(repo, "facts.jsonl"));
  const nodes = recordsOf(records, "node");
  const edges = recordsOf(records, "edge");
  const unresolved = recordsOf(records, "unresolved");
  const node = (qualifiedName) => nodes.find((record) => record.qualified_name === qualifiedName);
  const invokeRunner = node("src/semantics.invokeRunner");
  const exactRunner = node("src/semantics.exactRunner");
  const baseRun = node("src/semantics.BaseRunner.run");
  const childRun = node("src/semantics.ChildRunner.run");
  const mixedRunner = node("src/semantics.MixedRunner");
  const mixin = node("src/semantics.mixin");
  const interfaceRun = node("src/semantics.Runner.run");
  const higherOrder = node("src/semantics.higherOrder");
  const invokeCallback = node("src/semantics.invokeCallback");
  const localCallback = node("src/semantics.localCallback");
  const methodCalls = node("src/semantics.methodCalls");
  const workerRun = node("src/semantics.Worker.run");
  const arrowCaller = node("src/semantics.arrowCaller");
  const arrowCallable = node("src/semantics.arrowCallable");
  const taggedCaller = node("src/semantics.taggedCaller");
  const tag = node("src/semantics.tag");
  const renderDefault = node("src/default-consumer.RenderDefault");
  const assigned = node("src/default-component.AssignedComponent");
  const inner = node("src/default-component.InnerComponent");
  for (const value of [invokeRunner, exactRunner, baseRun, childRun, mixedRunner, mixin, interfaceRun, higherOrder, invokeCallback, localCallback, methodCalls, workerRun, arrowCaller, arrowCallable, taggedCaller, tag, renderDefault, assigned, inner]) assert.ok(value);

  for (const target of [baseRun, childRun]) assert.ok(edges.some((record) => record.source === invokeRunner.id && record.target === target.id && record.relation === "possible-calls"));
  assert.ok(!edges.some((record) => record.source === invokeRunner.id && record.target === interfaceRun.id && ["calls", "possible-calls"].includes(record.relation)));
  assert.ok(edges.some((record) => record.source === exactRunner.id && record.target === childRun.id && record.relation === "calls"));
  assert.ok(edges.some((record) => record.source === childRun.id && record.target === baseRun.id && record.relation === "overrides"));
  assert.ok(edges.some((record) => record.source === mixedRunner.id && record.target === mixin.id && record.relation === "uses-trait"));
  assert.ok(edges.some((record) => record.source === higherOrder.id && record.target === invokeCallback.id && record.relation === "calls"));
  assert.ok(edges.some((record) => record.source === invokeCallback.id && record.target === localCallback.id && record.relation === "calls"));
  assert.ok(edges.some((record) => record.source === higherOrder.id && record.target === localCallback.id && record.relation === "possible-calls" && record.attributes?.callback === true));
  assert.equal(edges.filter((record) => record.source === methodCalls.id && record.target === workerRun.id && record.relation === "calls").length, 2);
  assert.ok(edges.some((record) => record.source === arrowCaller.id && record.target === arrowCallable.id && record.relation === "calls"));
  assert.ok(edges.some((record) => record.source === taggedCaller.id && record.target === tag.id && record.relation === "calls"));
  assert.ok(edges.some((record) => record.source === renderDefault.id && record.target === assigned.id && record.relation === "calls"));
  assert.ok(edges.some((record) => record.source === assigned.id && record.target === inner.id && record.relation === "calls" && record.attributes?.wrapper === true));
  assert.ok(!unresolved.some((record) => [invokeRunner.id, exactRunner.id, higherOrder.id, invokeCallback.id, methodCalls.id, arrowCaller.id, taggedCaller.id, renderDefault.id].includes(record.source) && record.relation === "calls" && record.reason === "missing-target"));
});

test("emits conservative reads and writes for TypeScript locals, members, updates, and shadowing", () => {
  const repo = makeFixture();
  const records = runAdapter(repo, path.join(repo, "facts.jsonl"));
  const nodes = recordsOf(records, "node");
  const edges = recordsOf(records, "edge");
  const nodeById = new Map(nodes.map((record) => [record.id, record]));
  const flow = nodes.find((record) => record.qualified_name === "src/dataflow.flow");
  const inner = nodes.find((record) => record.qualified_name === "src/dataflow.inner");
  assert.ok(flow && inner);
  const flowEdges = edges.filter((record) => record.source === flow.id && ["reads", "writes"].includes(record.relation));
  const innerEdges = edges.filter((record) => record.source === inner.id && ["reads", "writes"].includes(record.relation));
  assert.ok(flowEdges.some((record) => nodeById.get(record.target)?.name === "FLOW_CONST"));
  assert.ok(flowEdges.some((record) => nodeById.get(record.target)?.name === "field"));
  assert.ok(flowEdges.some((record) => nodeById.get(record.target)?.name === "local" && record.relation === "reads"));
  assert.ok(flowEdges.some((record) => nodeById.get(record.target)?.name === "local" && record.relation === "writes"));
  assert.ok(flowEdges.some((record) => nodeById.get(record.target)?.name === "value"));
  assert.ok(innerEdges.some((record) => nodeById.get(record.target)?.name === "value"));
  assert.notDeepEqual(new Set(flowEdges.map((record) => record.target)), new Set(innerEdges.map((record) => record.target)));
  assert.ok(edges.filter((record) => ["reads", "writes"].includes(record.relation)).every((record) => nodeById.has(record.target)));
});

test("uses contract IDs, canonical ordering, and stable repeat runs", () => {
  const repo = makeFixture();
  const firstPath = path.join(repo, "first.jsonl");
  const secondPath = path.join(repo, "second.jsonl");
  const first = runAdapter(repo, firstPath);
  const second = runAdapter(repo, secondPath);
  assert.equal(fs.readFileSync(firstPath, "utf8"), fs.readFileSync(secondPath, "utf8"));

  const facts = first.slice(1);
  const nodes = recordsOf(facts, "node");
  assert.equal(new Set(nodes.map((record) => record.id)).size, nodes.length);
  for (const record of first) assertSortedObject(record);
  assert.deepEqual(facts, [...facts].sort((left, right) => compare(recordKey(left), recordKey(right))));

  const file = nodes.find((record) => record.kind === "file" && record.path === "src/base.ts");
  assert.ok(file);
  const expectedId = `sha256:${crypto.createHash("sha256").update("lexicon:v1\0typescript\0file\0src/base.ts").digest("hex")}`;
  const expectedContent = `sha256:${crypto.createHash("sha256").update(fs.readFileSync(path.join(repo, "src/base.ts"))).digest("hex")}`;
  assert.equal(file.id, expectedId);
  assert.equal(file.content_id, expectedContent);
  assert.ok(!first.some((record) => JSON.stringify(record).includes(repo)));
});

test("passes the repository JSONL validator", () => {
  const repo = makeFixture();
  const output = path.join(repo, "facts.jsonl");
  runAdapter(repo, output);
  const python = process.env.PYTHON || (process.platform === "win32" ? "python" : "python3");
  const result = spawnSync(python, [path.join(REPO_ROOT, "tools", "validate_jsonl.py"), output], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
});

test("emits package manifest and sound local module dependencies", () => {
  const repo = makeFixture();
  fs.writeFileSync(path.join(repo, "package.json"), JSON.stringify({
    dependencies: { express: "^5.0.0", local: "file:./local-package" },
    devDependencies: { vitest: "^2.0.0" },
    peerDependencies: { react: ">=18" },
    optionalDependencies: { optional: "~1" },
  }));
  fs.mkdirSync(path.join(repo, "local-package"), { recursive: true });
  fs.writeFileSync(path.join(repo, "local-package", "index.ts"), "export const local = true;\n");
  const records = runAdapter(repo, path.join(repo, "dependencies.jsonl"));
  const edges = records.filter((record) => record.record === "edge" && record.relation === "depends-on");
  assert.ok(edges.some((record) => record.attributes?.category === "runtime" && record.attributes.constraint === "^5.0.0"));
  assert.ok(edges.some((record) => record.attributes?.category === "development" && record.attributes.dev === true));
  assert.ok(edges.some((record) => record.attributes?.category === "peer" && record.attributes.peer === true));
  assert.ok(edges.some((record) => record.attributes?.path === true && record.attributes.source === "package.json:dependencies"));
  assert.ok(edges.every((record) => record.attributes && typeof record.attributes.source === "string"));
});
