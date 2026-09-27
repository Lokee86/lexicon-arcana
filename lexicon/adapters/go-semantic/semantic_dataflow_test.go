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

	if !hasDataflow(result.Records, local, "write", 15, 2) {
		t.Fatal("missing compound local write")
	}
	if hasDataflow(result.Records, local, "read", 15, 2) {
		t.Fatal("compound access widened beyond the legacy oracle")
	}
	for _, target := range []string{outer, shadow, field, constant} {
		if !hasDataflowTarget(result.Records, target) {
			t.Fatalf("missing typed dataflow target %q", target)
		}
	}
}

func hasDataflow(records []semanticRecord, target, kind string, line, column uint32) bool {
	for _, value := range records {
		record, ok := value.(dataflowObservation)
		if ok && record.Target == target && record.Kind == kind &&
			record.Span.StartLine == line && record.Span.StartColumn == column {
			return true
		}
	}
	return false
}

func hasDataflowTarget(records []semanticRecord, target string) bool {
	for _, value := range records {
		record, ok := value.(dataflowObservation)
		if ok && record.Target == target {
			return true
		}
	}
	return false
}
