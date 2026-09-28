package main

import (
	"path/filepath"
	"testing"
)

func TestSSACapturesFreeVariables(t *testing.T) {
	root, err := filepath.Abs(filepath.Join("..", "..", "testdata", "go_oracle", "repositories", "higher_order"))
	if err != nil {
		t.Fatal(err)
	}
	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           []string{"go.mod", "main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/oracle/higher"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}
	var captures []captureObservation
	for _, value := range result.Observations {
		if capture, ok := value.(captureObservation); ok {
			captures = append(captures, capture)
		}
	}
	if len(captures) != 1 {
		t.Fatalf("captures = %d, want 1: %#v", len(captures), captures)
	}
	capture := captures[0]
	if capture.SourceKey != "closure:example.com/oracle/higher:main.go:22:13" ||
		capture.TargetKey != "variable:example.com/oracle/higher:main.go:21:2:captured" ||
		capture.TargetName != "captured" || capture.CaptureIndex != 0 || capture.Span == nil ||
		capture.Span.StartLine != 21 || capture.Span.StartColumn != 2 {
		t.Fatalf("capture = %#v", capture)
	}
}

func TestSSAResolvesHigherOrderAndClosureCalls(t *testing.T) {
	root, err := filepath.Abs(filepath.Join("..", "..", "testdata", "go_oracle", "repositories", "higher_order"))
	if err != nil {
		t.Fatal(err)
	}
	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           []string{"go.mod", "main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/oracle/higher"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}
	calls := callRecords(result.Observations)
	assertSemanticCall(t, calls,
		"function:example.com/oracle/higher:apply",
		"function:example.com/oracle/higher:first")
	assertSemanticCall(t, calls,
		"function:example.com/oracle/higher:apply",
		"function:example.com/oracle/higher:second")
	assertSemanticCall(t, calls,
		"function:example.com/oracle/higher:caller",
		"function:example.com/oracle/higher:target")

	firstClosure := "closure:example.com/oracle/higher:main.go:22:13"
	secondClosure := "closure:example.com/oracle/higher:main.go:27:2"
	assertSemanticCall(t, calls,
		"function:example.com/oracle/higher:caller", firstClosure)
	assertSemanticCall(t, calls,
		"function:example.com/oracle/higher:caller", secondClosure)
	assertSemanticCall(t, calls,
		firstClosure, "function:example.com/oracle/higher:target")
	assertSemanticCall(t, calls,
		secondClosure, "function:example.com/oracle/higher:target")
	assertNoUnresolvedCalls(t, calls)
}

func TestSSAResolvesReturnedFunctionsAndMethodValues(t *testing.T) {
	root := t.TempDir()
	writeSemanticFile(t, root, "go.mod", "module example.com/flow\n\ngo 1.22\n")
	writeSemanticFile(t, root, "main.go", `package flow

type Worker struct{}
func (Worker) Run() {}
func target() {}
func choose() func() { return target }

func caller() {
	f := choose()
	f()
	worker := Worker{}
	method := worker.Run
	method()
}
`)
	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           []string{"go.mod", "main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/flow"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}
	calls := callRecords(result.Observations)
	assertSemanticCall(t, calls,
		"function:example.com/flow:caller",
		"function:example.com/flow:target")
	if !hasSemanticCallTarget(calls,
		"function:example.com/flow:caller",
		"method:example.com/flow:Worker.Run",
	) && !hasCallPrefix(calls,
		"function:example.com/flow:caller",
		"ssa-function:example.com/flow:",
	) {
		t.Fatalf("method value was not resolved: %#v", calls)
	}
	assertNoUnresolvedCalls(t, calls)
}

func TestSSAResolvesInterfaceInvokeToConcreteMethods(t *testing.T) {
	root, err := filepath.Abs(filepath.Join("..", "..", "testdata", "go_oracle", "repositories", "relationships"))
	if err != nil {
		t.Fatal(err)
	}
	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           []string{"go.mod", "main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/oracle/relationships"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}
	calls := callRecords(result.Observations)
	source := "function:example.com/oracle/relationships:invoke"
	assertSemanticCall(t, calls, source,
		"method:example.com/oracle/relationships:Base.Run")
	assertSemanticCall(t, calls, source,
		"method:example.com/oracle/relationships:Direct.Run")
	assertNoUnresolvedCalls(t, calls)
}

