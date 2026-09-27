package main

import (
	"path/filepath"
	"testing"
)

func TestSSACapturesFreeVariables(t *testing.T) {
	root, err := filepath.Abs(filepath.Join("..", "go", "testdata", "oracle", "higher_order"))
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
	var captures []relationship
	for _, record := range result.Records {
		value, ok := record.(relationship)
		if ok && value.Kind == "references" {
			captures = append(captures, value)
		}
	}
	if len(captures) != 1 {
		t.Fatalf("captures = %d, want 1: %#v", len(captures), captures)
	}
	capture := captures[0]
	if capture.Source != "closure:example.com/oracle/higher:main.go:22:13" ||
		capture.Target != "variable:example.com/oracle/higher:main.go:21:2:captured" ||
		capture.TargetName != "captured" || capture.CaptureIndex == nil ||
		*capture.CaptureIndex != 0 || capture.Span == nil ||
		capture.Span.StartLine != 21 || capture.Span.StartColumn != 2 {
		t.Fatalf("capture = %#v", capture)
	}
}

func TestSSAResolvesHigherOrderAndClosureCalls(t *testing.T) {
	root, err := filepath.Abs(filepath.Join("..", "go", "testdata", "oracle", "higher_order"))
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
	assertSemanticCall(t, result.Records,
		"function:example.com/oracle/higher:apply",
		"function:example.com/oracle/higher:first", "possible")
	assertSemanticCall(t, result.Records,
		"function:example.com/oracle/higher:apply",
		"function:example.com/oracle/higher:second", "possible")
	assertSemanticCall(t, result.Records,
		"function:example.com/oracle/higher:caller",
		"function:example.com/oracle/higher:target", "definite")

	firstClosure := "closure:example.com/oracle/higher:main.go:22:13"
	secondClosure := "closure:example.com/oracle/higher:main.go:27:2"
	assertSemanticCall(t, result.Records,
		"function:example.com/oracle/higher:caller", firstClosure, "definite")
	assertSemanticCall(t, result.Records,
		"function:example.com/oracle/higher:caller", secondClosure, "definite")
	assertSemanticCall(t, result.Records,
		firstClosure, "function:example.com/oracle/higher:target", "definite")
	assertSemanticCall(t, result.Records,
		secondClosure, "function:example.com/oracle/higher:target", "definite")
	assertNoUnresolvedCalls(t, result.Records)
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
	assertSemanticCall(t, result.Records,
		"function:example.com/flow:caller",
		"function:example.com/flow:target", "definite")
	if !hasCallTarget(result.Records,
		"function:example.com/flow:caller",
		"method:example.com/flow:Worker.Run",
	) && !hasCallPrefix(result.Records,
		"function:example.com/flow:caller",
		"ssa-function:example.com/flow:",
	) {
		t.Fatalf("method value was not resolved: %#v", callRecords(result.Records))
	}
	assertNoUnresolvedCalls(t, result.Records)
}

func TestSSAResolvesInterfaceInvokeToConcreteMethods(t *testing.T) {
	root, err := filepath.Abs(filepath.Join("..", "go", "testdata", "oracle", "relationships"))
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
	source := "function:example.com/oracle/relationships:invoke"
	assertSemanticCall(t, result.Records, source,
		"method:example.com/oracle/relationships:Base.Run", "possible")
	assertSemanticCall(t, result.Records, source,
		"method:example.com/oracle/relationships:Direct.Run", "possible")
	assertNoUnresolvedCalls(t, result.Records)
}

func TestSSAMergeDropsInterfaceContractWhenConcreteTargetsExist(t *testing.T) {
	location := span{StartLine: 10, StartColumn: 2, EndLine: 10, EndColumn: 9}
	direct := []semanticRecord{callObservation{
		Record: "call",
		Source: "function:example.com/test:invoke",
		Target: "interface-method:example.com/test:Runner.Run",
		Kind:   "definite",
		Class:  "interface",
		Owner:  "main.go",
		Span:   location,
	}}
	key := recordCallsiteKey(direct[0])
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
	if hasCallTarget(merged,
		"function:example.com/test:invoke",
		"interface-method:example.com/test:Runner.Run",
	) {
		t.Fatalf("interface contract survived concrete merge: %#v", callRecords(merged))
	}
	assertSemanticCall(t, merged,
		"function:example.com/test:invoke",
		"method:example.com/test:First.Run", "possible")
	assertSemanticCall(t, merged,
		"function:example.com/test:invoke",
		"method:example.com/test:Second.Run", "possible")
}

func TestSSAMergeDropsContractTargetReturnedByVTAWhenConcreteTargetExists(t *testing.T) {
	location := span{StartLine: 10, StartColumn: 2, EndLine: 10, EndColumn: 9}
	direct := []semanticRecord{callObservation{
		Record: "call",
		Source: "function:example.com/test:invoke",
		Target: "method:example.com/test:Concrete.Run",
		Kind:   "definite",
		Class:  "interface",
		Owner:  "main.go",
		Span:   location,
	}}
	key := recordCallsiteKey(direct[0])
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
	if hasCallTarget(merged,
		"function:example.com/test:invoke",
		"interface-method:example.com/test:Runner.Run",
	) {
		t.Fatalf("interface contract survived concrete merge: %#v", callRecords(merged))
	}
	assertSemanticCall(t, merged,
		"function:example.com/test:invoke",
		"method:example.com/test:Concrete.Run", "definite")
}

func TestSSAMergePreservesEarlierModuleResolutionForDynamicCall(t *testing.T) {
	location := span{StartLine: 10, StartColumn: 2, EndLine: 10, EndColumn: 9}
	direct := []semanticRecord{unresolvedObservation{
		Record:   "unresolved",
		Source:   "function:example.com/test:caller",
		Relation: "calls",
		Reason:   "dynamic-target",
		Class:    "dynamic",
		Owner:    "main.go",
		Span:     location,
	}}
	key := recordCallsiteKey(direct[0])
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
		t.Fatalf("records = %d, want 1: %#v", len(merged), merged)
	}
	if _, ok := merged[0].(unresolvedObservation); !ok {
		t.Fatalf("later-module SSA replaced an earlier resolution: %#v", merged)
	}
}
