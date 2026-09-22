import * as ts from "typescript";
import {
  callCallee,
  isSignatureDeclaration,
  mergeSets,
  symbolLocation,
  unwrapExpression,
  type ParameterTargets,
} from "./call-shared";
import type { FactStore, PendingCall } from "./model";

export type DispatchIndex = Map<string, string[]>;

export function buildDispatchIndex(facts: FactStore): DispatchIndex {
  const index: DispatchIndex = new Map();
  for (const [target, declarations] of facts.idDeclarations) {
    if (facts.nodes.get(target)?.kind !== "method") continue;
    const names = new Set<string>();
    for (const declaration of declarations) {
      if (ts.isMethodDeclaration(declaration)) {
        names.add(declaration.name.getText(declaration.getSourceFile()));
      }
    }
    for (const name of names) {
      const targets = index.get(name) ?? [];
      targets.push(target);
      index.set(name, targets);
    }
  }
  for (const targets of index.values()) targets.sort();
  return index;
}

export function resolveCallTargets(
  facts: FactStore,
  checker: ts.TypeChecker,
  call: PendingCall,
  parameterTargets: ParameterTargets,
  dispatchIndex: DispatchIndex,
): Set<string> {
  const targets = new Set<string>();
  if (call.kind === "jsx") {
    const tagName = (call.expression as ts.JsxOpeningLikeElement).tagName;
    addSymbolTargets(facts, checker, checker.getSymbolAtLocation(tagName), targets, parameterTargets, true);
    addImportedBindingTargets(facts, call, targets, true);
    if (targets.size === 0) addDefaultImportTarget(facts, checker, call, targets, parameterTargets);
    return targets;
  }

  const expression = call.expression as ts.CallExpression | ts.NewExpression | ts.TaggedTemplateExpression;
  const resolved = checker.getResolvedSignature(expression);
  if (resolved) addSignatureTarget(facts, checker, resolved, targets, parameterTargets, call.kind === "constructor");
  const callee = callCallee(call);
  if (!callee) return targets;
  const type = checker.getTypeAtLocation(callee);
  const signatures = call.kind === "constructor" ? type.getConstructSignatures() : type.getCallSignatures();
  for (const signature of signatures) {
    addSignatureTarget(facts, checker, signature, targets, parameterTargets, call.kind === "constructor");
  }
  addSymbolTargets(
    facts,
    checker,
    checker.getSymbolAtLocation(symbolLocation(callee)),
    targets,
    parameterTargets,
    call.kind === "constructor",
  );
  addImportedBindingTargets(facts, call, targets, call.kind === "constructor");
  if (targets.size === 0) addDefaultImportTarget(facts, checker, call, targets, parameterTargets);
  normalizeConstructorTargets(facts, call, targets);
  return expandDispatchTargets(facts, checker, call, targets, dispatchIndex);
}

function addImportedBindingTargets(
  facts: FactStore,
  call: PendingCall,
  targets: Set<string>,
  includeTypes: boolean,
): void {
  const reference = call.kind === "jsx"
    ? (call.expression as ts.JsxOpeningLikeElement).tagName
    : callCallee(call);
  if (!reference) return;
  const unwrapped = unwrapExpression(reference as ts.Expression);
  if (ts.isIdentifier(unwrapped)) {
    const target = facts.bindings.get(call.moduleKey)?.get(unwrapped.text)?.targetId;
    if (target && isCallableTarget(facts, target, includeTypes)) targets.add(target);
    return;
  }
  if (!ts.isPropertyAccessExpression(unwrapped) || !ts.isIdentifier(unwrapped.expression)) return;
  const moduleId = facts.bindings.get(call.moduleKey)?.get(unwrapped.expression.text)?.targetId;
  if (!moduleId || facts.nodes.get(moduleId)?.kind !== "module") return;
  const importedModuleKey = [...facts.modules].find(([, id]) => id === moduleId)?.[0];
  if (!importedModuleKey) return;
  const target = facts.bindings.get(importedModuleKey)?.get(unwrapped.name.text)?.targetId
    ?? facts.symbols.get(`${importedModuleKey}.${unwrapped.name.text}`);
  if (target && isCallableTarget(facts, target, includeTypes)) targets.add(target);
}

function normalizeConstructorTargets(facts: FactStore, call: PendingCall, targets: Set<string>): void {
  if (call.kind !== "constructor") return;
  const hasConstructor = [...targets].some((target) => facts.nodes.get(target)?.kind === "constructor");
  if (!hasConstructor) return;
  for (const target of [...targets]) if (facts.nodes.get(target)?.kind === "type") targets.delete(target);
}

