import * as path from "node:path";
import * as ts from "typescript";
import { relativeSourcePath } from "./call-shared";
import { expressionText, spanFor, spanForNodeId, staticTarget } from "./contract";
import { moduleKeyFor, type PathMapping } from "./discovery";
import type { FactStore, ImportInfo } from "./model";

export function resolveImports(facts: FactStore, pathMappings: PathMapping[] = []): void {
  resolveCommonJsBindings(facts);
  resolveReexportBindings(facts, pathMappings);
  for (const info of facts.imports) {
    if (!info.source) {
      facts.addUnresolved(info.ownerId, "imports", info.expression, "unsupported-form", spanForNodeId(facts, info.nodeId));
      continue;
    }
    const moduleId = resolveModule(facts, info.moduleKey, info.source, pathMappings);
    const moduleBindings = facts.bindings.get(info.moduleKey) ?? new Map();
    facts.bindings.set(info.moduleKey, moduleBindings);
    for (const item of info.names) {
      const target = resolveImportTarget(facts, info.moduleKey, info.source, item, pathMappings);
      const reason = moduleResolutionReason(facts, info.moduleKey, info.source, pathMappings);
      moduleBindings.set(item.local, { targetId: target, external: !target && reason === "external-target" });
      if (target) facts.addEdge(info.ownerId, target, "imports", spanForNodeId(facts, info.nodeId));
      else facts.addUnresolved(
        info.ownerId,
        "imports",
        info.expression,
        moduleId ? "missing-target" : reason,
        spanForNodeId(facts, info.nodeId),
        `${info.source}:${item.imported}`,
      );
    }
  }
}

function resolveCommonJsBindings(facts: FactStore): void {
  for (const [moduleKey, exports] of facts.commonJsExports) {
    const bindings = facts.bindings.get(moduleKey) ?? new Map();
    facts.bindings.set(moduleKey, bindings);
    for (const [name, expression] of exports) {
      const targetId = resolveLocalExportExpression(facts, moduleKey, expression);
      if (targetId) bindings.set(name, { targetId, external: false });
    }
  }
}

function resolveReexportBindings(facts: FactStore, pathMappings: PathMapping[]): void {
  for (const reexport of facts.reexports) {
    const targetModuleKey = resolveModuleKey(facts, reexport.moduleKey, reexport.source, pathMappings);
    const target = targetModuleKey ? facts.modules.get(targetModuleKey) ?? null : null;
    if (target) facts.addEdge(reexport.ownerId, target, "imports", reexport.span);
    else facts.addUnresolved(
      reexport.ownerId,
      "imports",
      reexport.expression,
      moduleResolutionReason(facts, reexport.moduleKey, reexport.source, pathMappings),
      reexport.span,
      reexport.source,
    );
    if (!targetModuleKey) continue;
    const bindings = facts.bindings.get(reexport.moduleKey) ?? new Map();
    facts.bindings.set(reexport.moduleKey, bindings);
    for (const name of reexport.names) {
      const targetId = resolveExportedSymbol(facts, targetModuleKey, name.imported, new Set<string>(), pathMappings);
      if (targetId) bindings.set(name.exported, { targetId, external: false });
    }
  }
}

export function resolveRelationships(facts: FactStore, checker?: ts.TypeChecker): void {
  for (const relationship of facts.relationships) {
    const targetName = staticTarget(relationship.expression)
      ?? (ts.isCallExpression(relationship.expression) ? staticTarget(relationship.expression.expression) : null);
    const recordSpan = spanFor(
      relationship.expression,
      relationship.sourceFile,
      relativeSourcePath(facts, relationship.sourceFile),
    );
    if (!targetName) {
      facts.addUnresolved(
        relationship.source,
        relationship.relation,
        expressionText(relationship.expression, relationship.sourceFile),
        "unsupported-form",
        recordSpan,
      );
      continue;
    }
    const target = resolveSymbol(facts, relationship.moduleKey, relationship.scope, targetName);
    if (target) facts.addEdge(relationship.source, target, relationship.relation, recordSpan);
    else facts.addUnresolved(
      relationship.source,
      relationship.relation,
      targetName,
      isImportedExternal(facts, relationship.moduleKey, targetName) ? "external-target" : "missing-target",
      recordSpan,
      targetName,
    );
  }
  if (checker) emitOverrides(facts, checker);
}

