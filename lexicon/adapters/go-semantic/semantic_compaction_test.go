package main

import (
	"reflect"
	"testing"
)

func TestDirectCallShardMergeMatchesGlobalObservationMerge(t *testing.T) {
	location := span{StartLine: 9, StartColumn: 3, EndLine: 9, EndColumn: 14}
	unresolved := callsiteObservation{
		Observation:   "callsite",
		SourceKey:     "function:example.com/test:caller",
		Form:          "dynamic",
		Resolution:    "missing",
		Expression:    "service.Run",
		CandidateName: "Run",
		Owner:         "main.go",
		Span:          location,
	}
	first := resolvedCall(
		"function:example.com/test:caller",
		"method:example.com/test:First.Run",
		"direct",
		"main.go",
		location,
	)
	second := resolvedCall(
		"function:example.com/test:caller",
		"method:example.com/test:Second.Run",
		"dynamic",
		"main.go",
		location,
	)
	leftRaw := []callsiteObservation{unresolved, first, first}
	rightRaw := []callsiteObservation{unresolved, second}

	var left directCallAccumulator
	left.addObservations(leftRaw)
	var right directCallAccumulator
	right.addObservations(rightRaw)
	left.merge(right)

	want := mergeCallsiteObservations(first, second)
	got := left.observations()
	if len(got) != 1 || !reflect.DeepEqual(got[0], want) {
		t.Fatalf("shard compaction changed call semantics\ngot:  %#v\nwant: %#v", got, want)
	}
	if left.raw != len(leftRaw)+len(rightRaw) {
		t.Fatalf("raw calls = %d, want %d", left.raw, len(leftRaw)+len(rightRaw))
	}
}

func TestDataflowShardMergeDeduplicatesExactObservations(t *testing.T) {
	first := dataflowObservation{
		Observation: "dataflow",
		SourceKey:   "function:example.com/test:caller",
		TargetKey:   "variable:example.com/test:main.go:3:5:value",
		Access:      "read",
		Owner:       "main.go",
		Span:        span{StartLine: 9, StartColumn: 3, EndLine: 9, EndColumn: 8},
	}
	second := dataflowObservation{
		Observation: "dataflow",
		SourceKey:   first.SourceKey,
		TargetKey:   first.TargetKey,
		Access:      "write",
		Owner:       first.Owner,
		Span:        span{StartLine: 10, StartColumn: 3, EndLine: 10, EndColumn: 8},
	}

	var left dataflowAccumulator
	left.addObservations([]dataflowObservation{first, first})
	var right dataflowAccumulator
	right.addObservations([]dataflowObservation{first, second})
	left.merge(right)

	gotRaw := left.observations()
	if left.raw != 4 {
		t.Fatalf("raw dataflow = %d, want 4", left.raw)
	}
	if len(gotRaw) != 2 {
		t.Fatalf("compacted dataflow = %d, want 2: %#v", len(gotRaw), gotRaw)
	}
	got := []dataflowObservation{gotRaw[0].(dataflowObservation), gotRaw[1].(dataflowObservation)}
	if got[0] != first || got[1] != second {
		t.Fatalf("unexpected compacted ordering: %#v", got)
	}
}
