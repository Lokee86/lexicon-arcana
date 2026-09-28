package main

import (
	"fmt"
	"go/ast"
	"go/token"

	"golang.org/x/tools/go/packages"
)

func (index *semanticIndex) registerCallsite(
	pkg *packages.Package,
	owner, source string,
	call *ast.CallExpr,
	position token.Pos,
) {
	location := sourceSpan(pkg.Fset, call.Pos(), call.End())
	key := callsiteRecordKey(source, owner, location)
	start := pkg.Fset.PositionFor(position, false)
	index.callsiteKeys[callsiteStartKey(source, owner, start)] = key
}

func callsiteRecordKey(source, owner string, location span) string {
	return fmt.Sprintf(
		"%s\x00%s\x00%d\x00%d\x00%d\x00%d",
		source, owner,
		location.StartLine, location.StartColumn,
		location.EndLine, location.EndColumn,
	)
}

func callsiteStartKey(source, owner string, position token.Position) string {
	return fmt.Sprintf(
		"%s\x00%s\x00%d\x00%d",
		source, owner, position.Line, position.Column,
	)
}

func callsiteObservationKey(value callsiteObservation) string {
	return callsiteRecordKey(value.SourceKey, value.Owner, value.Span)
}