function addDefaultImportTarget(
  facts: FactStore,
  checker: ts.TypeChecker,
  call: PendingCall,
  targets: Set<string>,
  parameterTargets: ParameterTargets,
): void {
  const reference = call.kind === "jsx"
    ? (call.expression as ts.JsxOpeningLikeElement).tagName
    : callCallee(call);
  if (!reference || !ts.isIdentifier(reference)) return;
  const binding = facts.bindings.get(call.moduleKey)?.get(reference.text);
  if (!binding?.targetId || facts.nodes.get(binding.targetId)?.kind !== "module") return;
  const moduleKey = [...facts.modules].find(([, id]) => id === binding.targetId)?.[0];
  const exported = moduleKey ? facts.defaultExports.get(moduleKey) : undefined;
  if (!exported) return;
  for (const target of resolveExpressionTargets(facts, checker, exported, parameterTargets)) targets.add(target);
}

function expandDispatchTargets(
  facts: FactStore,
  checker: ts.TypeChecker,
  call: PendingCall,
  targets: Set<string>,
  dispatchIndex: DispatchIndex,
): Set<string> {
  if (call.kind !== "call") return targets;
  const callee = unwrapExpression((call.expression as ts.CallExpression).expression);
  const access = ts.isPropertyAccessExpression(callee) || ts.isElementAccessExpression(callee) ? callee : null;
  if (!access || ts.isNewExpression(unwrapExpression(access.expression))) return targets;
  const methodName = ts.isPropertyAccessExpression(access)
    ? access.name.text
    : ts.isStringLiteralLike(access.argumentExpression)
      ? access.argumentExpression.text
      : null;
  if (!methodName) return targets;
  const receiverType = checker.getTypeAtLocation(access.expression);
  if ((receiverType.flags & (ts.TypeFlags.Any | ts.TypeFlags.Unknown)) !== 0) return targets;
  const implementations = new Set<string>();
  for (const target of dispatchIndex.get(methodName) ?? []) {
    for (const declaration of facts.idDeclarations.get(target) ?? []) {
      if (!ts.isMethodDeclaration(declaration)) continue;
      if ((ts.getModifiers(declaration) ?? []).some((modifier) => modifier.kind === ts.SyntaxKind.StaticKeyword)) continue;
      const owner = declaration.parent;
      if (!ts.isClassDeclaration(owner) || !owner.name) continue;
      const symbol = checker.getSymbolAtLocation(owner.name);
      if (!symbol) continue;
      const instanceType = checker.getDeclaredTypeOfSymbol(symbol);
      if (receiverAcceptsCandidate(checker, receiverType, instanceType)) implementations.add(target);
    }
  }
  if (implementations.size === 0) return targets;
  const result = new Set<string>();
  for (const target of targets) {
    const declarations = facts.idDeclarations.get(target) ?? [];
    if (!declarations.some(ts.isMethodSignature)) result.add(target);
  }
  for (const target of implementations) result.add(target);
  return result;
}

function receiverAcceptsCandidate(checker: ts.TypeChecker, receiverType: ts.Type, candidateType: ts.Type): boolean {
  const receiverSymbol = receiverType.getSymbol();
  if (receiverSymbol && (receiverSymbol.flags & ts.SymbolFlags.Class) !== 0) {
    return classTypeDerivesFrom(candidateType, receiverSymbol, new Set<ts.Symbol>());
  }
  return checker.isTypeAssignableTo(candidateType, receiverType);
}

function classTypeDerivesFrom(type: ts.Type, expected: ts.Symbol, seen: Set<ts.Symbol>): boolean {
  const symbol = type.getSymbol();
  if (!symbol || seen.has(symbol)) return false;
  if (symbol === expected) return true;
  seen.add(symbol);
  if ((type.flags & ts.TypeFlags.Object) === 0) return false;
  return ((type as ts.InterfaceType).getBaseTypes() ?? []).some((base) => classTypeDerivesFrom(base, expected, seen));
}

export function resolveExpressionTargets(
  facts: FactStore,
  checker: ts.TypeChecker,
  expression: ts.Expression,
  parameterTargets: ParameterTargets,
  visited = new Set<ts.Node>(),
): Set<string> {
  const unwrapped = unwrapExpression(expression);
  if (visited.has(unwrapped)) return new Set();
  const nextVisited = new Set(visited);
  nextVisited.add(unwrapped);
  if (ts.isConditionalExpression(unwrapped)) {
    return mergeSets(
      resolveExpressionTargets(facts, checker, unwrapped.whenTrue, parameterTargets, nextVisited),
      resolveExpressionTargets(facts, checker, unwrapped.whenFalse, parameterTargets, nextVisited),
    );
  }
  const targets = new Set<string>();
  const direct = facts.declarationIds.get(unwrapped);
  if (direct && isCallableTarget(facts, direct, false)) targets.add(direct);
  addSymbolTargets(
    facts,
    checker,
    checker.getSymbolAtLocation(symbolLocation(unwrapped)),
    targets,
    parameterTargets,
    false,
    nextVisited,
  );
  const type = checker.getTypeAtLocation(unwrapped);
  for (const signature of type.getCallSignatures()) {
    addSignatureTarget(facts, checker, signature, targets, parameterTargets, false, nextVisited);
  }
  return targets;
}

