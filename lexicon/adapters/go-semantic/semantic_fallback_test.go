package main

import (
	"path/filepath"
	"testing"
)

func TestSyntaxFallbackPreservesInactiveBuildVariantCalls(t *testing.T) {
	root, err := filepath.Abs(filepath.Join("..", "go", "testdata", "oracle", "build_tags"))
	if err != nil {
		t.Fatal(err)
	}
	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files: []string{
			"enabled_default.go",
			"enabled_special.go",
			"go.mod",
			"special_test.go",
		},
		Modules:   []module{{Root: ".", Path: "example.com/oracle/tagged"}},
		Execution: execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}

	source := "test:example.com/oracle/tagged:TestTagged"
	want := map[string]bool{
		"function:example.com/oracle/tagged:Enabled": true,
		"function:go:builtins:append":                true,
		"function:os:Executable":                     true,
		"function:os/exec:Command":                   true,
		"dynamic-method:t.Fatal":                     true,
		"dynamic-method:cmd.Start":                   true,
	}
	for _, record := range result.Records {
		call, ok := record.(callObservation)
		if ok && call.Source == source {
			delete(want, call.Target)
		}
	}
	if len(want) != 0 {
		t.Fatalf("missing inactive build-variant fallback calls: %#v", want)
	}
}
