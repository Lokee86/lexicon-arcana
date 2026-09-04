import * as ts from "typescript";
import { spanFor, staticTarget } from "./contract";
import type { FactStore, FileContext } from "./model";

const CAPABILITIES = "control-flow,error-handling,calls,source-spans,outcome-obligations";
type ErrorAction = "propagate" | "record" | "recover";

type ActionEvidence = {
  action: ErrorAction;
  node: ts.Node;
};

export function emitSemanticFacts(contexts: FileContext[], facts: FactStore): void {
  for (const context of contexts) {
    emitCapabilities(context, facts);
    visit(context.sourceFile, context, facts);
  }
}

function emitCapabilities(context: FileContext, facts: FactStore): void {
  const language = semanticLanguage(context);
  const identity = `@semantic/capabilities/${language}/${context.relativePath}`;
  facts.addNode(
    "protocol",
    `semantic-capabilities:${language}:${CAPABILITIES}`,
    context.relativePath,
    identity,
    identity,
  );
}

function visit(node: ts.Node, context: FileContext, facts: FactStore): void {
  if (ts.isCatchClause(node)) emitCatchClause(node, context, facts);
  ts.forEachChild(node, (child) => visit(child, context, facts));
}

function emitCatchClause(clause: ts.CatchClause, context: FileContext, facts: FactStore): void {
  const language = semanticLanguage(context);
  const start = clause.getStart(context.sourceFile);
  const identity = `@semantic/error-handler/${language}/${context.relativePath}:${start}`;
  const handlerId = facts.addNode(
    "protocol",
    `error-handler:${language}`,
    context.relativePath,
    identity,
    identity,
    spanFor(clause, context.sourceFile, context.relativePath),
  );
  for (const evidence of collectActions(clause.block)) {
    const actionStart = evidence.node.getStart(context.sourceFile);
    const actionIdentity = `${identity}/${evidence.action}:${actionStart}`;
    const actionId = facts.addNode(
      "protocol",
      `error-action:${evidence.action}`,
      context.relativePath,
      actionIdentity,
      actionIdentity,
      spanFor(evidence.node, context.sourceFile, context.relativePath),
    );
    facts.addEdge(handlerId, actionId, "contains", spanFor(evidence.node, context.sourceFile, context.relativePath));
  }
}

function collectActions(block: ts.Block): ActionEvidence[] {
  const actions = new Map<ErrorAction, ts.Node>();
  const record = (action: ErrorAction, node: ts.Node): void => {
    if (!actions.has(action)) actions.set(action, node);
  };
  const inspect = (node: ts.Node): void => {
    if (node !== block && (ts.isFunctionLike(node) || ts.isCatchClause(node))) return;
    if (ts.isThrowStatement(node)) record("propagate", node);
    if (ts.isReturnStatement(node) && node.expression) {
      if (isPromiseReject(node.expression)) record("propagate", node);
      else record("recover", node);
    }
    if (ts.isCallExpression(node)) {
      const target = staticTarget(node.expression);
      if (target === "Promise.reject") record("propagate", node);
      else if (isRecordingTarget(target)) record("record", node);
      else record("recover", node);
    }
    if (ts.isBinaryExpression(node) && isAssignmentOperator(node.operatorToken.kind)) record("recover", node);
    if (ts.isBreakStatement(node) || ts.isContinueStatement(node)) record("recover", node);
    ts.forEachChild(node, inspect);
  };
  inspect(block);
  return [...actions.entries()].map(([action, node]) => ({ action, node }));
}

function isPromiseReject(expression: ts.Expression): boolean {
  return ts.isCallExpression(expression) && staticTarget(expression.expression) === "Promise.reject";
}

function isRecordingTarget(target: string | null): boolean {
  if (!target) return false;
  const leaf = target.split(".").at(-1)?.toLowerCase() ?? "";
  return ["error", "warn", "log", "reporterror", "captureerror", "captureexception"].includes(leaf);
}

function isAssignmentOperator(kind: ts.SyntaxKind): boolean {
  return kind >= ts.SyntaxKind.FirstAssignment && kind <= ts.SyntaxKind.LastAssignment;
}

export function semanticLanguage(context: FileContext): "javascript" | "typescript" {
  return /\.(?:js|jsx|mjs|cjs)$/i.test(context.relativePath) ? "javascript" : "typescript";
}
