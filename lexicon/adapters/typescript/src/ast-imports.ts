import * as ts from "typescript";
import { expressionText, hasModifier, isStaticModuleSpecifier, spanFor } from "./contract";
import type { FileContext, FactStore, ImportName } from "./model";

export function recordImport(node: ts.ImportDeclaration, ownerId: string, context: FileContext, facts: FactStore): void {
  const source = isStaticModuleSpecifier(node.moduleSpecifier) ? node.moduleSpecifier.text : null;
  const names: ImportName[] = [];
  const clause = node.importClause;
  if (!clause) names.push({ imported: "*", local: "*", kind: "side-effect" });
  if (clause?.name) names.push({ imported: "default", local: clause.name.text, kind: "default" });
  if (clause?.namedBindings && ts.isNamespaceImport(clause.namedBindings)) {
    names.push({ imported: "*", local: clause.namedBindings.name.text, kind: "namespace" });
  } else if (clause?.namedBindings && ts.isNamedImports(clause.namedBindings)) {
    for (const element of clause.namedBindings.elements) {
      names.push({ imported: element.propertyName?.text ?? element.name.text, local: element.name.text, kind: "named" });
    }
  }
  addImportRecord(node, ownerId, context, facts, source, names);
}

export function recordImportEquals(node: ts.ImportEqualsDeclaration, ownerId: string, context: FileContext, facts: FactStore): void {
  const moduleReference = node.moduleReference;
  const source = ts.isExternalModuleReference(moduleReference) && isStaticModuleSpecifier(moduleReference.expression)
    ? moduleReference.expression.text
    : null;
  addImportRecord(node, ownerId, context, facts, source, [{ imported: "*", local: node.name.text, kind: "namespace" }]);
}

export function recordRequireCall(node: ts.CallExpression, ownerId: string, context: FileContext, facts: FactStore): boolean {
  if (!ts.isIdentifier(node.expression) || node.expression.text !== "require" || node.arguments.length !== 1) return false;
  const sourceNode = node.arguments[0];
  if (!isStaticModuleSpecifier(sourceNode)) return false;
  const names: ImportName[] = [];
  const declaration = ts.isVariableDeclaration(node.parent) && node.parent.initializer === node ? node.parent : null;
  if (declaration && ts.isIdentifier(declaration.name)) {
    names.push({ imported: "*", local: declaration.name.text, kind: "namespace" });
  } else if (declaration && ts.isObjectBindingPattern(declaration.name)) {
    for (const element of declaration.name.elements) {
      if (element.dotDotDotToken || !ts.isIdentifier(element.name)) continue;
      const imported = element.propertyName && (ts.isIdentifier(element.propertyName) || ts.isStringLiteralLike(element.propertyName))
        ? element.propertyName.text
        : element.name.text;
      names.push({ imported, local: element.name.text, kind: "named" });
    }
  } else {
    names.push({ imported: "*", local: "*", kind: "side-effect" });
  }
  addImportRecord(node, ownerId, context, facts, sourceNode.text, names);
  return true;
}

function addImportRecord(
  node: ts.Node,
  ownerId: string,
  context: FileContext,
  facts: FactStore,
  source: string | null,
  names: ImportName[],
): void {
  const recordSpan = spanFor(node, context.sourceFile, context.relativePath);
  const identity = `${context.moduleKey}::import:${recordSpan.start_line}:${recordSpan.start_column}`;
  const id = facts.addNode(
    "import",
    names.map((item) => item.imported).join(",") || "import",
    context.relativePath,
    identity,
    identity,
    recordSpan,
    { expression: expressionText(node, context.sourceFile) },
  );
  facts.addEdge(ownerId, id, "defines", recordSpan);
  facts.imports.push({
    nodeId: id,
    ownerId,
    moduleKey: context.moduleKey,
    sourceFile: context.sourceFile,
    source,
    expression: expressionText(node, context.sourceFile),
    names,
  });
}

export function recordExportDeclaration(node: ts.ExportDeclaration, ownerId: string, context: FileContext, facts: FactStore): void {
  const recordSpan = spanFor(node, context.sourceFile, context.relativePath);
  const source = isStaticModuleSpecifier(node.moduleSpecifier) ? node.moduleSpecifier.text : null;
  const names = node.exportClause && ts.isNamedExports(node.exportClause)
    ? node.exportClause.elements.map((element) => element.name.text)
    : ["*"];
  addExportRecord(node, ownerId, context, facts, names.join(","), false);
  if (source) {
    const reexported = node.exportClause && ts.isNamedExports(node.exportClause)
      ? node.exportClause.elements.map((element) => ({
        imported: element.propertyName?.text ?? element.name.text,
        exported: element.name.text,
      }))
      : [];
    facts.reexports.push({
      ownerId,
      moduleKey: context.moduleKey,
      source,
      expression: expressionText(node, context.sourceFile),
      names: reexported,
      span: recordSpan,
    });
  } else if (node.moduleSpecifier) {
    facts.addUnresolved(ownerId, "imports", expressionText(node, context.sourceFile), "unsupported-form", recordSpan);
  }
}

