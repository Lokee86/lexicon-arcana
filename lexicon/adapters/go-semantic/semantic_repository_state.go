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
	hasIndex     bool
	owner        string
	hasSpan      bool
	startLine    uint32
	startColumn  uint32
	endLine      uint32
	endColumn    uint32
}

type semanticRepositoryState struct {
	directBeforeSSA directCallAccumulator
	dataflow        dataflowAccumulator
	finalCalls      directCallAccumulator
	relationships   map[relationshipKey]relationship
	targets         map[string]targetObservation
	captures        map[captureKey]relationship
}

func newSemanticRepositoryState() *semanticRepositoryState {
	return &semanticRepositoryState{
		relationships: make(map[relationshipKey]relationship),
		targets:       make(map[string]targetObservation),
		captures:      make(map[captureKey]relationship),
	}
}

func (state *semanticRepositoryState) addRelationships(values []relationship) {
	for _, value := range values {
		key := relationshipKey{
			source: value.Source,
			target: value.Target,
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

func (state *semanticRepositoryState) addSSARecords(values []semanticRecord) {
	for _, value := range values {
		switch record := value.(type) {
		case callObservation, unresolvedObservation:
			state.finalCalls.addWithoutCounting(record)
		case targetObservation:
			if _, exists := state.targets[record.Identity]; !exists {
				state.targets[record.Identity] = record
			}
		case relationship:
			key := captureRecordKey(record)
			if _, exists := state.captures[key]; !exists {
				state.captures[key] = record
			}
		default:
			panic("unexpected SSA semantic record")
		}
	}
}

func (state *semanticRepositoryState) relationshipRecords() []semanticRecord {
	values := make([]relationship, 0, len(state.relationships))
	for _, value := range state.relationships {
		values = append(values, value)
	}
	sortRelationships(values)
	result := make([]semanticRecord, 0, len(values))
	for _, value := range values {
		result = append(result, value)
	}
	return result
}

func (state *semanticRepositoryState) targetRecords() []semanticRecord {
	identities := make([]string, 0, len(state.targets))
	for identity := range state.targets {
		identities = append(identities, identity)
	}
	sort.Strings(identities)
	result := make([]semanticRecord, 0, len(identities))
	for _, identity := range identities {
		result = append(result, state.targets[identity])
	}
	return result
}

func (state *semanticRepositoryState) captureRecords() []semanticRecord {
	values := make([]relationship, 0, len(state.captures))
	for _, value := range state.captures {
		values = append(values, value)
	}
	sort.SliceStable(values, func(i, j int) bool {
		if values[i].Source != values[j].Source {
			return values[i].Source < values[j].Source
		}
		left := -1
		if values[i].CaptureIndex != nil {
			left = *values[i].CaptureIndex
		}
		right := -1
		if values[j].CaptureIndex != nil {
			right = *values[j].CaptureIndex
		}
		if left != right {
			return left < right
		}
		if values[i].Target != values[j].Target {
			return values[i].Target < values[j].Target
		}
		return values[i].TargetName < values[j].TargetName
	})
	result := make([]semanticRecord, 0, len(values))
	for _, value := range values {
		result = append(result, value)
	}
	return result
}

func captureRecordKey(record relationship) captureKey {
	key := captureKey{
		source:     record.Source,
		target:     record.Target,
		targetName: record.TargetName,
		owner:      record.Owner,
	}
	if record.CaptureIndex != nil {
		key.captureIndex = *record.CaptureIndex
		key.hasIndex = true
	}
	if record.Span != nil {
		key.hasSpan = true
		key.startLine = record.Span.StartLine
		key.startColumn = record.Span.StartColumn
		key.endLine = record.Span.EndLine
		key.endColumn = record.Span.EndColumn
	}
	return key
}
