package main

import (
	"path/filepath"
	"strings"
	"testing"
)

func TestSSAPackageLevelFunctionLiteralUsesSyntheticTarget(t *testing.T) {
	root := t.TempDir()
	writeSemanticFile(t, root, "go.mod", "module example.com/package-literal\n\ngo 1.22\n")
	writeSemanticFile(t, root, "main.go", `package literal

var callback = func() {}

func caller() {
	callback()
}
`)
	absolute, err := filepath.Abs(root)
	if err != nil {
		t.Fatal(err)
	}
	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  absolute,
		Files:           []string{"go.mod", "main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/package-literal"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}
	for _, record := range result.records {
		call, ok := record.(callObservation)
		if !ok || call.Source != "function:example.com/package-literal:caller" {
			continue
		}
		if strings.HasPrefix(call.Target, "closure:") {
			t.Fatalf("package-level literal used structural closure target: %#v", call)
		}
		if strings.HasPrefix(call.Target, "ssa-function:example.com/package-literal:") {
			return
		}
	}
	t.Fatalf("synthetic SSA target not found: %#v", callRecords(result.records))
}
