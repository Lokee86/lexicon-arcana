import * as fs from "node:fs";
import * as path from "node:path";
import * as ts from "typescript";
import { declarationName, digest, expressionText, hasModifier, spanFor } from "./contract";
import { moduleKeyFor } from "./discovery";
import {
  recordCommonJsExport,
  recordExportAssignment,
  recordExportDeclaration,
  recordExportedDeclaration,
  recordImport,
  recordImportEquals,
  recordRequireCall,
} from "./ast-imports";
import type { FactStore, FileContext, PendingCall } from "./model";

export function createFileContext(
  root: string,
  relativePath: string,
  directoryIds: Map<string, string>,
  facts: FactStore,
  sourceFile: ts.SourceFile,
): FileContext {
  const absolutePath = path.join(root, relativePath.split("/").join(path.sep));
  const sourceBytes = fs.readFileSync(absolutePath);
  const fileSpan = spanFor(sourceFile, sourceFile, relativePath);
  const fileId = facts.addNode("file", path.posix.basename(relativePath), relativePath, relativePath, relativePath, fileSpan, undefined, digest(sourceBytes));
  const parent = path.posix.dirname(relativePath) || ".";
  facts.addEdge(directoryIds.get(parent) ?? directoryIds.get(".")!, fileId, "contains");
  const moduleKey = moduleKeyFor(relativePath);
  const moduleId = facts.addNode("module", path.posix.basename(moduleKey), relativePath, moduleKey, moduleKey);
  facts.modules.set(moduleKey, moduleId);
  facts.registerDeclaration(sourceFile, moduleId);
  facts.addEdge(fileId, moduleId, "contains", fileSpan);
  if (((sourceFile as ts.SourceFile & { parseDiagnostics?: ts.Diagnostic[] }).parseDiagnostics ?? []).length > 0) {
    facts.addUnresolved(moduleId, "parses", relativePath, "unsupported-form", undefined, "syntax-diagnostics");
  }
  return { relativePath, moduleKey, sourceFile, fileId, moduleId };
}

export function extractDeclarations(context: FileContext, facts: FactStore): void {
  visit(context.sourceFile, context.moduleId, [], context, facts);
}

function visit(node: ts.Node, ownerId: string, scope: string[], context: FileContext, facts: FactStore): void {
  if (ts.isSourceFile(node)) {
    node.forEachChild((child) => visit(child, ownerId, scope, context, facts));
    return;
  }
  if (ts.isImportDeclaration(node)) return recordImport(node, ownerId, context, facts);
  if (ts.isImportEqualsDeclaration(node)) return recordImportEquals(node, ownerId, context, facts);
  if (ts.isExportDeclaration(node)) return recordExportDeclaration(node, ownerId, context, facts);
  if (ts.isExportAssignment(node)) return recordExportAssignment(node, ownerId, context, facts);
  if (ts.isClassDeclaration(node) || ts.isClassExpression(node)) {
    visitClass(node, ownerId, scope, context, facts);
    return;
  }
  if (ts.isInterfaceDeclaration(node)) return visitInterface(node, ownerId, scope, context, facts);
  if (ts.isTypeAliasDeclaration(node)) return visitTypeAlias(node, ownerId, scope, context, facts);
  if (ts.isFunctionDeclaration(node)) return visitFunction(node, ownerId, scope, context, facts);
  if (ts.isFunctionExpression(node) || ts.isArrowFunction(node)) {
    visitFunctionExpression(node, ownerId, scope, context, facts);
    return;
  }
  if (ts.isMethodDeclaration(node) || ts.isMethodSignature(node) || ts.isGetAccessorDeclaration(node) || ts.isSetAccessorDeclaration(node)) {
    return visitMethod(node, ownerId, scope, context, facts);
  }
  if (ts.isConstructorDeclaration(node)) return visitMethod(node, ownerId, scope, context, facts, true);
  if (ts.isPropertyDeclaration(node) || ts.isPropertySignature(node)) return visitProperty(node, ownerId, scope, context, facts);
  if (ts.isVariableStatement(node)) return visitVariableList(node.declarationList, ownerId, scope, context, facts);
  if (ts.isVariableDeclarationList(node) && !ts.isVariableStatement(node.parent)) return visitVariableList(node, ownerId, scope, context, facts);
  if (ts.isBinaryExpression(node) && recordCommonJsExport(node, ownerId, context, facts)) {
    visit(node.right, ownerId, scope, context, facts);
    return;
  }
  if (ts.isNewExpression(node)) recordCall(node, "constructor", ownerId, scope, context, facts);
  else if (ts.isCallExpression(node)) {
    if (recordRequireCall(node, ownerId, context, facts)) return;
    if (node.expression.kind === ts.SyntaxKind.ImportKeyword) {
      facts.addUnresolved(ownerId, "imports", expressionText(node, context.sourceFile), "dynamic-target", spanFor(node, context.sourceFile, context.relativePath));
    } else {
      recordCall(node, "call", ownerId, scope, context, facts);
    }
  } else if (ts.isTaggedTemplateExpression(node)) {
    recordCall(node, "tagged-template", ownerId, scope, context, facts);
  } else if (ts.isJsxSelfClosingElement(node) || ts.isJsxOpeningElement(node)) {
    recordCall(node, "jsx", ownerId, scope, context, facts);
  }
  node.forEachChild((child) => visit(child, ownerId, scope, context, facts));
}