function emitOverrides(facts: FactStore, checker: ts.TypeChecker): void {
  for (const [methodId, declarations] of facts.idDeclarations) {
    if (facts.nodes.get(methodId)?.kind !== "method") continue;
    const method = declarations.find((declaration): declaration is ts.MethodDeclaration => ts.isMethodDeclaration(declaration));
    if (!method || !ts.isClassDeclaration(method.parent) || !method.name) continue;
    const owner = method.parent;
    const ownerSymbol = owner.name ? checker.getSymbolAtLocation(owner.name) : undefined;
    if (!ownerSymbol) continue;
    const ownerType = checker.getDeclaredTypeOfSymbol(ownerSymbol);
    const methodName = method.name.getText(method.getSourceFile());
    for (const ancestor of ancestorTypes(ownerType, new Set<ts.Symbol>())) {
      const property = checker.getPropertyOfType(ancestor, methodName);
      for (const declaration of property?.declarations ?? []) {
        if (!ts.isMethodDeclaration(declaration) || !ts.isClassDeclaration(declaration.parent)) continue;
        const target = facts.declarationIds.get(declaration) ?? facts.declarationIds.get(declaration.name);
        if (target && target !== methodId) facts.addEdge(methodId, target, "overrides");
      }
    }
  }
}

function ancestorTypes(type: ts.Type, seen: Set<ts.Symbol>): ts.Type[] {
  if ((type.flags & ts.TypeFlags.Object) === 0) return [];
  const bases = (type as ts.InterfaceType).getBaseTypes() ?? [];
  const result: ts.Type[] = [];
  for (const base of bases) {
    const symbol = base.getSymbol();
    if (symbol && seen.has(symbol)) continue;
    if (symbol) seen.add(symbol);
    result.push(base, ...ancestorTypes(base, seen));
  }
  return result;
}

function resolveSymbol(facts: FactStore, moduleKey: string, scope: string[], name: string): string | null {
  const first = name.split(".")[0];
  const binding = facts.bindings.get(moduleKey)?.get(first);
  if (binding) {
    if (!binding.targetId) return null;
    if (name === first) return binding.targetId;
    const base = findQualifiedName(facts, binding.targetId);
    return facts.symbols.get(`${base}.${name.split(".").slice(1).join(".")}`) ?? null;
  }
  const candidates: string[] = [];
  for (let count = scope.length; count >= 0; count -= 1) {
    const prefix = scope.slice(0, count).join(".");
    candidates.push(`${moduleKey}.${prefix}${prefix ? "." : ""}${name}`);
  }
  candidates.push(`${moduleKey}.${name}`);
  for (const candidate of candidates) {
    const target = facts.symbols.get(candidate) ?? facts.modules.get(candidate);
    if (target) return target;
  }
  return null;
}

function findQualifiedName(facts: FactStore, id: string): string {
  for (const [qualifiedName, symbolId] of facts.symbols) if (symbolId === id) return qualifiedName;
  for (const [moduleKey, moduleId] of facts.modules) if (moduleId === id) return moduleKey;
  return "";
}

function resolveImportTarget(
  facts: FactStore,
  importer: string,
  source: string,
  item: ImportInfo["names"][number],
  pathMappings: PathMapping[],
): string | null {
  const moduleId = resolveModule(facts, importer, source, pathMappings);
  if (!moduleId) return null;
  if (item.kind === "default") {
    const targetModule = resolveModuleKey(facts, importer, source, pathMappings);
    return targetModule ? facts.defaultExportIds.get(targetModule) ?? moduleId : moduleId;
  }
  if (item.kind === "side-effect" || item.kind === "namespace") return moduleId;
  const targetModule = resolveModuleKey(facts, importer, source, pathMappings);
  return targetModule
    ? resolveExportedSymbol(facts, targetModule, item.imported, new Set<string>(), pathMappings)
    : null;
}

function resolveExportedSymbol(
  facts: FactStore,
  moduleKey: string,
  name: string,
  seen = new Set<string>(),
  pathMappings: PathMapping[] = [],
): string | null {
  const direct = facts.symbols.get(`${moduleKey}.${name}`);
  if (direct) return direct;
  const commonJsExpression = facts.commonJsExports.get(moduleKey)?.get(name);
  const commonJsTarget = commonJsExpression
    ? resolveLocalExportExpression(facts, moduleKey, commonJsExpression)
    : null;
  if (commonJsTarget) return commonJsTarget;
  const key = `${moduleKey}:${name}`;
  if (seen.has(key)) return null;
  seen.add(key);
  const binding = facts.bindings.get(moduleKey)?.get(name);
  if (binding?.targetId) return binding.targetId;
  for (const reexport of facts.reexports) {
    if (reexport.moduleKey !== moduleKey) continue;
    const exported = reexport.names.find((item) => item.exported === name);
    if (!exported) continue;
    const targetModuleKey = resolveModuleKey(facts, moduleKey, reexport.source, pathMappings);
    if (!targetModuleKey) continue;
    const target = resolveExportedSymbol(facts, targetModuleKey, exported.imported, seen, pathMappings);
    if (target) return target;
  }
  return null;
}

