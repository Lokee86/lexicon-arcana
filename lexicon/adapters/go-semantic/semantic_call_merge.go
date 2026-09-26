package main

import "sort"

func mergeDirectCallRecords(records []semanticRecord) []semanticRecord {
	byKey := make(map[string][]semanticRecord)
	var keys []string
	for _, record := range records {
		key := recordCallsiteKey(record)
		existing, found := byKey[key]
		if !found {
			keys = append(keys, key)
			byKey[key] = []semanticRecord{record}
			continue
		}
		switch incoming := record.(type) {
		case callObservation:
			if _, unresolved := existing[0].(unresolvedObservation); unresolved {
				existing = existing[:0]
			}
			existing = appendResolvedObservation(existing, incoming)
			byKey[key] = normalizeResolvedObservations(existing)
		case unresolvedObservation:
			if _, resolved := existing[0].(callObservation); resolved {
				continue
			}
			// Legacy mergeSemanticCall preserves the first unresolved reason.
		default:
			panic("unexpected direct call record")
		}
	}
	sort.Strings(keys)
	result := make([]semanticRecord, 0, len(records))
	for _, key := range keys {
		result = append(result, byKey[key]...)
	}
	sortSemanticCallRecords(result)
	return result
}

func appendResolvedObservation(
	records []semanticRecord,
	incoming callObservation,
) []semanticRecord {
	for _, record := range records {
		existing, ok := record.(callObservation)
		if !ok {
			continue
		}
		if existing.Target == incoming.Target && existing.Kind == incoming.Kind {
			return records
		}
	}
	return append(records, incoming)
}

func normalizeResolvedObservations(records []semanticRecord) []semanticRecord {
	targets := make(map[string]bool)
	class := ""
	for _, record := range records {
		call, ok := record.(callObservation)
		if !ok {
			continue
		}
		if call.Kind == "definite" || call.Kind == "possible" {
			targets[call.Target] = true
		}
		if callClassPriority(call.Class) > callClassPriority(class) {
			class = call.Class
		}
	}
	kind := "definite"
	if len(targets) > 1 {
		kind = "possible"
	}
	for index, record := range records {
		call, ok := record.(callObservation)
		if !ok {
			continue
		}
		if call.Kind == "definite" || call.Kind == "possible" {
			call.Kind = kind
		}
		if class != "" {
			call.Class = class
		}
		records[index] = call
	}
	sort.SliceStable(records, func(left, right int) bool {
		leftCall := records[left].(callObservation)
		rightCall := records[right].(callObservation)
		if leftCall.Kind != rightCall.Kind {
			return leftCall.Kind < rightCall.Kind
		}
		return leftCall.Target < rightCall.Target
	})
	return records
}

func callClassPriority(class string) int {
	switch class {
	case "interface":
		return 6
	case "dynamic":
		return 5
	case "conversion":
		return 4
	case "builtin":
		return 3
	case "external":
		return 2
	case "internal":
		return 1
	default:
		return 0
	}
}
