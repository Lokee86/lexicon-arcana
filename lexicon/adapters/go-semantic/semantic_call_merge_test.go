package main

import "testing"

func TestMergeDirectCallsPrefersResolvedPackageView(t *testing.T) {
	location := span{StartLine: 7, StartColumn: 2, EndLine: 7, EndColumn: 12}
	unresolved := unresolvedObservation{
		Record: "unresolved", Source: "function:example.com/test:caller",
		Relation: "calls", Expression: "info.Size", CandidateName: "Size",
		Reason: "dynamic-target", Class: "dynamic", Owner: "main.go", Span: location,
	}
	resolved := callRecord(
		"function:example.com/test:caller",
		"method:io/fs:FileInfo.Size",
		"definite", "external", "main.go", location,
	)

	merged := mergeDirectCallRecords([]semanticRecord{unresolved, resolved, unresolved})
	if len(merged) != 1 {
		t.Fatalf("merged records = %d, want 1: %#v", len(merged), merged)
	}
	call, ok := merged[0].(callObservation)
	if !ok || call.Target != "method:io/fs:FileInfo.Size" || call.Kind != "definite" {
		t.Fatalf("merged call = %#v", merged[0])
	}
}

func TestMergeDirectCallsPromotesMultipleTargetsToPossible(t *testing.T) {
	location := span{StartLine: 8, StartColumn: 2, EndLine: 8, EndColumn: 9}
	records := []semanticRecord{
		callRecord("function:example.com/test:caller", "function:example.com/test:first",
			"definite", "internal", "main.go", location),
		callRecord("function:example.com/test:caller", "function:example.com/test:second",
			"definite", "dynamic", "main.go", location),
	}
	merged := mergeDirectCallRecords(records)
	if len(merged) != 2 {
		t.Fatalf("merged records = %d, want 2", len(merged))
	}
	for _, record := range merged {
		call := record.(callObservation)
		if call.Kind != "possible" || call.Class != "dynamic" {
			t.Fatalf("merged call = %#v", call)
		}
	}
}
