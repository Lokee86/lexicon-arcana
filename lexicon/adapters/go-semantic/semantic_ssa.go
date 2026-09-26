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
	Class     string
	Name      string
	Namespace string
	Container string
	Internal  bool
	Generated bool
}

func (index *semanticIndex) mergeSSASemantics(direct []semanticRecord) []semanticRecord {
	if len(index.roots) == 0 {
		return direct
	}
	var captures []semanticRecord
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
	var result []semanticRecord
	for _, target := range sortedSSATargets(materializations) {
		result = append(result, targetObservation{
			Record: "target", Identity: target.Identity, Class: target.Class,
			Name: target.Name, Namespace: target.Namespace, Container: target.Container,
		})
	}
	if len(direct) != 0 {
		result = append(result, mergeSSAOutcomes(direct, outcomes)...)
	}
	result = append(result, captures...)
	return result
}

func mergeSSAOutcomes(
	direct []semanticRecord,
	outcomes map[string]*ssaOutcome,
) []semanticRecord {
	byKey := make(map[string][]semanticRecord)
	for _, record := range direct {
		key := recordCallsiteKey(record)
		byKey[key] = append(byKey[key], record)
	}
	keys := make([]string, 0, len(outcomes))
	for key := range outcomes {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	for _, key := range keys {
		existing := byKey[key]
		if len(existing) == 0 {
			continue
		}
		outcome := outcomes[key]
		if !outcome.Invoke && hasResolvedCall(existing) {
			continue
		}
		targets := sortedSSATargets(outcome.Targets)
		if len(targets) == 0 {
			continue
		}
		source := recordSource(existing[0])
		owner, location := recordLocation(existing[0])
		kind := "possible"
		if len(targets) == 1 {
			kind = "definite"
		}
		class := "dynamic"
		if outcome.Invoke {
			class = "interface"
		}
		replacement := make([]semanticRecord, 0, len(targets)+len(existing))
		if outcome.Invoke {
			for _, record := range existing {
				call, resolved := record.(callObservation)
				if resolved && !strings.HasPrefix(call.Target, "interface-method:") {
					replacement = append(replacement, record)
				}
			}
		}
		for _, target := range targets {
			replacement = append(replacement, callRecordWithTarget(
				source, target.Identity, kind, class,
				target.Name, target.Namespace, target.Container, owner, location,
			))
		}
		byKey[key] = mergeDirectCallRecords(replacement)
	}
	var result []semanticRecord
	for _, records := range byKey {
		result = append(result, records...)
	}
	sortSemanticCallRecords(result)
	return result
}

func hasResolvedCall(records []semanticRecord) bool {
	for _, record := range records {
		if _, ok := record.(callObservation); ok {
			return true
		}
	}
	return false
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