function recordCall(expression: PendingCall["expression"], kind: PendingCall["kind"], source: string, scope: string[], context: FileContext, facts: FactStore): void {
  facts.calls.push({ expression, kind, moduleKey: context.moduleKey, scope, source, sourceFile: context.sourceFile });
}

function addSymbol(kind: string, name: string, scope: string[], node: ts.Node, ownerId: string, context: FileContext, facts: FactStore): string {
  const qualifiedName = `${context.moduleKey}.${[...scope, name].join(".")}`;
  let id = facts.symbols.get(qualifiedName);
  if (!id) {
    id = facts.addNode(kind, name, context.relativePath, qualifiedName, qualifiedName, spanFor(node, context.sourceFile, context.relativePath));
    facts.symbols.set(qualifiedName, id);
    facts.addEdge(ownerId, id, "defines", spanFor(node, context.sourceFile, context.relativePath));
  }
  facts.registerDeclaration(node, id);
  const declaration = node as ts.NamedDeclaration;
  if (declaration.name) facts.registerDeclaration(declaration.name, id);
  return id;
}

function addParameters(node: ts.SignatureDeclarationBase, ownerId: string, scope: string[], context: FileContext, facts: FactStore): void {
  for (const parameter of node.parameters) {
    for (const binding of parameterBindings(parameter.name)) {
      const name = binding.name;
      const qualifiedName = `${context.moduleKey}.${[...scope, name].join(".")}`;
      const span = spanFor(binding.node, context.sourceFile, context.relativePath);
      const id = facts.addNode("parameter", name, context.relativePath, qualifiedName, qualifiedName, span);
      facts.addEdge(ownerId, id, "defines", span);
      facts.registerDeclaration(parameter, id);
      facts.registerDeclaration(binding.node, id);
    }
  }
}

function parameterBindings(name: ts.BindingName): { name: string; node: ts.Node }[] {
  if (ts.isIdentifier(name)) return [{ name: name.text, node: name }];
  const result: { name: string; node: ts.Node }[] = [];
  for (const element of name.elements) {
    if (ts.isBindingElement(element)) result.push(...parameterBindings(element.name));
  }
  return result;
}

function visitClass(
  node: ts.ClassDeclaration | ts.ClassExpression,
  ownerId: string,
  scope: string[],
  context: FileContext,
  facts: FactStore,
  preferredName?: string,
): string {
  const name = preferredName ?? declarationName(node.name) ?? anonymousName("class", node, context.sourceFile);
  const id = addSymbol("type", name, scope, node, ownerId, context, facts);
  if (hasModifier(node, ts.SyntaxKind.ExportKeyword)) recordExportedDeclaration(ownerId, node, name, id, context, facts);
  if (hasModifier(node, ts.SyntaxKind.DefaultKeyword)) facts.defaultExportIds.set(context.moduleKey, id);
  addHeritage(node, id, scope, context, facts);
  node.members.forEach((member) => visit(member, id, [...scope, name], context, facts));
  return id;
}

function visitInterface(node: ts.InterfaceDeclaration, ownerId: string, scope: string[], context: FileContext, facts: FactStore): void {
  const name = node.name.text;
  const id = addSymbol("interface", name, scope, node, ownerId, context, facts);
  if (hasModifier(node, ts.SyntaxKind.ExportKeyword)) recordExportedDeclaration(ownerId, node, name, id, context, facts);
  addHeritage(node, id, scope, context, facts);
  node.members.forEach((member) => visit(member, id, [...scope, name], context, facts));
}