export function recordExportedDeclaration(ownerId: string, node: ts.Node, name: string, targetId: string, context: FileContext, facts: FactStore): void {
  const id = addExportRecord(node, ownerId, context, facts, name, hasModifier(node, ts.SyntaxKind.DefaultKeyword));
  facts.addEdge(id, targetId, "defines", spanFor(node, context.sourceFile, context.relativePath));
}

export function recordExportAssignment(node: ts.ExportAssignment, ownerId: string, context: FileContext, facts: FactStore): void {
  if (!node.isExportEquals) facts.defaultExports.set(context.moduleKey, node.expression);
  addExportRecord(node, ownerId, context, facts, node.isExportEquals ? "=" : "default", !node.isExportEquals);
}

export function recordCommonJsExport(node: ts.BinaryExpression, ownerId: string, context: FileContext, facts: FactStore): boolean {
  if (node.operatorToken.kind !== ts.SyntaxKind.EqualsToken) return false;
  const exportName = commonJsExportName(node.left);
  if (!exportName) return false;
  if (exportName === "default") {
    facts.defaultExports.set(context.moduleKey, node.right);
    addCommonJsObjectExports(node.right, context, facts);
  } else {
    commonJsExports(context, facts).set(exportName, node.right);
  }
  addExportRecord(node, ownerId, context, facts, exportName, exportName === "default");
  return true;
}

function addCommonJsObjectExports(expression: ts.Expression, context: FileContext, facts: FactStore): void {
  if (!ts.isObjectLiteralExpression(expression)) return;
  const exports = commonJsExports(context, facts);
  for (const property of expression.properties) {
    if (ts.isShorthandPropertyAssignment(property)) exports.set(property.name.text, property.name);
    else if (ts.isPropertyAssignment(property)) {
      const name = staticPropertyName(property.name);
      if (name) exports.set(name, property.initializer);
    }
  }
}

function commonJsExports(context: FileContext, facts: FactStore): Map<string, ts.Expression> {
  const exports = facts.commonJsExports.get(context.moduleKey) ?? new Map<string, ts.Expression>();
  facts.commonJsExports.set(context.moduleKey, exports);
  return exports;
}

function commonJsExportName(left: ts.Expression): string | null {
  if (ts.isPropertyAccessExpression(left)) {
    if (ts.isIdentifier(left.expression) && left.expression.text === "exports") return left.name.text;
    if (isModuleExports(left)) return "default";
    if (ts.isPropertyAccessExpression(left.expression) && isModuleExports(left.expression)) return left.name.text;
  }
  if (ts.isElementAccessExpression(left) && left.argumentExpression && ts.isStringLiteralLike(left.argumentExpression)) {
    if (ts.isIdentifier(left.expression) && left.expression.text === "exports") return left.argumentExpression.text;
    if (ts.isPropertyAccessExpression(left.expression) && isModuleExports(left.expression)) return left.argumentExpression.text;
  }
  return null;
}

function isModuleExports(expression: ts.PropertyAccessExpression): boolean {
  return ts.isIdentifier(expression.expression)
    && expression.expression.text === "module"
    && expression.name.text === "exports";
}

function staticPropertyName(name: ts.PropertyName): string | null {
  if (ts.isIdentifier(name) || ts.isStringLiteralLike(name) || ts.isNumericLiteral(name)) return name.text;
  return null;
}

function addExportRecord(
  node: ts.Node,
  ownerId: string,
  context: FileContext,
  facts: FactStore,
  name: string,
  isDefault: boolean,
): string {
  const recordSpan = spanFor(node, context.sourceFile, context.relativePath);
  const identity = `${context.moduleKey}::export:${recordSpan.start_line}:${recordSpan.start_column}:${name}`;
  const id = facts.addNode(
    "export",
    name,
    context.relativePath,
    identity,
    identity,
    recordSpan,
    { name, default: isDefault, expression: expressionText(node, context.sourceFile) },
  );
  facts.addEdge(ownerId, id, "defines", recordSpan);
  return id;
}
