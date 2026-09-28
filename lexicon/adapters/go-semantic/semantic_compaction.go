package main

import "sort"

type directCallAccumulator struct {
	byKey map[string]callsiteObservation
	order []string
	raw   int
}

func (accumulator *directCallAccumulator) addObservations(values []callsiteObservation) {
	accumulator.raw += len(values)
	for _, value := range values {
		accumulator.add(value)
	}
}

func (accumulator *directCallAccumulator) add(value callsiteObservation) {
	if accumulator.byKey == nil {
		accumulator.byKey = make(map[string]callsiteObservation)
	}
	key := callsiteObservationKey(value)
	existing, found := accumulator.byKey[key]
	if !found {
		accumulator.order = append(accumulator.order, key)
		accumulator.byKey[key] = normalizedCallsite(value)
		return
	}
	accumulator.byKey[key] = mergeCallsiteObservations(existing, value)
}

func mergeCallsiteObservations(
	existing callsiteObservation,
	incoming callsiteObservation,
) callsiteObservation {
	existingResolved := len(existing.Targets) != 0
	incomingResolved := len(incoming.Targets) != 0
	switch {
	case existingResolved && !incomingResolved:
		return existing
	case !existingResolved && incomingResolved:
		return normalizedCallsite(incoming)
	case !existingResolved && !incomingResolved:
		return existing
	}

	existing.Form = mergeCallForm(existing.Form, incoming.Form)
	existing.Resolution = "resolved"
	for _, target := range incoming.Targets {
		if !hasCallTarget(existing.Targets, target.SemanticKey) {
			existing.Targets = append(existing.Targets, target)
		}
	}
	existing.Expression = ""
	existing.CandidateNamespace = ""
	existing.CandidateName = ""
	return normalizedCallsite(existing)
}

func normalizedCallsite(value callsiteObservation) callsiteObservation {
	if len(value.Targets) != 0 {
		value.Resolution = "resolved"
		value.Expression = ""
		value.CandidateNamespace = ""
		value.CandidateName = ""
		value.Targets = normalizedCallTargets(value.Targets)
	}
	return value
}

func normalizedCallTargets(values []callTargetObservation) []callTargetObservation {
	byKey := make(map[string]callTargetObservation, len(values))
	for _, value := range values {
		if existing, ok := byKey[value.SemanticKey]; ok {
			byKey[value.SemanticKey] = mergeCallTargetEvidence(existing, value)
			continue
		}
		byKey[value.SemanticKey] = value
	}
	keys := make([]string, 0, len(byKey))
	for key := range byKey {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	result := make([]callTargetObservation, 0, len(keys))
	for _, key := range keys {
		result = append(result, byKey[key])
	}
	return result
}

func mergeCallTargetEvidence(
	left callTargetObservation,
	right callTargetObservation,
) callTargetObservation {
	if left.Name == "" {
		left.Name = right.Name
	}
	if left.Namespace == "" {
		left.Namespace = right.Namespace
	}
	if left.ContainerKey == "" {
		left.ContainerKey = right.ContainerKey
	}
	if left.Owner == "" {
		left.Owner = right.Owner
	}
	if left.Span == nil {
		left.Span = right.Span
	}
	left.Generated = left.Generated || right.Generated
	return left
}

func hasCallTarget(targets []callTargetObservation, semanticKey string) bool {
	for _, target := range targets {
		if target.SemanticKey == semanticKey {
			return true
		}
	}
	return false
}

func (accumulator *directCallAccumulator) merge(source directCallAccumulator) {
	accumulator.raw += source.raw
	for _, key := range source.order {
		accumulator.addWithoutCounting(source.byKey[key])
	}
}

func (accumulator *directCallAccumulator) addWithoutCounting(value callsiteObservation) {
	raw := accumulator.raw
	accumulator.add(value)
	accumulator.raw = raw
}

func (accumulator directCallAccumulator) observations() []callsiteObservation {
	keys := append([]string(nil), accumulator.order...)
	sort.Strings(keys)
	result := make([]callsiteObservation, 0, len(keys))
	for _, key := range keys {
		result = append(result, accumulator.byKey[key])
	}
	sortCallsiteObservations(result)
	return result
}

func (accumulator directCallAccumulator) compactedCount() int {
	return len(accumulator.byKey)
}

type dataflowKey struct {
	source      string
	target      string
	access      string
	owner       string
	startLine   uint32
	startColumn uint32
	endLine     uint32
	endColumn   uint32
}

type dataflowAccumulator struct {
	byKey map[dataflowKey]dataflowObservation
	order []dataflowKey
	raw   int
}

func (accumulator *dataflowAccumulator) addObservations(values []dataflowObservation) {
	accumulator.raw += len(values)
	if accumulator.byKey == nil {
		accumulator.byKey = make(map[dataflowKey]dataflowObservation)
	}
	for _, value := range values {
		key := dataflowObservationKey(value)
		if _, exists := accumulator.byKey[key]; exists {
			continue
		}
		accumulator.byKey[key] = value
		accumulator.order = append(accumulator.order, key)
	}
}

func (accumulator *dataflowAccumulator) merge(source dataflowAccumulator) {
	accumulator.raw += source.raw
	if accumulator.byKey == nil {
		accumulator.byKey = make(map[dataflowKey]dataflowObservation)
	}
	for _, key := range source.order {
		if _, exists := accumulator.byKey[key]; exists {
			continue
		}
		accumulator.byKey[key] = source.byKey[key]
		accumulator.order = append(accumulator.order, key)
	}
}

func (accumulator dataflowAccumulator) observations() []observation {
	values := make([]dataflowObservation, 0, len(accumulator.order))
	for _, key := range accumulator.order {
		values = append(values, accumulator.byKey[key])
	}
	sortDataflowObservations(values)
	result := make([]observation, 0, len(values))
	for _, value := range values {
		result = append(result, value)
	}
	return result
}

func dataflowObservationKey(value dataflowObservation) dataflowKey {
	return dataflowKey{
		source:      value.SourceKey,
		target:      value.TargetKey,
		access:      value.Access,
		owner:       value.Owner,
		startLine:   value.Span.StartLine,
		startColumn: value.Span.StartColumn,
		endLine:     value.Span.EndLine,
		endColumn:   value.Span.EndColumn,
	}
}
