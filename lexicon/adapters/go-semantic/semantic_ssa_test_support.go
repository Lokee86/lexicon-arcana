package main

import "testing"

func assertSemanticCall(
	t *testing.T,
	records []semanticRecord,
	source, target, kind string,
) {
	t.Helper()
	for _, record := range records {
		value, ok := record.(callObservation)
		if ok && value.Source == source && value.Target == target && value.Kind == kind {
			return
		}
	}
	t.Fatalf("missing %s call %s -> %s", kind, source, target)
}

func assertNoUnresolvedCalls(t *testing.T, records []semanticRecord) {
	t.Helper()
	for _, record := range records {
		if value, ok := record.(unresolvedObservation); ok && value.Relation == "calls" {
			t.Fatalf("unexpected unresolved call: %#v", value)
		}
	}
}

func hasCallTarget(records []semanticRecord, source, target string) bool {
	for _, record := range records {
		value, ok := record.(callObservation)
		if ok && value.Source == source && value.Target == target {
			return true
		}
	}
	return false
}

func hasCallPrefix(records []semanticRecord, source, prefix string) bool {
	for _, record := range records {
		value, ok := record.(callObservation)
		if ok && value.Source == source && len(value.Target) >= len(prefix) &&
			value.Target[:len(prefix)] == prefix {
			return true
		}
	}
	return false
}

func callRecords(records []semanticRecord) []callObservation {
	var result []callObservation
	for _, record := range records {
		if value, ok := record.(callObservation); ok {
			result = append(result, value)
		}
	}
	return result
}
