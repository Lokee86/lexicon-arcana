package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestStructuralScanKeepsInactiveBuildVariants(t *testing.T) {
	root, err := filepath.Abs(filepath.Join("..", "..", "testdata", "go_oracle", "repositories", "build_tags"))
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

	enabled := declarations(result.Observations, "function:example.com/oracle/tagged:Enabled")
	if len(enabled) != 2 {
		t.Fatalf("Enabled declarations = %d, want 2", len(enabled))
	}
	if enabled[0].Owner != "enabled_default.go" || enabled[1].Owner != "enabled_special.go" {
		t.Fatalf("Enabled owners = %q, %q", enabled[0].Owner, enabled[1].Owner)
	}
	test := declarations(result.Observations, "test:example.com/oracle/tagged:TestTagged")
	if len(test) != 1 || test[0].Kind != "test" || test[0].Owner != "special_test.go" {
		t.Fatalf("TestTagged declaration = %#v", test)
	}
	for _, key := range []string{
		"import:external:os",
		"import:external:os/exec",
		"import:external:testing",
	} {
		if len(declarations(result.Observations, key)) != 1 {
			t.Fatalf("missing import %q", key)
		}
	}
}

func TestStructuralScanPreservesNestedClosureOwnership(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "go.mod"), []byte("module example.com/nested\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	source := "package nested\nfunc Outer() {\n\t_ = func() {\n\t\t_ = func() {}\n\t}\n}\n"
	if err := os.WriteFile(filepath.Join(root, "main.go"), []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           []string{"go.mod", "main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/nested"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}
	var closures []declarationObservation
	for _, value := range result.Observations {
		item, ok := value.(declarationObservation)
		if ok && strings.HasPrefix(item.SemanticKey, "closure:example.com/nested:main.go:") {
			closures = append(closures, item)
		}
	}
	if len(closures) != 2 {
		t.Fatalf("closures = %d, want 2: %#v", len(closures), closures)
	}
	nested := 0
	for _, closure := range closures {
		if strings.HasPrefix(closure.Metadata["container"], "closure:example.com/nested:main.go:") {
			nested++
		}
	}
	if nested != 1 {
		t.Fatalf("nested closure ownership count = %d, want 1: %#v", nested, closures)
	}
}

func TestStructuralScanUsesOnlyRustSuppliedInventory(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "go.mod"), []byte("module example.com/inventory\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	for name, source := range map[string]string{
		"visible.go": "package inventory\nfunc Visible() {}\n",
		"hidden.go":  "package inventory\nfunc Hidden() {}\n",
	} {
		if err := os.WriteFile(filepath.Join(root, name), []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           []string{"go.mod", "visible.go"},
		Modules:         []module{{Root: ".", Path: "example.com/inventory"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}
	for _, value := range result.Observations {
		item, ok := value.(declarationObservation)
		if !ok {
			continue
		}
		if strings.Contains(item.SemanticKey, "Hidden") || item.Owner == "hidden.go" {
			t.Fatalf("helper rediscovered excluded input: %#v", item)
		}
	}
	if len(declarations(result.Observations, "function:example.com/inventory:Visible")) != 1 {
		t.Fatal("missing visible declaration")
	}
}

func declarations(values []observation, semanticKey string) []declarationObservation {
	var result []declarationObservation
	for _, value := range values {
		item, ok := value.(declarationObservation)
		if ok && item.SemanticKey == semanticKey {
			result = append(result, item)
		}
	}
	return result
}
