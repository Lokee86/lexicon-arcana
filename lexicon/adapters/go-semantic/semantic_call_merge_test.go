package main

import "testing"

func TestMergeCallsitesPrefersResolvedEvidence(t *testing.T) {
	location := span{StartLine: 7, StartColumn: 2, EndLine: 7, EndColumn: 12}
	unresolved := callsiteObservation{
		Observation:   "callsite",
		SourceKey:     "function:example.com/test:caller",
		Form:          "dynamic",
		Resolution:    "missing",
		Expression:    "info.Size",
		CandidateName: "Size",
		Owner:         "main.go",
		Span:          location,
	}
	resolved := resolvedCall(
		"function:example.com/test:caller",
		"method:io/fs:FileInfo.Size",
		"direct",
		"main.go",
		location,
	)

	merged := mergeCallsiteObservations(unresolved, resolved)
	if !hasResolvedCall(merged) || len(merged.Targets) != 1 {
		t.Fatalf("merged callsite = %#v", merged)
	}
	if merged.Targets[0].SemanticKey != "method:io/fs:FileInfo.Size" {
		t.Fatalf("merged target = %#v", merged.Targets)
	}
}

func TestMergeCallsitesCombinesTargetsWithoutCertaintyPolicy(t *testing.T) {
	location := span{StartLine: 8, StartColumn: 2, EndLine: 8, EndColumn: 9}
	first := resolvedCall(
		"function:example.com/test:caller",
		"function:example.com/test:first",
		"direct",
		"main.go",
		location,
	)
	second := resolvedCall(
		"function:example.com/test:caller",
		"function:example.com/test:second",
		"dynamic",
		"main.go",
		location,
	)
	merged := mergeCallsiteObservations(first, second)
	if merged.Form != "dynamic" || merged.Resolution != "resolved" {
		t.Fatalf("merged callsite = %#v", merged)
	}
	if len(merged.Targets) != 2 {
		t.Fatalf("merged targets = %d, want 2: %#v", len(merged.Targets), merged.Targets)
	}
}

func TestNormalizedCallsiteDeduplicatesTargetEvidence(t *testing.T) {
	value := callsiteObservation{
		Observation: "callsite",
		SourceKey:   "function:example.com/test:invoke",
		Form:        "interface",
		Resolution:  "resolved",
		Targets: []callTargetObservation{
			{SemanticKey: "method:example.com/test:Fast.Run"},
			{
				SemanticKey: "method:example.com/test:Fast.Run",
				Name:        "Run",
				Namespace:   "example.com/test",
			},
		},
		Owner: "main.go",
		Span:  span{StartLine: 10, StartColumn: 2, EndLine: 10, EndColumn: 9},
	}
	got := normalizedCallsite(value)
	if len(got.Targets) != 1 {
		t.Fatalf("targets = %d, want 1: %#v", len(got.Targets), got.Targets)
	}
	if got.Targets[0].Name != "Run" || got.Targets[0].Namespace != "example.com/test" {
		t.Fatalf("duplicate target evidence was not merged: %#v", got.Targets[0])
	}
}
