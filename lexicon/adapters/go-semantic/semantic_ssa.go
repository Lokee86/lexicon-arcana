package main

import (
	"sort"
	"strings"

	"golang.org/x/tools/go/callgraph/vta"
	"golang.org/x/tools/go/ssa"
	"golang.org/x/tools/go/ssa/ssautil"
)

type ssaOutcome struct {
	Invoke  bool
	Targets map[string]ssaTarget
}

type ssaTarget struct {
	Identity  string
	Name      string
	Namespace string
	Container string
	Internal  bool
	Generated bool
}

func (index *semanticIndex) mergeSSASemantics(
	direct []callsiteObservation,
	previouslyResolved map[string]bool,
) []observation {
	if len(index.roots) == 0 {
		return callsitesAsObservations(direct)
	}
	var captures []captureObservation
	outcomes := make(map[string]*ssaOutcome)
	materializations := make(map[string]ssaTarget)
	for _, target := range index.generatedTestMainTargets() {
		materializations[target.Identity] = target
	}
	program, _ := ssautil.AllPackages(index.roots, ssa.InstantiateGenerics)
	program.Build()
	functions := ssautil.AllFunctions(program)
	captures = append(captures, index.collectSSACaptures(functions, program.Fset)...)
	graph := vta.CallGraph(functions, nil)
	for _, node := range graph.Nodes {
		for _, edge := range node.Out {
			if edge.Site == nil || edge.Callee == nil || edge.Caller == nil {
				continue
			}
			if generated, ok := index.generatedTestMainTarget(edge.Caller.Func); ok {
				materializations[generated.Identity] = generated
			}
			source, generatedSource, ok := index.ssaSourceIdentity(edge.Caller.Func, program.Fset)
			if !ok {
				continue
			}
			if generatedSource != nil {
				materializations[generatedSource.Identity] = *generatedSource
			}
			position := program.Fset.PositionFor(edge.Site.Pos(), false)
			owner, ok := index.ownerForPosition(position.Filename)
			if !ok {
				continue
			}
			key, exists := index.callsiteKeys[callsiteStartKey(source, owner, position)]
			if !exists {
				continue
			}
			target, ok := index.ssaTargetIdentity(edge.Callee.Func, program.Fset)
			if !ok {
				continue
			}
			if target.Generated || !target.Internal {
				materializations[target.Identity] = target
			}
			common := edge.Site.Common()
			outcome := outcomes[key]
			if outcome == nil {
				outcome = &ssaOutcome{Targets: make(map[string]ssaTarget)}
				outcomes[key] = outcome
			}
			outcome.Invoke = outcome.Invoke || common.IsInvoke()
			if !common.IsInvoke() || target.Internal {
				outcome.Targets[target.Identity] = target
			}
		}
	}

	result := make([]observation, 0, len(materializations)+len(direct)+len(captures))
	for _, target := range sortedSSATargets(materializations) {
		result = append(result, symbolObservation{
			Observation:  "symbol",
			SemanticKey:  target.Identity,
			Name:         target.Name,
			Namespace:    target.Namespace,
			ContainerKey: target.Container,
			Generated:    target.Generated,
		})
	}
	if len(direct) != 0 {
		for _, call := range mergeSSAOutcomes(direct, outcomes, previouslyResolved) {
			result = append(result, call)
		}
	}
	for _, capture := range captures {
		result = append(result, capture)
	}
	return result
}

func mergeSSAOutcomes(
	direct []callsiteObservation,
	outcomes map[string]*ssaOutcome,
	previouslyResolved map[string]bool,
) []callsiteObservation {
	byKey := make(map[string]callsiteObservation, len(direct))
	order := make([]string, 0, len(direct))
	for _, value := range direct {
		key := callsiteObservationKey(value)
		if _, exists := byKey[key]; !exists {
			order = append(order, key)
		}
		if existing, exists := byKey[key]; exists {
			byKey[key] = mergeCallsiteObservations(existing, value)
		} else {
			byKey[key] = normalizedCallsite(value)
		}
	}

	keys := make([]string, 0, len(outcomes))
	for key := range outcomes {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	for _, key := range keys {
		existing, exists := byKey[key]
		if !exists {
			continue
		}
		outcome := outcomes[key]
		if !outcome.Invoke && (hasResolvedCall(existing) || previouslyResolved[key]) {
			continue
		}
		targets := sortedSSATargets(outcome.Targets)
		if len(targets) == 0 {
			continue
		}

		form := "dynamic"
		if outcome.Invoke {
			form = "interface"
		}
		replacement := callsiteObservation{
			Observation: "callsite",
			SourceKey:   existing.SourceKey,
			Form:        form,
			Resolution:  "resolved",
			Owner:       existing.Owner,
			Span:        existing.Span,
		}

		hasConcreteExisting := false
		if outcome.Invoke && hasResolvedCall(existing) {
			for _, target := range existing.Targets {
				if strings.HasPrefix(target.SemanticKey, "interface-method:") {
					continue
				}
				replacement.Targets = append(replacement.Targets, target)
				hasConcreteExisting = true
			}
		}
		for _, target := range targets {
			if outcome.Invoke && hasConcreteExisting &&
				strings.HasPrefix(target.Identity, "interface-method:") {
				continue
			}
			replacement.Targets = append(replacement.Targets, callTargetObservation{
				SemanticKey:  target.Identity,
				Name:         target.Name,
				Namespace:    target.Namespace,
				ContainerKey: target.Container,
				Generated:    target.Generated,
			})
		}
		replacement.Form = mergeCallForm(existing.Form, replacement.Form)
		byKey[key] = normalizedCallsite(replacement)
	}

	result := make([]callsiteObservation, 0, len(byKey))
	for _, key := range order {
		result = append(result, byKey[key])
	}
	sortCallsiteObservations(result)
	return result
}

func hasResolvedCall(value callsiteObservation) bool {
	return value.Resolution == "resolved" && len(value.Targets) != 0
}

func sortedSSATargets(values map[string]ssaTarget) []ssaTarget {
	result := make([]ssaTarget, 0, len(values))
	for _, value := range values {
		result = append(result, value)
	}
	sort.Slice(result, func(i, j int) bool {
		return result[i].Identity < result[j].Identity
	})
	return result
}
