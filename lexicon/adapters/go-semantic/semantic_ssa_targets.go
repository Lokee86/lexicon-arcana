package main

import (
	"go/ast"
	"go/token"
	"go/types"
	"path"
	"strconv"
	"strings"

	"golang.org/x/tools/go/ssa"
)

func (index *semanticIndex) ssaSourceIdentity(
	function *ssa.Function,
	set *token.FileSet,
) (string, *ssaTarget, bool) {
	if function == nil {
		return "", nil, false
	}
	if object := function.Object(); object != nil {
		typed, ok := object.(*types.Func)
		if !ok {
			return "", nil, false
		}
		namespace := canonicalNamespace(index.request.Modules, objectNamespace(typed))
		if !internalNamespace(index.request.Modules, namespace) {
			return "", nil, false
		}
		targets := index.targetCandidates(typed)
		if len(targets) > 1 {
			return "", nil, false
		}
		if len(targets) == 1 {
			return targets[0].Identity, nil, true
		}
		generated := index.generatedInternalTarget(typed, namespace)
		return generated.Identity, &generated, true
	}
	literal, ok := function.Syntax().(*ast.FuncLit)
	if !ok {
		return "", nil, false
	}
	position := set.PositionFor(literal.Pos(), false)
	owner, ok := index.ownerForPosition(position.Filename)
	if !ok {
		return "", nil, false
	}
	identity := closureIdentity(moduleImportPath(index.request, owner), owner, position)
	if !index.structuralClosures[identity] {
		return "", nil, false
	}
	return identity, nil, true
}

func (index *semanticIndex) ssaTargetIdentity(
	function *ssa.Function,
	set *token.FileSet,
) (ssaTarget, bool) {
	if function == nil {
		return ssaTarget{}, false
	}
	if object := function.Object(); object != nil {
		typed, ok := object.(*types.Func)
		if !ok {
			return ssaTarget{}, false
		}
		namespace := canonicalNamespace(index.request.Modules, objectNamespace(typed))
		internal := internalNamespace(index.request.Modules, namespace)
		if internal {
			targets := index.targetCandidates(typed)
			if len(targets) > 1 {
				return ssaTarget{}, false
			}
			if len(targets) == 1 {
				return ssaTarget{
					Identity: targets[0].Identity,
					Name:     typed.Name(), Namespace: namespace, Internal: true,
				}, true
			}
			return index.generatedInternalTarget(typed, namespace), true
		}
		return ssaTarget{
			Identity: semanticFunctionIdentity(index.request.Modules, typed),
			Name:     typed.Name(), Namespace: namespace,
		}, true
	}
	if literal, ok := function.Syntax().(*ast.FuncLit); ok {
		position := set.PositionFor(literal.Pos(), false)
		if owner, exists := index.ownerForPosition(position.Filename); exists {
			namespace := moduleImportPath(index.request, owner)
			identity := closureIdentity(namespace, owner, position)
			if index.structuralClosures[identity] {
				return ssaTarget{
					Identity:  identity,
					Name:      function.Name(),
					Namespace: namespace, Internal: true,
				}, true
			}
		}
	}
	namespace := index.ssaFunctionNamespace(function)
	if namespace == "" {
		namespace = "go:ssa"
	}
	internal := internalNamespace(index.request.Modules, namespace)
	identity := "ssa-function:" + namespace + ":" + function.String()
	if position := set.PositionFor(function.Pos(), false); position.IsValid() {
		identity += ":" + strconv.Itoa(position.Line) + ":" + strconv.Itoa(position.Column)
	}
	name := function.Name()
	if name == "" {
		name = function.String()
	}
	container := ""
	if internal {
		container = index.packageContainerIdentity(namespace)
	}
	return ssaTarget{
		Identity: identity, Name: name,
		Namespace: namespace, Container: container, Internal: internal,
		Generated: true,
	}, true
}

func (index *semanticIndex) generatedTestMainTargets() []ssaTarget {
	byIdentity := make(map[string]ssaTarget)
	for _, pkg := range index.packages {
		if pkg == nil || pkg.Types == nil || pkg.Name != "main" {
			continue
		}
		namespace := canonicalNamespace(index.request.Modules, pkg.Types.Path())
		if !strings.HasSuffix(namespace, ".test") || !internalNamespace(index.request.Modules, namespace) {
			continue
		}
		object, ok := pkg.Types.Scope().Lookup("main").(*types.Func)
		if !ok || len(index.targetCandidates(object)) != 0 {
			continue
		}
		target := index.generatedInternalTarget(object, namespace)
		byIdentity[target.Identity] = target
	}
	return sortedSSATargets(byIdentity)
}

func (index *semanticIndex) generatedTestMainTarget(function *ssa.Function) (ssaTarget, bool) {
	if function == nil || function.Name() != "main" {
		return ssaTarget{}, false
	}
	object, ok := function.Object().(*types.Func)
	if !ok {
		return ssaTarget{}, false
	}
	namespace := canonicalNamespace(index.request.Modules, objectNamespace(object))
	if !strings.HasSuffix(namespace, ".test") || !internalNamespace(index.request.Modules, namespace) {
		return ssaTarget{}, false
	}
	if len(index.targetCandidates(object)) != 0 {
		return ssaTarget{}, false
	}
	return index.generatedInternalTarget(object, namespace), true
}

func (index *semanticIndex) generatedInternalTarget(
	function *types.Func,
	namespace string,
) ssaTarget {
	return ssaTarget{
		Identity:  semanticFunctionIdentity(index.request.Modules, function),
		Name:      function.Name(),
		Namespace: namespace,
		Container: index.packageContainerIdentity(namespace),
		Internal:  true,
		Generated: true,
	}
}

func (index *semanticIndex) packageContainerIdentity(namespace string) string {
	expectedName := path.Base(namespace)
	selectedName := ""
	selectedScore := -1
	for _, pkg := range index.packages {
		if pkg.Types == nil {
			continue
		}
		candidateNamespace := canonicalNamespace(index.request.Modules, pkg.Types.Path())
		if candidateNamespace != namespace {
			continue
		}
		score := 0
		if !strings.HasSuffix(pkg.Name, "_test") {
			score++
		}
		if pkg.Name == expectedName {
			score += 2
		}
		if score > selectedScore || (score == selectedScore && (selectedName == "" || pkg.Name < selectedName)) {
			selectedName = pkg.Name
			selectedScore = score
		}
	}
	if selectedScore < 0 {
		return ""
	}
	return packageIdentity(namespace, selectedName)
}

func (index *semanticIndex) ssaFunctionNamespace(function *ssa.Function) string {
	for current := function; current != nil; current = current.Parent() {
		if current.Pkg != nil && current.Pkg.Pkg != nil {
			return canonicalNamespace(index.request.Modules, current.Pkg.Pkg.Path())
		}
		if object := current.Object(); object != nil && object.Pkg() != nil {
			return canonicalNamespace(index.request.Modules, object.Pkg().Path())
		}
	}
	return ""
}
