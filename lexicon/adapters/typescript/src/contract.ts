import * as crypto from "node:crypto";
import * as ts from "typescript";
import type { Fact, JsonRecord, Span } from "./model";

export const VERSION = "0.5.0";
export const LANGUAGE = "typescript";
export const SCHEMA_VERSION = 1;

export function digest(value: string | Buffer): string {
  const input = typeof value === "string" ? Buffer.from(value, "utf8") : value;
  return `sha256:${crypto.createHash("sha256").update(input).digest("hex")}`;
}

export function nodeId(kind: string, identity: string): string {
  return digest(`lexicon:v1\0${LANGUAGE}\0${kind}\0${identity}`);
}

export function sortedObject(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortedObject);
  if (value !== null && typeof value === "object") {
    const object = value as JsonRecord;
    return Object.fromEntries(Object.keys(object).sort().map((key) => [key, sortedObject(object[key])]));
  }
  return value;
}

export function jsonLine(record: unknown): string {
  return JSON.stringify(sortedObject(record), undefined, undefined);
}

export function spanFor(node: ts.Node, sourceFile: ts.SourceFile, relativePath: string): Span {
  const start = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
  const end = sourceFile.getLineAndCharacterOfPosition(node.getEnd());
  return {
    end_column: end.character + 1,
    end_line: end.line + 1,
    path: relativePath,
    start_column: start.character + 1,
    start_line: start.line + 1,
  };
}

export function spanKey(record: JsonRecord): unknown[] {
  const value = (record.span ?? {}) as JsonRecord;
  return [value.path ?? "", value.start_line ?? 0, value.start_column ?? 0, value.end_line ?? 0, value.end_column ?? 0];
}

export function factSortKey(record: Fact): unknown[] {
  if (record.record === "node") return [0, record.id, record.kind, record.path, record.qualified_name];
  if (record.record === "edge") return [1, record.source, record.target, record.relation, ...spanKey(record)];
  return [2, record.source, record.relation, record.expression, record.reason, ...spanKey(record)];
}

export function compareKeys(left: unknown[], right: unknown[]): number {
  for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
    const a = left[index] ?? "";
    const b = right[index] ?? "";
    if (typeof a === "number" && typeof b === "number") {
      if (a < b) return -1;
      if (a > b) return 1;
      continue;
    }
    const leftText = String(a);
    const rightText = String(b);
    if (leftText < rightText) return -1;
    if (leftText > rightText) return 1;
  }
  return 0;
}

export function declarationName(name: ts.PropertyName | ts.BindingName | undefined): string | null {
  if (!name) return null;
  if (ts.isIdentifier(name) || ts.isStringLiteral(name) || ts.isNumericLiteral(name)) return name.text;
  return null;
}

export function expressionText(node: ts.Node, sourceFile: ts.SourceFile): string {
  return node.getText(sourceFile).trim();
}

export function staticTarget(node: ts.Expression): string | null {
  if (ts.isIdentifier(node)) return node.text;
  if (ts.isPropertyAccessExpression(node)) {
    const parent = staticTarget(node.expression);
    return parent ? `${parent}.${node.name.text}` : null;
  }
  if (ts.isParenthesizedExpression(node)) return staticTarget(node.expression);
  return null;
}

export function hasModifier(node: ts.Node, kind: ts.SyntaxKind): boolean {
  return (ts.canHaveModifiers(node) ? ts.getModifiers(node) ?? [] : []).some((modifier) => modifier.kind === kind);
}

export function isStaticModuleSpecifier(node: ts.Node | undefined): node is ts.StringLiteralLike {
  return !!node && (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node));
}

export function spanForNodeId(facts: { nodes: Map<string, Fact> }, id: string): Span | undefined {
  return facts.nodes.get(id)?.span as Span | undefined;
}
