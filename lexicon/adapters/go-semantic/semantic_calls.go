package main

import (
	"go/ast"
	"go/types"
	"sort"

	"golang.org/x/tools/go/packages"
)

func (index *semanticIndex) collectDirectCalls() []semanticRecord {
	var result []semanticRecord
	for _, pkg := range index.packages {
		if pkg.TypesInfo == nil || pkg.Fset == nil {
			continue
		}
		for _, file := range pkg.Syntax {
			owner, ok := index.ownerForPosition(pkg.Fset.PositionFor(file.Pos(), false).Filename)
			if !ok {
				continue
			}
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
		}
	}
	sortSemanticCallRecords(result)
	return result
}

func (index *semanticIndex) collectCallableCalls(
	pkg *packages.Package,
	owner, caller string,
	body *ast.BlockStmt,
	result *[]semanticRecord,
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
		*result = append(*result, index.resolveDirectCall(pkg, owner, caller, call, location))
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
) semanticRecord {
	if typed, exists := pkg.TypesInfo.Types[call.Fun]; exists && typed.IsType() {
		return callRecord(caller, typeIdentityFromType(index.request.Modules, typed.Type),
			"conversion", "conversion", owner, location)
	}
	object := calledObject(pkg.TypesInfo, call.Fun)
	switch object := object.(type) {
	case *types.Builtin:
		return callRecord(caller, "function:go:builtins:"+object.Name(),
			"definite", "builtin", owner, location)
	case *types.TypeName:
		return callRecord(caller, typeIdentityFromType(index.request.Modules, object.Type()),
			"conversion", "conversion", owner, location)
	case *types.Func:
		return index.resolveFunctionCall(pkg, owner, caller, call, object, location)
	case nil:
		reason, namespace, name := classifyCallExpression(call.Fun)
		return unresolvedForPackage(pkg.Fset, caller, owner, call, reason, namespace, name, "dynamic", location)
	default:
		return unresolvedForPackage(pkg.Fset, caller, owner, call, "dynamic-target", "",
			expressionName(call.Fun), "dynamic", location)
	}
}

func (index *semanticIndex) resolveFunctionCall(
	pkg *packages.Package,
	owner, caller string,
	call *ast.CallExpr,
	function *types.Func,
	location span,
) semanticRecord {
	namespace := canonicalNamespace(index.request.Modules, objectNamespace(function))
	if isInterfaceCall(pkg.TypesInfo, call.Fun) {
		return unresolvedForPackage(pkg.Fset, caller, owner, call, "dynamic-target", namespace,
			function.Name(), "interface", location)
	}
	semanticID := semanticFunctionIdentity(index.request.Modules, function)
	if internalNamespace(index.request.Modules, namespace) {
		targets := index.targetCandidates(function)
		if len(targets) != 1 {
			return unresolvedForPackage(pkg.Fset, caller, owner, call, "ambiguous-target", namespace,
				function.Name(), "internal", location)
		}
		return callRecord(caller, targets[0].Identity, "definite", "internal", owner, location)
	}
	return callRecord(caller, semanticID, "definite", "external", owner, location)
}

func sortSemanticCallRecords(records []semanticRecord) {
	sort.SliceStable(records, func(i, j int) bool {
		leftOwner, leftSpan := recordLocation(records[i])
		rightOwner, rightSpan := recordLocation(records[j])
		if leftOwner != rightOwner {
			return leftOwner < rightOwner
		}
		if leftSpan.StartLine != rightSpan.StartLine {
			return leftSpan.StartLine < rightSpan.StartLine
		}
		if leftSpan.StartColumn != rightSpan.StartColumn {
			return leftSpan.StartColumn < rightSpan.StartColumn
		}
		if recordSource(records[i]) != recordSource(records[j]) {
			return recordSource(records[i]) < recordSource(records[j])
		}
		return recordTarget(records[i]) < recordTarget(records[j])
	})
}
