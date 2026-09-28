package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestSSAMaterializesGeneratedTestMainSource(t *testing.T) {
	root := t.TempDir()
	writeSemanticFile(t, root, "go.mod", "module example.com/test-main\n\ngo 1.22\n")
	pkg := filepath.Join(root, "internal", "sample")
	if err := os.MkdirAll(pkg, 0o755); err != nil {
		t.Fatal(err)
	}
	writeSemanticFile(t, pkg, "main.go", `package sample

func Value() int { return 1 }
`)
	writeSemanticFile(t, pkg, "main_test.go", `package sample

import "testing"

func TestValue(t *testing.T) {
	if Value() != 1 {
		t.Fatal("unexpected value")
	}
}
`)

	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           []string{"go.mod", "internal/sample/main.go", "internal/sample/main_test.go"},
		Modules:         []module{{Root: ".", Path: "example.com/test-main"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}

	for _, value := range result.Observations {
		target, ok := value.(symbolObservation)
		if !ok {
			continue
		}
		if target.SemanticKey == "function:example.com/test-main/internal/sample.test:main" &&
			target.Generated &&
			target.Name == "main" &&
			target.Namespace == "example.com/test-main/internal/sample.test" {
			return
		}
	}
	t.Fatalf("generated test main target not materialized")
}