func TestSSAMergeDropsInterfaceContractWhenConcreteTargetsExist(t *testing.T) {
	location := span{StartLine: 10, StartColumn: 2, EndLine: 10, EndColumn: 9}
	direct := []callsiteObservation{resolvedCall(
		"function:example.com/test:invoke",
		"interface-method:example.com/test:Runner.Run",
		"interface",
		"main.go",
		location,
	)}
	key := callsiteObservationKey(direct[0])
	outcomes := map[string]*ssaOutcome{
		key: {
			Invoke: true,
			Targets: map[string]ssaTarget{
				"method:example.com/test:First.Run": {
					Identity: "method:example.com/test:First.Run",
					Internal: true,
				},
				"method:example.com/test:Second.Run": {
					Identity: "method:example.com/test:Second.Run",
					Internal: true,
				},
			},
		},
	}
	merged := mergeSSAOutcomes(direct, outcomes, nil)
	if hasSemanticCallTarget(merged,
		"function:example.com/test:invoke",
		"interface-method:example.com/test:Runner.Run",
	) {
		t.Fatalf("interface contract survived concrete merge: %#v", merged)
	}
	assertSemanticCall(t, merged,
		"function:example.com/test:invoke",
		"method:example.com/test:First.Run")
	assertSemanticCall(t, merged,
		"function:example.com/test:invoke",
		"method:example.com/test:Second.Run")
}

func TestSSAMergeDropsContractTargetReturnedByVTAWhenConcreteTargetExists(t *testing.T) {
	location := span{StartLine: 10, StartColumn: 2, EndLine: 10, EndColumn: 9}
	direct := []callsiteObservation{resolvedCall(
		"function:example.com/test:invoke",
		"method:example.com/test:Concrete.Run",
		"interface",
		"main.go",
		location,
	)}
	key := callsiteObservationKey(direct[0])
	outcomes := map[string]*ssaOutcome{
		key: {
			Invoke: true,
			Targets: map[string]ssaTarget{
				"interface-method:example.com/test:Runner.Run": {
					Identity: "interface-method:example.com/test:Runner.Run",
					Internal: true,
				},
				"method:example.com/test:Concrete.Run": {
					Identity: "method:example.com/test:Concrete.Run",
					Internal: true,
				},
			},
		},
	}
	merged := mergeSSAOutcomes(direct, outcomes, nil)
	if hasSemanticCallTarget(merged,
		"function:example.com/test:invoke",
		"interface-method:example.com/test:Runner.Run",
	) {
		t.Fatalf("interface contract survived concrete merge: %#v", merged)
	}
	assertSemanticCall(t, merged,
		"function:example.com/test:invoke",
		"method:example.com/test:Concrete.Run")
}

func TestSSAMergePreservesEarlierModuleResolutionForDynamicCall(t *testing.T) {
	location := span{StartLine: 10, StartColumn: 2, EndLine: 10, EndColumn: 9}
	direct := []callsiteObservation{{
		Observation: "callsite",
		SourceKey:   "function:example.com/test:caller",
		Form:        "dynamic",
		Resolution:  "missing",
		Owner:       "main.go",
		Span:        location,
	}}
	key := callsiteObservationKey(direct[0])
	outcomes := map[string]*ssaOutcome{
		key: {
			Targets: map[string]ssaTarget{
				"function:example.com/test:override": {
					Identity: "function:example.com/test:override",
					Internal: true,
				},
			},
		},
	}
	merged := mergeSSAOutcomes(direct, outcomes, map[string]bool{key: true})
	if len(merged) != 1 {
		t.Fatalf("observations = %d, want 1: %#v", len(merged), merged)
	}
	if hasResolvedCall(merged[0]) {
		t.Fatalf("later-module SSA replaced an earlier resolution: %#v", merged)
	}
}
