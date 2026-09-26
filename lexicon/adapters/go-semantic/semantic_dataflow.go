package main

import (
	"fmt"
	"go/ast"
	"go/token"
	"go/types"
	"sort"

	"golang.org/x/tools/go/packages"
)

// collectDataflow preserves the legacy adapter's conservative typed reads/writes.
// It deliberately reports only repository-local go/types objects and skips nested
// function literals; closure free-variable evidence remains the SSA capture pass.
func (index *semanticIndex) collectDataflow() []semanticRecord {
	var result []semanticRecord
	for _, job := range index.semanticFileJobs() {
		result = append(result, index.collectDataflowForFile(job.pkg, job.file, job.owner)...)
	}
	sortDataflowRecords(result)
	return result
}

func (index *semanticIndex) collectDataflowForFile(
	pkg *packages.Package,
	file *ast.File,
	owner string,
) []semanticRecord {
	var result []semanticRecord
	for _, declaration := range file.Decls {
		function, ok := declaration.(*ast.FuncDecl)
		if !ok || function.Body == nil {
			continue
		}
		object, ok := pkg.TypesInfo.Defs[function.Name].(*types.Func)
		if !ok {
			continue
		}
		source, ok := index.targetsByObject[object]
		if !ok {
			continue
		}
		visitor := dataflowVisitor{
			index: index, pkg: pkg, owner: owner, source: source.Identity,
			result: &result,
		}
		visitor.visitBlock(function.Body)
	}
	return result
}

type dataflowVisitor struct {
	index  *semanticIndex
	pkg    *packages.Package
	owner  string
	source string
	result *[]semanticRecord
}

func (visitor *dataflowVisitor) visitBlock(block *ast.BlockStmt) {
	for _, statement := range block.List {
		visitor.visitStmt(statement)
	}
}

func (visitor *dataflowVisitor) visitStmt(statement ast.Stmt) {
	switch statement := statement.(type) {
	case *ast.AssignStmt:
		for _, value := range statement.Rhs {
			visitor.visitExpr(value)
		}
		for _, target := range statement.Lhs {
			visitor.visitTarget(target, statement.Tok != token.ASSIGN)
		}
	case *ast.IncDecStmt:
		visitor.visitTarget(statement.X, true)
	case *ast.DeclStmt:
		if declaration, ok := statement.Decl.(*ast.GenDecl); ok {
			visitor.visitGenDecl(declaration)
		}
	default:
		ast.Inspect(statement, func(node ast.Node) bool {
			if node == statement {
				return true
			}
			if nested, ok := node.(ast.Stmt); ok {
				visitor.visitStmt(nested)
				return false
			}
			if _, nested := node.(*ast.FuncLit); nested {
				return false
			}
			if expression, ok := node.(ast.Expr); ok {
				visitor.visitExpr(expression)
				return false
			}
			return true
		})
	}
}

func (visitor *dataflowVisitor) visitGenDecl(declaration *ast.GenDecl) {
	for _, specification := range declaration.Specs {
		value, ok := specification.(*ast.ValueSpec)
		if !ok {
			continue
		}
		for _, initializer := range value.Values {
			visitor.visitExpr(initializer)
		}
		for _, name := range value.Names {
			visitor.visitTarget(name, false)
		}
	}
}

func (visitor *dataflowVisitor) visitTarget(expression ast.Expr, compound bool) {
	switch expression := expression.(type) {
	case *ast.Ident:
		if compound {
			visitor.addObject(expression, true)
		}
		visitor.addObject(expression, false, true)
	case *ast.SelectorExpr:
		visitor.visitExpr(expression.X)
		if compound {
			visitor.addObject(expression.Sel, true)
		}
		visitor.addObject(expression.Sel, false, true)
	case *ast.ParenExpr:
		visitor.visitTarget(expression.X, compound)
	case *ast.StarExpr:
		visitor.visitExpr(expression.X)
	case *ast.IndexExpr:
		visitor.visitExpr(expression.X)
		visitor.visitExpr(expression.Index)
	case *ast.IndexListExpr:
		visitor.visitExpr(expression.X)
		for _, index := range expression.Indices {
			visitor.visitExpr(index)
		}
	default:
		visitor.visitExpr(expression)
	}
}

func (visitor *dataflowVisitor) visitExpr(expression ast.Expr) {
	switch expression := expression.(type) {
	case *ast.Ident:
		visitor.addObject(expression, false)
	case *ast.SelectorExpr:
		visitor.visitExpr(expression.X)
		visitor.addObject(expression.Sel, false)
	case *ast.FuncLit:
		return
	default:
		ast.Inspect(expression, func(node ast.Node) bool {
			if node == expression {
				return true
			}
			if _, nested := node.(*ast.FuncLit); nested {
				return false
			}
			if identifier, ok := node.(*ast.Ident); ok {
				visitor.addObject(identifier, false)
				return false
			}
			if selector, ok := node.(*ast.SelectorExpr); ok {
				visitor.visitExpr(selector)
				return false
			}
			return true
		})
	}
}

func (visitor *dataflowVisitor) addObject(identifier *ast.Ident, write bool, forceWrite ...bool) {
	object := visitor.pkg.TypesInfo.ObjectOf(identifier)
	if object == nil || object.Pkg() == nil {
		return
	}
	namespace := object.Pkg().Path()
	if !internalNamespace(visitor.index.request.Modules,
		canonicalNamespace(visitor.index.request.Modules, namespace)) {
		return
	}
	position := visitor.pkg.Fset.PositionFor(object.Pos(), false)
	if !position.IsValid() {
		return
	}
	targetOwner, ok := visitor.index.ownerForPosition(position.Filename)
	if !ok {
		return
	}
	kind := "variable"
	switch value := object.(type) {
	case *types.Const:
		kind = "constant"
	case *types.Var:
		if value.IsField() {
			kind = "field"
		}
	}
	target := fmt.Sprintf("%s:%s:%s:%d:%d:%s",
		kind, namespace, targetOwner, position.Line, position.Column, object.Name())
	relation := "read"
	if write || (len(forceWrite) > 0 && forceWrite[0]) {
		relation = "write"
	}
	*visitor.result = append(*visitor.result, dataflowObservation{
		Record: "dataflow", Source: visitor.source, Target: target, Kind: relation,
		Owner: visitor.owner, Span: sourceSpan(visitor.pkg.Fset, identifier.Pos(), identifier.End()),
	})
}

func sortDataflowRecords(records []semanticRecord) {
	sort.SliceStable(records, func(i, j int) bool {
		left := records[i].(dataflowObservation)
		right := records[j].(dataflowObservation)
		if left.Owner != right.Owner {
			return left.Owner < right.Owner
		}
		if left.Span.StartLine != right.Span.StartLine {
			return left.Span.StartLine < right.Span.StartLine
		}
		if left.Span.StartColumn != right.Span.StartColumn {
			return left.Span.StartColumn < right.Span.StartColumn
		}
		if left.Source != right.Source {
			return left.Source < right.Source
		}
		if left.Target != right.Target {
			return left.Target < right.Target
		}
		return left.Kind < right.Kind
	})
}
