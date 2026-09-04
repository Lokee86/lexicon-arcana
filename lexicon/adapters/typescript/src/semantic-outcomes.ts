import * as ts from "typescript";
import { spanFor } from "./contract";
import type { FactStore, FileContext } from "./model";
import { semanticLanguage } from "./semantic-facts";

export function emitOutcomeFacts(contexts: FileContext[], facts: FactStore, checker: ts.TypeChecker): void {
  for (const context of contexts) {
    visit(context.sourceFile, undefined, context, facts, checker);
  }
}

function visit(
  node: ts.Node,
  parent: ts.Node | undefined,
  context: FileContext,
  facts: FactStore,
  checker: ts.TypeChecker,
): void {
  if (ts.isCallExpression(node) && isPromiseLike(node, checker)) {
    emitOperation(node, parent, context, facts);
  }
  ts.forEachChild(node, (child) => visit(child, node, context, facts, checker));
}

function emitOperation(
  call: ts.CallExpression,
  parent: ts.Node | undefined,
  context: FileContext,
  facts: FactStore,
): void {
  const language = semanticLanguage(context);
  const start = call.getStart(context.sourceFile);
  const identity = `@semantic/outcome-operation/${language}/${context.relativePath}:${start}`;
  const operationSpan = spanFor(call, context.sourceFile, context.relativePath);
  const operationId = facts.addNode(
    "protocol",
    `outcome-operation:${language}:async`,
    context.relativePath,
    identity,
    identity,
    operationSpan,
  );
  if (!isConsumed(call, parent)) return;
  const actionIdentity = `${identity}/consume:${start}`;
  const actionId = facts.addNode(
    "protocol",
    "outcome-action:consume",
    context.relativePath,
    actionIdentity,
    actionIdentity,
    operationSpan,
  );
  facts.addEdge(operationId, actionId, "contains", operationSpan);
}

function isPromiseLike(call: ts.CallExpression, checker: ts.TypeChecker): boolean {
  return checker.getPropertyOfType(checker.getTypeAtLocation(call), "then") !== undefined;
}

function isConsumed(call: ts.CallExpression, parent: ts.Node | undefined): boolean {
  if (!parent || !ts.isExpressionStatement(parent)) return true;
  return isHandledPromiseChain(call);
}

function isHandledPromiseChain(call: ts.CallExpression): boolean {
  if (!ts.isPropertyAccessExpression(call.expression)) return false;
  const method = call.expression.name.text;
  if (method === "catch") return true;
  return method === "then" && call.arguments.length >= 2;
}