function addSignatureTarget(
  facts: FactStore,
  checker: ts.TypeChecker,
  signature: ts.Signature,
  targets: Set<string>,
  parameterTargets: ParameterTargets,
  includeTypes: boolean,
  visited = new Set<ts.Node>(),
): void {
  const declaration = signature.getDeclaration();
  if (declaration) addDeclarationTarget(facts, checker, declaration, targets, parameterTargets, includeTypes, visited);
}

function addSymbolTargets(
  facts: FactStore,
  checker: ts.TypeChecker,
  symbol: ts.Symbol | undefined,
  targets: Set<string>,
  parameterTargets: ParameterTargets,
  includeTypes: boolean,
  visited = new Set<ts.Node>(),
): void {
  if (!symbol) return;
  const resolved = (symbol.flags & ts.SymbolFlags.Alias) !== 0 ? checker.getAliasedSymbol(symbol) : symbol;
  for (const declaration of resolved.declarations ?? []) {
    addDeclarationTarget(facts, checker, declaration, targets, parameterTargets, includeTypes, visited);
  }
  if (resolved.valueDeclaration) {
    addDeclarationTarget(facts, checker, resolved.valueDeclaration, targets, parameterTargets, includeTypes, visited);
  }
}

function addDeclarationTarget(
  facts: FactStore,
  checker: ts.TypeChecker,
  declaration: ts.Declaration,
  targets: Set<string>,
  parameterTargets: ParameterTargets,
  includeTypes: boolean,
  visited = new Set<ts.Node>(),
): void {
  if (visited.has(declaration)) return;
  const nextVisited = new Set(visited);
  nextVisited.add(declaration);
  const parameter = enclosingParameterTarget(declaration);
  if (parameter) {
    for (const target of parameterTargets.get(parameter) ?? []) targets.add(target);
    if (ts.isBindingElement(parameter) && parameter.initializer) {
      for (const target of resolveExpressionTargets(facts, checker, parameter.initializer, parameterTargets, nextVisited)) {
        targets.add(target);
      }
    }
    return;
  }
  if (ts.isExportAssignment(declaration)) {
    for (const target of resolveExpressionTargets(facts, checker, declaration.expression, parameterTargets, nextVisited)) targets.add(target);
    return;
  }
  const valueExpression = declarationValueExpression(declaration);
  if (valueExpression) {
    for (const target of resolveExpressionTargets(facts, checker, valueExpression, parameterTargets, nextVisited)) targets.add(target);
    if (targets.size > 0) return;
  }
  const direct = declarationId(facts, declaration);
  if (direct && isCallableTarget(facts, direct, includeTypes)) {
    targets.add(direct);
    return;
  }
  const named = declaration as ts.NamedDeclaration;
  if (!named.name) return;
  const symbol = checker.getSymbolAtLocation(named.name);
  if (symbol && (symbol.flags & ts.SymbolFlags.Alias) !== 0) {
    addSymbolTargets(facts, checker, symbol, targets, parameterTargets, includeTypes, nextVisited);
  }
}

function isCallableTarget(facts: FactStore, target: string, includeTypes: boolean): boolean {
  const kind = facts.nodes.get(target)?.kind;
  if (kind === "function" || kind === "method" || kind === "constructor") return true;
  if (!includeTypes || kind !== "type") return false;
  return (facts.idDeclarations.get(target) ?? []).some(
    (declaration) => ts.isClassDeclaration(declaration) || ts.isClassExpression(declaration),
  );
}

function enclosingParameterTarget(node: ts.Node): ts.ParameterDeclaration | ts.BindingElement | null {
  let current: ts.Node | undefined = node;
  while (current && !ts.isSourceFile(current)) {
    if (ts.isBindingElement(current) || ts.isParameter(current)) return current;
    if (isSignatureDeclaration(current) && current !== node) return null;
    current = current.parent;
  }
  return null;
}

function declarationValueExpression(node: ts.Node): ts.Expression | null {
  if (
    ts.isVariableDeclaration(node)
    || ts.isPropertyDeclaration(node)
    || ts.isParameter(node)
    || ts.isBindingElement(node)
  ) return node.initializer ?? null;
  if (ts.isPropertyAssignment(node)) return node.initializer;
  if (ts.isShorthandPropertyAssignment(node)) return node.name;
  if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.EqualsToken) return node.right;
  return null;
}

function declarationId(facts: FactStore, declaration: ts.Node): string | null {
  const direct = facts.declarationIds.get(declaration);
  if (direct) return direct;
  const named = declaration as ts.NamedDeclaration;
  return named.name ? facts.declarationIds.get(named.name) ?? null : null;
}
