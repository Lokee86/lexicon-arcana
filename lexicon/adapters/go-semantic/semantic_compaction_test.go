package main

import (
	"reflect"
	"testing"
)

func TestDirectCallShardMergeMatchesGlobalCompaction(t *testing.T) {
	location := span{StartLine: 9, StartColumn: 3, EndLine: 9, EndColumn: 14}
	unresolved := unresolvedObservation{
		Record: "unresolved", Source: "function:example.com/test:caller",
		Relation: "calls", Expression: "service.Run", CandidateName: "Run",
		Reason: "dynamic-target", Class: "dynamic", Owner: "main.go", Span: location,
	}
	first := callRecord(
		"function:example.com/test:caller",
		"method:example.com/test:First.Run",
		"definite", "internal", "main.go", location,
	)
	second := callRecord(
		"function:example.com/test:caller",
		"method:example.com/test:Second.Run",
		"definite", "dynamic", "main.go", location,
	)
	leftRaw := []semanticRecord{unresolved, first, first}
	rightRaw := []semanticRecord{unresolved, second}

	var left directCallAccumulator
	left.addRecords(leftRaw)
	var right directCallAccumulator
	right.addRecords(rightRaw)
	left.merge(right)

	want := mergeDirectCallRecords(append(append([]semanticRecord(nil), leftRaw...), rightRaw...))
	got := left.records()
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("shard compaction changed call semantics\ngot:  %#v\nwant: %#v", got, want)
	}
	if left.raw != len(leftRaw)+len(rightRaw) {
		t.Fatalf("raw calls = %d, want %d", left.raw, len(leftRaw)+len(rightRaw))
	}
}

func TestDataflowShardMergeDeduplicatesExactRecords(t *testing.T) {
	first := dataflowObservation{
		Record: "dataflow",
		Source: "function:example.com/test:caller",
		Target: "variable:example.com/test:main.go:3:5:value",
		Kind:   "read",
		Owner:  "main.go",
		Span:   span{StartLine: 9, StartColumn: 3, EndLine: 9, EndColumn: 8},
	}
	second := dataflowObservation{
		Record: "dataflow",
		Source: first.Source,
		Target: first.Target,
		Kind:   "write",
		Owner:  first.Owner,
		Span:   span{StartLine: 10, StartColumn: 3, EndLine: 10, EndColumn: 8},
	}

	var left dataflowAccumulator
	left.addRecords([]semanticRecord{first, first})
	var right dataflowAccumulator
	right.addRecords([]semanticRecord{first, second})
	left.merge(right)

	got := left.records()
	if left.raw != 4 {
		t.Fatalf("raw dataflow = %d, want 4", left.raw)
	}
	if len(got) != 2 {
		t.Fatalf("compacted dataflow = %d, want 2: %#v", len(got), got)
	}
	if got[0] != first || got[1] != second {
		t.Fatalf("unexpected compacted ordering: %#v", got)
	}
}