function addHeritage(node: ts.ClassLikeDeclaration | ts.InterfaceDeclaration, sourceId: string, scope: string[], context: FileContext, facts: FactStore): void {
  for (const clause of node.heritageClauses ?? []) {
    for (const type of clause.types) {
      const relation = clause.token === ts.SyntaxKind.ExtendsKeyword
        ? (ts.isCallExpression(type.expression) ? "uses-trait" : "extends")
        : "implements";
      facts.relationships.push({ source: sourceId, relation, expression: type.expression, sourceFile: context.sourceFile, moduleKey: context.moduleKey, scope });
    }
  }
}

function visitTypeAlias(node: ts.TypeAliasDeclaration, ownerId: string, scope: string[], context: FileContext, facts: FactStore): void {
  const name = declarationName(node.name) ?? anonymousName("type", node, context.sourceFile);
  const id = addSymbol("type", name, scope, node, ownerId, context, facts);
  if (hasModifier(node, ts.SyntaxKind.ExportKeyword)) recordExportedDeclaration(ownerId, node, name, id, context, facts);
}

function visitFunction(node: ts.FunctionDeclaration, ownerId: string, scope: string[], context: FileContext, facts: FactStore): void {
  const name = declarationName(node.name) ?? anonymousName("function", node, context.sourceFile);
  const id = addSymbol("function", name, scope, node, ownerId, context, facts);
  addParameters(node, id, [...scope, name], context, facts);
  if (hasModifier(node, ts.SyntaxKind.ExportKeyword)) recordExportedDeclaration(ownerId, node, name, id, context, facts);
  if (hasModifier(node, ts.SyntaxKind.DefaultKeyword)) facts.defaultExportIds.set(context.moduleKey, id);
  node.forEachChild((child) => visit(child, id, [...scope, name], context, facts));
}

function visitFunctionExpression(node: ts.FunctionExpression | ts.ArrowFunction, ownerId: string, scope: string[], context: FileContext, facts: FactStore, preferredName?: string): string {
  const name = preferredName ?? functionExpressionName(node, context.sourceFile);
  const id = addSymbol("function", name, scope, node, ownerId, context, facts);
  addParameters(node, id, [...scope, name], context, facts);
  node.forEachChild((child) => visit(child, id, [...scope, name], context, facts));
  return id;
}

function visitMethod(
  node: ts.MethodDeclaration | ts.MethodSignature | ts.ConstructorDeclaration | ts.GetAccessorDeclaration | ts.SetAccessorDeclaration,
  ownerId: string,
  scope: string[],
  context: FileContext,
  facts: FactStore,
  constructor = false,
): void {
  const name = constructor ? "constructor" : declarationName(node.name) ?? "<computed>";
  const id = addSymbol(constructor ? "constructor" : "method", name, scope, node, ownerId, context, facts);
  addParameters(node, id, [...scope, name], context, facts);
  node.forEachChild((child) => visit(child, id, [...scope, name], context, facts));
}

function visitProperty(node: ts.PropertyDeclaration | ts.PropertySignature, ownerId: string, scope: string[], context: FileContext, facts: FactStore): void {
  const name = declarationName(node.name) ?? "<computed>";
  const initializer = ts.isPropertyDeclaration(node) ? node.initializer : undefined;
  if (initializer && (ts.isArrowFunction(initializer) || ts.isFunctionExpression(initializer))) {
    const id = visitFunctionExpression(initializer, ownerId, scope, context, facts, name);
    facts.registerDeclaration(node, id);
    facts.registerDeclaration(node.name, id);
    return;
  }
  const id = addSymbol("field", name, scope, node, ownerId, context, facts);
  facts.registerDeclaration(node.name, id);
  if (initializer) visit(initializer, ownerId, scope, context, facts);
}

