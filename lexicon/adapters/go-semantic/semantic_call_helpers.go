package main

import (
	"bytes"
	"go/ast"
	"go/printer"
	"go/token"
	"go/types"
)

func callRecord(source, target, kind, class, owner string, location span) callObservation {
	return callObservation{
		Record: "call", Source: source, Target: target, Kind: kind,
		Class: class, Owner: owner, Span: location,
	}
}

func unresolvedForPackage(
	set *token.FileSet,
	source, owner string,
	call *ast.CallExpr,
	reason, namespace, name, class string,
	location span,
) unresolvedObservation {
	return unresolvedObservation{
		Record: "unresolved", Source: source, Relation: "calls",
		Expression:         expressionText(set, call.Fun),
		CandidateNamespace: namespace, CandidateName: name,
		Reason: reason, Class: class, Owner: owner, Span: location,
	}
}

func calledObject(info *types.Info, expression ast.Expr) types.Object {
	switch expression := expression.(type) {
	case *ast.Ident:
		return info.Uses[expression]
	case *ast.SelectorExpr:
		if selection := info.Selections[expression]; selection != nil {
			return selection.Obj()
		}
		return info.Uses[expression.Sel]
	case *ast.IndexExpr:
		return calledObject(info, expression.X)
	case *ast.IndexListExpr:
		return calledObject(info, expression.X)
	case *ast.ParenExpr:
		return calledObject(info, expression.X)
	default:
		return nil
	}
}

func isInterfaceCall(info *types.Info, expression ast.Expr) bool {
	for {
		switch typed := expression.(type) {
		case *ast.IndexExpr:
			expression = typed.X
		case *ast.IndexListExpr:
			expression = typed.X
		case *ast.ParenExpr:
			expression = typed.X
		default:
			selector, ok := expression.(*ast.SelectorExpr)
			if !ok {
				return false
			}
			selection := info.Selections[selector]
			return selection != nil && isInterfaceType(selection.Recv())
		}
	}
}

func isInterfaceType(value types.Type) bool {
	value = types.Unalias(value)
	if pointer, ok := value.(*types.Pointer); ok {
		value = types.Unalias(pointer.Elem())
	}
	_, ok := value.Underlying().(*types.Interface)
	return ok
}

func typeIdentityFromType(modules []module, value types.Type) string {
	value = types.Unalias(value)
	for {
		pointer, ok := value.(*types.Pointer)
		if !ok {
			break
		}
		value = types.Unalias(pointer.Elem())
	}
	if named, ok := value.(*types.Named); ok {
		return typeIdentity(modules, named)
	}
	name := types.TypeString(value, func(pkg *types.Package) string { return pkg.Path() })
	return "type-expression:" + name
}

func classifyCallExpression(expression ast.Expr) (string, string, string) {
	if selector, ok := expression.(*ast.SelectorExpr); ok {
		return "unsupported-form", expressionName(selector.X), selector.Sel.Name
	}
	return "dynamic-target", "", expressionName(expression)
}

func expressionText(set *token.FileSet, expression ast.Expr) string {
	var output bytes.Buffer
	if err := printer.Fprint(&output, set, expression); err != nil {
		return expressionName(expression)
	}
	return output.String()
}

func recordLocation(record semanticRecord) (string, span) {
	switch value := record.(type) {
	case callObservation:
		return value.Owner, value.Span
	case unresolvedObservation:
		return value.Owner, value.Span
	default:
		return "", span{}
	}
}

func recordSource(record semanticRecord) string {
	switch value := record.(type) {
	case callObservation:
		return value.Source
	case unresolvedObservation:
		return value.Source
	default:
		return ""
	}
}