function resolveLocalExportExpression(facts: FactStore, moduleKey: string, expression: ts.Expression): string | null {
  const direct = facts.declarationIds.get(expression);
  if (direct) return direct;
  const targetName = staticTarget(expression);
  if (!targetName) return null;
  return facts.symbols.get(`${moduleKey}.${targetName}`) ?? null;
}

export function resolveModule(
  facts: FactStore,
  importer: string,
  source: string,
  pathMappings: PathMapping[] = [],
): string | null {
  const key = resolveModuleKey(facts, importer, source, pathMappings);
  return key ? facts.modules.get(key) ?? null : null;
}

function resolveModuleKey(
  facts: FactStore,
  importer: string,
  source: string,
  pathMappings: PathMapping[],
): string | null {
  if (!isRelative(source)) return resolvePathMappedModule(facts, importer, source, pathMappings);
  const base = moduleKeyFor(path.posix.normalize(path.posix.join(path.posix.dirname(importer), source)));
  for (const candidate of [base, `${base}/index`]) if (facts.modules.has(candidate)) return candidate;
  return null;
}

function resolvePathMappedModule(facts: FactStore, importer: string, source: string, pathMappings: PathMapping[]): string | null {
  const matches = matchingMappings(importer, source, pathMappings);
  if (matches.length === 0) {
    const direct = moduleKeyFor(source);
    return facts.modules.has(direct) ? direct : null;
  }
  const candidates = mappedCandidates(facts, source, matches);
  return candidates.size === 1 ? [...candidates][0] : null;
}

function mappedCandidates(facts: FactStore, source: string, mappings: PathMapping[]): Set<string> {
  const candidates = new Set<string>();
  for (const mapping of mappings) {
    const wildcard = mapping.pattern.indexOf("*");
    const capture = wildcard < 0
      ? ""
      : source.slice(
        mapping.pattern.slice(0, wildcard).length,
        source.length - mapping.pattern.slice(wildcard + 1).length || undefined,
      );
    for (const target of mapping.targets) {
      const substituted = wildcard < 0 ? target : target.replace("*", capture);
      const candidate = moduleKeyFor(path.posix.normalize(path.posix.join(mapping.baseUrl, substituted)));
      if (candidate === "." || candidate.startsWith("../") || path.posix.isAbsolute(candidate)) continue;
      if (facts.modules.has(candidate)) candidates.add(candidate);
      if (facts.modules.has(`${candidate}/index`)) candidates.add(`${candidate}/index`);
    }
  }
  return candidates;
}

function matchingMappings(importer: string, source: string, pathMappings: PathMapping[]): PathMapping[] {
  const matches = pathMappings.filter((mapping) => {
    if (!mappingAppliesToImporter(mapping, importer)) return false;
    const wildcard = mapping.pattern.indexOf("*");
    if (wildcard < 0) return mapping.pattern === source;
    const prefix = mapping.pattern.slice(0, wildcard);
    const suffix = mapping.pattern.slice(wildcard + 1);
    return source.startsWith(prefix)
      && source.endsWith(suffix)
      && source.length >= prefix.length + suffix.length;
  });
  if (matches.length === 0) return [];
  const scopeSpecificity = Math.max(...matches.map((mapping) => mappingScopeDepth(mapping.scope)));
  const scoped = matches.filter((mapping) => mappingScopeDepth(mapping.scope) === scopeSpecificity);
  const exact = scoped.filter((mapping) => !mapping.pattern.includes("*"));
  if (exact.length > 0) return exact;
  const specificity = Math.max(...scoped.map((mapping) => mapping.pattern.length - 1));
  return scoped.filter((mapping) => mapping.pattern.length - 1 === specificity);
}

function mappingAppliesToImporter(mapping: PathMapping, importer: string): boolean {
  return mapping.scope === "."
    || importer === mapping.scope
    || importer.startsWith(`${mapping.scope}/`);
}

function mappingScopeDepth(scope: string): number {
  return scope === "." ? 0 : scope.split("/").length;
}

function moduleResolutionReason(
  facts: FactStore,
  importer: string,
  source: string,
  pathMappings: PathMapping[],
): "missing-target" | "ambiguous-target" | "external-target" {
  if (isRelative(source)) return "missing-target";
  const matches = matchingMappings(importer, source, pathMappings);
  if (matches.length === 0) return "external-target";
  return mappedCandidates(facts, source, matches).size > 1 ? "ambiguous-target" : "missing-target";
}

function isRelative(source: string): boolean {
  return source.startsWith(".") || source.startsWith("/");
}

function isImportedExternal(facts: FactStore, moduleKey: string, targetName: string): boolean {
  return facts.bindings.get(moduleKey)?.get(targetName.split(".")[0])?.external ?? false;
}