function visitVariableList(list: ts.VariableDeclarationList, ownerId: string, scope: string[], context: FileContext, facts: FactStore): void {
  const defaultKind = (list.flags & ts.NodeFlags.Const) !== 0 ? "constant" : "variable";
  for (const declaration of list.declarations) {
    const name = declarationName(declaration.name) ?? "<computed>";
    if (declaration.initializer && (ts.isArrowFunction(declaration.initializer) || ts.isFunctionExpression(declaration.initializer))) {
      const id = visitFunctionExpression(declaration.initializer, ownerId, scope, context, facts, name);
      facts.registerDeclaration(declaration, id);
      facts.registerDeclaration(declaration.name, id);
      if (hasModifier(list.parent, ts.SyntaxKind.ExportKeyword)) recordExportedDeclaration(ownerId, list.parent, name, id, context, facts);
      continue;
    }
    if (declaration.initializer && ts.isClassExpression(declaration.initializer)) {
      const id = visitClass(declaration.initializer, ownerId, scope, context, facts, name);
      facts.registerDeclaration(declaration, id);
      facts.registerDeclaration(declaration.name, id);
      if (hasModifier(list.parent, ts.SyntaxKind.ExportKeyword)) recordExportedDeclaration(ownerId, list.parent, name, id, context, facts);
      continue;
    }
    const preservedCallback = declaration.initializer ? callbackPreservingFunction(declaration.initializer) : undefined;
    if (preservedCallback && declaration.initializer && ts.isCallExpression(declaration.initializer)) {
      const id = visitFunctionExpression(preservedCallback, ownerId, scope, context, facts, name);
      facts.registerDeclaration(declaration, id);
      facts.registerDeclaration(declaration.name, id);
      if (hasModifier(list.parent, ts.SyntaxKind.ExportKeyword)) recordExportedDeclaration(ownerId, list.parent, name, id, context, facts);

      // The wrapper invocation belongs to the enclosing scope, while the callback
      // body belongs to the named function above. Avoid visiting the first
      // argument twice or creating a second anonymous lambda for the same body.
      recordCall(declaration.initializer, "call", ownerId, scope, context, facts);
      for (const argument of declaration.initializer.arguments.slice(1)) {
        if (ts.isExpression(argument)) visit(argument, ownerId, scope, context, facts);
      }
      continue;
    }
    const callableArguments = declaration.initializer ? transparentCallableArguments(declaration.initializer) : [];
    if (callableArguments.length > 0) {
      const id = addSymbol("function", name, scope, declaration, ownerId, context, facts);
      facts.callableAliases.push({ source: id, expressions: callableArguments, moduleKey: context.moduleKey, scope, sourceFile: context.sourceFile });
      if (hasModifier(list.parent, ts.SyntaxKind.ExportKeyword)) recordExportedDeclaration(ownerId, list.parent, name, id, context, facts);
      visit(declaration.initializer!, ownerId, scope, context, facts);
      continue;
    }
    const id = addSymbol(defaultKind, name, scope, declaration, ownerId, context, facts);
    if (hasModifier(list.parent, ts.SyntaxKind.ExportKeyword)) recordExportedDeclaration(ownerId, list.parent, name, id, context, facts);
    if (declaration.initializer) visit(declaration.initializer, ownerId, scope, context, facts);
  }
}

function callbackPreservingFunction(expression: ts.Expression): ts.FunctionExpression | ts.ArrowFunction | undefined {
  if (!ts.isCallExpression(expression)) return undefined;
  const name = expression.expression.getText(expression.getSourceFile()).split(".").at(-1);
  if (name !== "useCallback") return undefined;
  const first = expression.arguments[0];
  return first && (ts.isArrowFunction(first) || ts.isFunctionExpression(first)) ? first : undefined;
}

function transparentCallableArguments(expression: ts.Expression): ts.Expression[] {
  if (!ts.isCallExpression(expression)) return [];
  const name = expression.expression.getText(expression.getSourceFile()).split(".").at(-1);
  if (!name || !new Set(["forwardRef", "memo", "assign", "useCallback"]).has(name)) return [];
  const first = expression.arguments[0];
  return first && ts.isExpression(first) ? [first] : [];
}

function functionExpressionName(node: ts.FunctionExpression | ts.ArrowFunction, sourceFile: ts.SourceFile): string {
  if (ts.isFunctionExpression(node) && node.name) return node.name.text;
  const parent = node.parent;
  if (ts.isVariableDeclaration(parent)) return declarationName(parent.name) ?? anonymousName("lambda", node, sourceFile);
  if (ts.isPropertyDeclaration(parent) || ts.isPropertyAssignment(parent)) return declarationName(parent.name) ?? anonymousName("lambda", node, sourceFile);
  return anonymousName("lambda", node, sourceFile);
}

function anonymousName(kind: string, node: ts.Node, sourceFile: ts.SourceFile): string {
  const start = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
  return `<${kind}>@${start.line + 1}:${start.character + 1}`;
}
