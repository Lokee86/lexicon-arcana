package main

import (
	"sort"

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
}

func (index *semanticIndex) mergeSSACalls(direct []semanticRecord) []semanticRecord {
	if len(index.roots) == 0 || len(direct) == 0 {
		return direct
	}
	program, _ := ssautil.AllPackages(index.roots, ssa.InstantiateGenerics)
	program.Build()
	functions := ssautil.AllFunctions(program)
	graph := vta.CallGraph(functions, nil)
	outcomes := make(map[string]*ssaOutcome)

	for _, node := range graph.Nodes {
		for _, edge := range node.Out {
			if edge.Site == nil || edge.Callee == nil || edge.Caller == nil {
				continue
			}
			source, ok := index.ssaSourceIdentity(edge.Caller.Func, program.Fset)
			if !ok {
				continue
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
	return mergeSSAOutcomes(direct, outcomes)
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
		replacement := make([]semanticRecord, 0, len(targets))
		for _, target := range targets {
			replacement = append(replacement, callRecordWithTarget(
				source, target.Identity, kind, class,
				target.Name, target.Namespace, target.Container, owner, location,
			))
		}
		byKey[key] = replacement
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
