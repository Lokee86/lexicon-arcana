package main

import "testing"

func TestPredeclaredErrorMethodUsesGoUnknownNamespace(t *testing.T) {
	root := t.TempDir()
	writeSemanticFile(t, root, "go.mod", "module example.com/error-interface\n\ngo 1.22\n")
	writeSemanticFile(t, root, "main.go", `package sample

func message(err error) string {
	return err.Error()
}
`)
	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           []string{"go.mod", "main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/error-interface"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}
	for _, record := range result.records {
		call, ok := record.(callObservation)
		if !ok || call.Source != "function:example.com/error-interface:message" {
			continue
		}
		if call.Target == "method:go:unknown:error.Error" && call.Class == "interface" {
			return
		}
	}
	t.Fatalf("go:unknown error method not resolved: %#v", callRecords(result.records))
}
