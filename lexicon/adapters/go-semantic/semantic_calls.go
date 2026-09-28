package main

import (
	"go/ast"
	"go/types"
	"sort"

	"golang.org/x/tools/go/packages"
)

func (index *semanticIndex) collectDirectCalls() []callsiteObservation {
	var result []callsiteObservation
	for _, job := range index.semanticFileJobs() {
		result = append(result, index.collectDirectCallsForFile(job.pkg, job.file, job.owner)...)
	}
	sortCallsiteObservations(result)
	return result
}

func (index *semanticIndex) collectDirectCallsForFile(
	pkg *packages.Package,
	file *ast.File,
	owner string,
) []callsiteObservation {
	var result []callsiteObservation
	for _, declaration := range file.Decls {
		function, ok := declaration.(*ast.FuncDecl)
		if !ok || function.Body == nil {
			continue
		}
		object, ok := pkg.TypesInfo.Defs[function.Name].(*types.Func)
		if !ok {
			continue
		}
		caller, ok := index.targetsByObject[object]
		if !ok {
			continue
		}
		index.collectCallableCalls(pkg, owner, caller.Identity, function.Body, &result)
	}
	return result
}

func (index *semanticIndex) collectCallableCalls(
	pkg *packages.Package,
	owner, caller string,
	body *ast.BlockStmt,
	result *[]callsiteObservation,
) {
	ast.Inspect(body, func(node ast.Node) bool {
		if node != body {
			switch typed := node.(type) {
			case *ast.FuncLit:
				position := pkg.Fset.PositionFor(typed.Pos(), false)
				closure := closureIdentity(moduleImportPath(index.request, owner), owner, position)
				index.collectCallableCalls(pkg, owner, closure, typed.Body, result)
				return false
			case *ast.GoStmt:
				index.registerCallsite(pkg, owner, caller, typed.Call, typed.Pos())
			case *ast.DeferStmt:
				index.registerCallsite(pkg, owner, caller, typed.Call, typed.Pos())
			}
		}
		call, ok := node.(*ast.CallExpr)
		if !ok {
			return true
		}
		location := sourceSpan(pkg.Fset, call.Pos(), call.End())
		*result = append(*result, index.resolveDirectCall(pkg, owner, caller, call, location)...)
		index.registerCallsite(pkg, owner, caller, call, call.Pos())
		if call.Lparen.IsValid() {
			index.registerCallsite(pkg, owner, caller, call, call.Lparen)
		}
		return true
	})
}

func (index *semanticIndex) resolveDirectCall(
	pkg *packages.Package,
	owner, caller string,
	call *ast.CallExpr,
	location span,
) []callsiteObservation {
	if typed, exists := pkg.TypesInfo.Types[call.Fun]; exists && typed.IsType() {
		return []callsiteObservation{resolvedCall(
			caller,
			typeIdentityFromType(index.request.Modules, typed.Type),
			"conversion",
			owner,
			location,
		)}
	}
	object := calledObject(pkg.TypesInfo, call.Fun)
	switch object := object.(type) {
	case *types.Builtin:
		return []callsiteObservation{resolvedCall(
			caller, "function:go:builtins:"+object.Name(), "builtin", owner, location,
		)}
	case *types.TypeName:
		return []callsiteObservation{resolvedCall(
			caller,
			typeIdentityFromType(index.request.Modules, object.Type()),
			"conversion",
			owner,
			location,
		)}
	case *types.Func:
		return index.resolveFunctionCall(pkg, owner, caller, call, object, location)
	case nil:
		resolution, namespace, name := classifyCallExpression(call.Fun)
		return []callsiteObservation{unresolvedCall(
			pkg.Fset, caller, owner, call,
			resolution, namespace, name, "dynamic", location,
		)}
	default:
		return []callsiteObservation{unresolvedCall(
			pkg.Fset, caller, owner, call,
			"missing", "", expressionName(call.Fun), "dynamic", location,
		)}
	}
}

func (index *semanticIndex) resolveFunctionCall(
	pkg *packages.Package,
	owner, caller string,
	call *ast.CallExpr,
	function *types.Func,
	location span,
) []callsiteObservation {
	namespace := canonicalNamespace(index.request.Modules, objectNamespace(function))
	semanticID := semanticFunctionIdentity(index.request.Modules, function)
	internal := internalNamespace(index.request.Modules, namespace)
	if isInterfaceCall(pkg.TypesInfo, call.Fun) {
		if !internal {
			return []callsiteObservation{
				resolvedCall(caller, semanticID, "interface", owner, location),
			}
		}
		if contract, named := index.targetsByObject[function]; named {
			implementations := index.interfaceImplementations[contract.Identity]
			if len(implementations) == 0 {
				return []callsiteObservation{unresolvedCall(
					pkg.Fset, caller, owner, call,
					"missing", namespace, function.Name(), "interface", location,
				)}
			}
			result := make([]callsiteObservation, 0, len(implementations))
			for _, target := range implementations {
				result = append(result, resolvedCall(
					caller, target.Identity, "interface", owner, location,
				))
			}
			return result
		}
		position := pkg.Fset.PositionFor(function.Pos(), false)
		targetOwner, ok := index.ownerForPosition(position.Filename)
		if !ok {
			return []callsiteObservation{unresolvedCall(
				pkg.Fset, caller, owner, call,
				"missing", namespace, function.Name(), "interface", location,
			)}
		}
		return []callsiteObservation{resolvedCallWithTargetProvenance(
			caller,
			semanticID,
			"interface",
			function.Name(),
			namespace,
			index.packageContainerIdentity(namespace),
			targetOwner,
			pointSpan(position),
			owner,
			location,
		)}
	}
	if internal {
		targets := index.targetCandidates(function)
		if len(targets) != 1 {
			return []callsiteObservation{unresolvedCall(
				pkg.Fset, caller, owner, call,
				"ambiguous", namespace, function.Name(), "direct", location,
			)}
		}
		return []callsiteObservation{
			resolvedCall(caller, targets[0].Identity, "direct", owner, location),
		}
	}
	return []callsiteObservation{
		resolvedCall(caller, semanticID, "direct", owner, location),
	}
}

func sortCallsiteObservations(values []callsiteObservation) {
	sort.SliceStable(values, func(i, j int) bool {
		left, right := values[i], values[j]
		if left.Owner != right.Owner {
			return left.Owner < right.Owner
		}
		if left.Span.StartLine != right.Span.StartLine {
			return left.Span.StartLine < right.Span.StartLine
		}
		if left.Span.StartColumn != right.Span.StartColumn {
			return left.Span.StartColumn < right.Span.StartColumn
		}
		if left.SourceKey != right.SourceKey {
			return left.SourceKey < right.SourceKey
		}
		return callsiteSortTarget(left) < callsiteSortTarget(right)
	})
}

func callsiteSortTarget(value callsiteObservation) string {
	if len(value.Targets) == 0 {
		return ""
	}
	return value.Targets[0].SemanticKey
}
