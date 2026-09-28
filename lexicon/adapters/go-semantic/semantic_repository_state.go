package main

import "sort"

type relationshipKey struct {
	source string
	target string
	kind   string
}

type captureKey struct {
	source       string
	target       string
	targetName   string
	captureIndex int
	owner        string
	hasSpan      bool
	startLine    uint32
	startColumn  uint32
	endLine      uint32
	endColumn    uint32
}

type semanticRepositoryState struct {
	directBeforeSSA      directCallAccumulator
	dataflow             dataflowAccumulator
	finalCalls           directCallAccumulator
	resolvedCallsiteKeys map[string]bool
	relationships        map[relationshipKey]relationshipObservation
	symbols              map[string]symbolObservation
	captures             map[captureKey]captureObservation
}

func newSemanticRepositoryState() *semanticRepositoryState {
	return &semanticRepositoryState{
		resolvedCallsiteKeys: make(map[string]bool),
		relationships:        make(map[relationshipKey]relationshipObservation),
		symbols:              make(map[string]symbolObservation),
		captures:             make(map[captureKey]captureObservation),
	}
}

func (state *semanticRepositoryState) addRelationships(values []relationshipObservation) {
	for _, value := range values {
		key := relationshipKey{
			source: value.SourceKey,
			target: value.TargetKey,
			kind:   value.Kind,
		}
		if _, exists := state.relationships[key]; !exists {
			state.relationships[key] = value
		}
	}
}

func (state *semanticRepositoryState) addCollection(collection semanticCollection) {
	state.directBeforeSSA.merge(collection.calls)
	state.dataflow.merge(collection.dataflow)
}

func (state *semanticRepositoryState) resolvedCallsites() map[string]bool {
	return state.resolvedCallsiteKeys
}

func (state *semanticRepositoryState) addSSAObservations(values []observation) {
	for _, value := range values {
		switch item := value.(type) {
		case callsiteObservation:
			state.finalCalls.addWithoutCounting(item)
			if hasResolvedCall(item) {
				state.resolvedCallsiteKeys[callsiteObservationKey(item)] = true
			}
		case symbolObservation:
			if _, exists := state.symbols[item.SemanticKey]; !exists {
				state.symbols[item.SemanticKey] = item
			}
		case captureObservation:
			key := captureObservationKey(item)
			if _, exists := state.captures[key]; !exists {
				state.captures[key] = item
			}
		default:
			panic("unexpected SSA semantic observation")
		}
	}
}

func (state *semanticRepositoryState) relationshipObservations() []observation {
	values := make([]relationshipObservation, 0, len(state.relationships))
	for _, value := range state.relationships {
		values = append(values, value)
	}
	sortRelationships(values)
	result := make([]observation, 0, len(values))
	for _, value := range values {
		result = append(result, value)
	}
	return result
}

func (state *semanticRepositoryState) symbolObservations() []observation {
	keys := make([]string, 0, len(state.symbols))
	for key := range state.symbols {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	result := make([]observation, 0, len(keys))
	for _, key := range keys {
		result = append(result, state.symbols[key])
	}
	return result
}

func (state *semanticRepositoryState) captureObservations() []observation {
	values := make([]captureObservation, 0, len(state.captures))
	for _, value := range state.captures {
		values = append(values, value)
	}
	sort.SliceStable(values, func(i, j int) bool {
		if values[i].SourceKey != values[j].SourceKey {
			return values[i].SourceKey < values[j].SourceKey
		}
		if values[i].CaptureIndex != values[j].CaptureIndex {
			return values[i].CaptureIndex < values[j].CaptureIndex
		}
		if values[i].TargetKey != values[j].TargetKey {
			return values[i].TargetKey < values[j].TargetKey
		}
		return values[i].TargetName < values[j].TargetName
	})
	result := make([]observation, 0, len(values))
	for _, value := range values {
		result = append(result, value)
	}
	return result
}

func captureObservationKey(value captureObservation) captureKey {
	key := captureKey{
		source:       value.SourceKey,
		target:       value.TargetKey,
		targetName:   value.TargetName,
		captureIndex: value.CaptureIndex,
		owner:        value.Owner,
	}
	if value.Span != nil {
		key.hasSpan = true
		key.startLine = value.Span.StartLine
		key.startColumn = value.Span.StartColumn
		key.endLine = value.Span.EndLine
		key.endColumn = value.Span.EndColumn
	}
	return key
}
