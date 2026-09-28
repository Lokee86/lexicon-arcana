package main

import (
	"fmt"
	"go/ast"
	"go/token"
	"sort"

	"golang.org/x/tools/go/ssa"
)

func (index *semanticIndex) collectSSACaptures(
	functions map[*ssa.Function]bool,
	set *token.FileSet,
) []captureObservation {
	ordered := make([]*ssa.Function, 0, len(functions))
	for function := range functions {
		ordered = append(ordered, function)
	}
	sort.Slice(ordered, func(i, j int) bool {
		return ordered[i].String() < ordered[j].String()
	})

	var result []captureObservation
	for _, function := range ordered {
		literal, ok := function.Syntax().(*ast.FuncLit)
		if !ok || len(function.FreeVars) == 0 {
			continue
		}
		position := set.PositionFor(literal.Pos(), false)
		owner, ok := index.ownerForPosition(position.Filename)
		if !ok {
			continue
		}
		namespace := moduleImportPath(index.request, owner)
		closure := closureIdentity(namespace, owner, position)
		if !index.structuralClosures[closure] {
			continue
		}

		for captureIndex, variable := range function.FreeVars {
			value := captureObservation{
				Observation:  "capture",
				SourceKey:    closure,
				TargetName:   variable.Name(),
				CaptureIndex: captureIndex,
				Owner:        owner,
			}
			variablePosition := set.PositionFor(variable.Pos(), false)
			if variablePosition.IsValid() {
				if variableOwner, exists := index.ownerForPosition(variablePosition.Filename); exists {
					value.Owner = variableOwner
					value.TargetKey = fmt.Sprintf(
						"variable:%s:%s:%d:%d:%s",
						moduleImportPath(index.request, variableOwner),
						variableOwner,
						variablePosition.Line,
						variablePosition.Column,
						variable.Name(),
					)
					evidence := pointSpan(variablePosition)
					value.Span = &evidence
				}
			}
			result = append(result, value)
		}
	}
	sort.SliceStable(result, func(i, j int) bool {
		if result[i].SourceKey != result[j].SourceKey {
			return result[i].SourceKey < result[j].SourceKey
		}
		return result[i].CaptureIndex < result[j].CaptureIndex
	})
	return result
}
