package main

import (
	"path/filepath"
	"testing"
)

func TestSemanticDataflowPreservesLegacyCompoundAndShadowing(t *testing.T) {
	root, err := filepath.Abs(filepath.Join("..", "..", "testdata", "go_oracle", "repositories", "dataflow"))
	if err != nil {
		t.Fatal(err)
	}
	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           []string{"go.mod", "main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/oracle/dataflow"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}

	local := "variable:example.com/oracle/dataflow:main.go:14:2:local"
	outer := "variable:example.com/oracle/dataflow:main.go:13:10:value"
	shadow := "variable:example.com/oracle/dataflow:main.go:19:3:value"
	field := "field:example.com/oracle/dataflow:main.go:4:2:Field"
	constant := "constant:example.com/oracle/dataflow:main.go:7:7:Constant"

	if !hasDataflow(result.Observations, local, "write", 15, 2) {
		t.Fatal("missing compound local write")
	}
	if hasDataflow(result.Observations, local, "read", 15, 2) {
		t.Fatal("compound access widened beyond the legacy oracle")
	}
	for _, target := range []string{outer, shadow, field, constant} {
		if !hasDataflowTarget(result.Observations, target) {
			t.Fatalf("missing typed dataflow target %q", target)
		}
	}
}

func hasDataflow(values []observation, target, access string, line, column uint32) bool {
	for _, value := range values {
		item, ok := value.(dataflowObservation)
		if ok && item.TargetKey == target && item.Access == access &&
			item.Span.StartLine == line && item.Span.StartColumn == column {
			return true
		}
	}
	return false
}

func hasDataflowTarget(values []observation, target string) bool {
	for _, value := range values {
		item, ok := value.(dataflowObservation)
		if ok && item.TargetKey == target {
			return true
		}
	}
	return false
}
