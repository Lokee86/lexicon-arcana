package main

import "sort"

type directCallAccumulator struct {
	byKey map[string][]semanticRecord
	order []string
	raw   int
}

func (accumulator *directCallAccumulator) addRecords(records []semanticRecord) {
	accumulator.raw += len(records)
	for _, record := range records {
		accumulator.add(record)
	}
}

func (accumulator *directCallAccumulator) add(record semanticRecord) {
	if accumulator.byKey == nil {
		accumulator.byKey = make(map[string][]semanticRecord)
	}
	key := recordCallsiteKey(record)
	existing, found := accumulator.byKey[key]
	if !found {
		accumulator.order = append(accumulator.order, key)
		accumulator.byKey[key] = []semanticRecord{record}
		return
	}
	switch incoming := record.(type) {
	case callObservation:
		if _, unresolved := existing[0].(unresolvedObservation); unresolved {
			existing = existing[:0]
		}
		existing = appendResolvedObservation(existing, incoming)
		accumulator.byKey[key] = normalizeResolvedObservations(existing)
	case unresolvedObservation:
		if _, resolved := existing[0].(callObservation); resolved {
			return
		}
		// Preserve the first unresolved observation for a callsite.
	default:
		panic("unexpected direct call record")
	}
}

func (accumulator *directCallAccumulator) merge(source directCallAccumulator) {
	accumulator.raw += source.raw
	for _, key := range source.order {
		for _, record := range source.byKey[key] {
			accumulator.addWithoutCounting(record)
		}
	}
}

func (accumulator *directCallAccumulator) addWithoutCounting(record semanticRecord) {
	raw := accumulator.raw
	accumulator.add(record)
	accumulator.raw = raw
}

func (accumulator directCallAccumulator) records() []semanticRecord {
	keys := append([]string(nil), accumulator.order...)
	sort.Strings(keys)
	result := make([]semanticRecord, 0, accumulator.compactedCount())
	for _, key := range keys {
		result = append(result, accumulator.byKey[key]...)
	}
	sortSemanticCallRecords(result)
	return result
}

func (accumulator directCallAccumulator) compactedCount() int {
	count := 0
	for _, records := range accumulator.byKey {
		count += len(records)
	}
	return count
}

type dataflowKey struct {
	source      string
	target      string
	kind        string
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

func (accumulator *dataflowAccumulator) addRecords(records []semanticRecord) {
	accumulator.raw += len(records)
	if accumulator.byKey == nil {
		accumulator.byKey = make(map[dataflowKey]dataflowObservation)
	}
	for _, record := range records {
		observation, ok := record.(dataflowObservation)
		if !ok {
			panic("unexpected dataflow record")
		}
		key := dataflowRecordKey(observation)
		if _, exists := accumulator.byKey[key]; exists {
			continue
		}
		accumulator.byKey[key] = observation
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

func (accumulator dataflowAccumulator) records() []semanticRecord {
	result := make([]semanticRecord, 0, len(accumulator.order))
	for _, key := range accumulator.order {
		result = append(result, accumulator.byKey[key])
	}
	sortDataflowRecords(result)
	return result
}

func dataflowRecordKey(record dataflowObservation) dataflowKey {
	return dataflowKey{
		source:      record.Source,
		target:      record.Target,
		kind:        record.Kind,
		owner:       record.Owner,
		startLine:   record.Span.StartLine,
		startColumn: record.Span.StartColumn,
		endLine:     record.Span.EndLine,
		endColumn:   record.Span.EndColumn,
	}
}
