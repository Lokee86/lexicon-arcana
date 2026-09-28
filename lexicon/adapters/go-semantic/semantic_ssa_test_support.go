package main

import (
	"strings"
	"testing"
)

func assertSemanticCall(
	t *testing.T,
	calls []callsiteObservation,
	source, target string,
) {
	t.Helper()
	if hasSemanticCallTarget(calls, source, target) {
		return
	}
	t.Fatalf("missing call %s -> %s", source, target)
}

func assertNoUnresolvedCalls(t *testing.T, calls []callsiteObservation) {
	t.Helper()
	for _, call := range calls {
		if !hasResolvedCall(call) {
			t.Fatalf("unexpected unresolved call: %#v", call)
		}
	}
}

func hasSemanticCallTarget(calls []callsiteObservation, source, target string) bool {
	for _, call := range calls {
		if call.SourceKey != source {
			continue
		}
		for _, candidate := range call.Targets {
			if candidate.SemanticKey == target {
				return true
			}
		}
	}
	return false
}

func hasCallPrefix(calls []callsiteObservation, source, prefix string) bool {
	for _, call := range calls {
		if call.SourceKey != source {
			continue
		}
		for _, candidate := range call.Targets {
			if strings.HasPrefix(candidate.SemanticKey, prefix) {
				return true
			}
		}
	}
	return false
}

func callRecords(values []observation) []callsiteObservation {
	var result []callsiteObservation
	for _, value := range values {
		if call, ok := value.(callsiteObservation); ok {
			result = append(result, call)
		}
	}
	return result
}
