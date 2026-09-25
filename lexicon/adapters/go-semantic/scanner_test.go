package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestStructuralScanKeepsInactiveBuildVariants(t *testing.T) {
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

	enabled := declarations(result.Records, "function:example.com/oracle/tagged:Enabled")
	if len(enabled) != 2 {
		t.Fatalf("Enabled declarations = %d, want 2", len(enabled))
	}
	if enabled[0].Owner != "enabled_default.go" || enabled[1].Owner != "enabled_special.go" {
		t.Fatalf("Enabled owners = %q, %q", enabled[0].Owner, enabled[1].Owner)
	}
	test := declarations(result.Records, "test:example.com/oracle/tagged:TestTagged")
	if len(test) != 1 || test[0].Kind != "test" || test[0].Owner != "special_test.go" {
		t.Fatalf("TestTagged declaration = %#v", test)
	}
	for _, identity := range []string{
		"import:external:os",
		"import:external:os/exec",
		"import:external:testing",
	} {
		if len(declarations(result.Records, identity)) != 1 {
			t.Fatalf("missing import %q", identity)
		}
	}
}

func TestStructuralScanUsesOnlyRustSuppliedInventory(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "go.mod"), []byte(`module example.com/inventory
`), 0o644); err != nil {
		t.Fatal(err)
	}
	for name, source := range map[string]string{
		"visible.go": `package inventory
func Visible() {}
`,
		"hidden.go": `package inventory
func Hidden() {}
`,
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
	for _, record := range result.Records {
		if strings.Contains(record.Identity, "Hidden") || record.Owner == "hidden.go" {
			t.Fatalf("helper rediscovered excluded input: %#v", record)
		}
	}
	if len(declarations(result.Records, "function:example.com/inventory:Visible")) != 1 {
		t.Fatal("missing visible declaration")
	}
}

func declarations(records []declaration, identity string) []declaration {
	var result []declaration
	for _, record := range records {
		if record.Identity == identity {
			result = append(result, record)
		}
	}
	return result
}
