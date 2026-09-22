import * as fs from "node:fs";
import * as path from "node:path";
import { compareKeys, factSortKey, jsonLine, LANGUAGE, SCHEMA_VERSION, VERSION } from "./contract";
import type { Fact, FactStore } from "./model";

export function emitFacts(facts: FactStore, changedFiles?: string[], removedFiles?: string[]): Fact[] {
  const incremental = changedFiles !== undefined || removedFiles !== undefined;
  const selected = new Set((changedFiles ?? []).map(normalizePath));
  const header: Fact = {
    adapter_version: VERSION,
    language: LANGUAGE,
    record: "lexicon",
    repository: facts.repository,
    schema_version: SCHEMA_VERSION,
  };
  if (incremental) {
    header.mode = "incremental";
    header.changed_files = Array.from(selected).sort();
    header.removed_files = (removedFiles ?? []).map(normalizePath).sort();
    header.shared_complete = true;
  }
  const nodes = Array.from(facts.nodes.values()).sort((a, b) => compareKeys(factSortKey(a), factSortKey(b)));
  const owners = new Map(nodes.map((record) => [String(record.id ?? ""), directOwner(record)]));
  const records = [
    ...nodes,
    ...Array.from(facts.edges.values()).sort((a, b) => compareKeys(factSortKey(a), factSortKey(b))),
    ...Array.from(facts.unresolved.values()).sort((a, b) => compareKeys(factSortKey(a), factSortKey(b))),
  ];
  return [header, ...(incremental ? records.filter((record) => includeRecord(record, owners, selected)) : records)];
}

function normalizePath(value: string): string {
  return value.replace(/\\/g, "/");
}

function directOwner(record: Fact): string {
  if (typeof record.owner === "string" && record.owner) return normalizePath(record.owner);
  const span = record.span as Record<string, unknown> | undefined;
  if (span && typeof span.path === "string") return normalizePath(span.path);
  if (record.record === "node" && record.kind === "file" && typeof record.path === "string") return normalizePath(record.path);
  return "";
}

function includeRecord(record: Fact, owners: Map<string, string>, selected: Set<string>): boolean {
  let owner = directOwner(record);
  if (!owner && typeof record.source === "string") owner = owners.get(record.source) ?? "";
  return !owner || selected.has(owner);
}

export function writeJsonl(records: Fact[], outputPath: string): void {
  if (outputPath === "-") {
    for (const record of records) process.stdout.write(jsonLine(record) + "\n");
    return;
  }
  const destination = path.resolve(outputPath);
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  const file = fs.openSync(destination, "w");
  try {
    for (const record of records) fs.writeSync(file, jsonLine(record) + "\n", undefined, "utf8");
  } finally {
    fs.closeSync(file);
  }
}
