package main

import (
	"os"
	"path/filepath"
	"testing"
)

const updateGoOracleEnv = "LEXICON_UPDATE_GO_ORACLE"

func TestGoMigrationOracle(t *testing.T) {
	fixtures := []string{
		"basic_calls",
		"relationships",
		"higher_order",
		"dataflow",
		"build_tags",
		"multi_module",
		"parallel",
	}

	for _, name := range fixtures {
		t.Run(name, func(t *testing.T) {
			root := filepath.Join("testdata", "oracle", name)
			first, summary, err := scanRepository(root)
			if err != nil {
				t.Fatal(err)
			}
			if summary.SemanticErrors != 0 {
				t.Fatalf("semantic errors = %d, want 0", summary.SemanticErrors)
			}
			second, _, err := scanRepository(root)
			if err != nil {
				t.Fatal(err)
			}
			got := encodeFacts(first)
			if got != encodeFacts(second) {
				t.Fatal("oracle fixture output is not deterministic")
			}

			golden := filepath.Join("testdata", "oracle_golden", name+".jsonl")
			if os.Getenv(updateGoOracleEnv) == "1" {
				if err := os.MkdirAll(filepath.Dir(golden), 0o755); err != nil {
					t.Fatal(err)
				}
				if err := os.WriteFile(golden, []byte(got), 0o644); err != nil {
					t.Fatal(err)
				}
			}
			want, err := os.ReadFile(golden)
			if err != nil {
				t.Fatalf("read oracle %s: %v; regenerate with %s=1 go test ./...", golden, err, updateGoOracleEnv)
			}
			if got != string(want) {
				t.Fatalf("oracle output changed for %s; inspect the semantic diff before regenerating %s", name, golden)
			}
		})
	}
}

func TestGoMigrationOracleParallelDeterminism(t *testing.T) {
	root := filepath.Join("testdata", "oracle", "parallel")
	configurations := []ScanOptions{
		{SemanticWorkers: 1, SemanticShards: 1, MergeFanIn: 2},
		{SemanticWorkers: 4, SemanticShards: 8, MergeFanIn: 4},
		{SemanticWorkers: 3, SemanticShards: 6, MergeFanIn: 8},
	}

	var want string
	for index, options := range configurations {
		facts, summary, err := scanRepositoryWithOptions(root, options)
		if err != nil {
			t.Fatal(err)
		}
		if summary.SemanticErrors != 0 {
			t.Fatalf("configuration %d semantic errors = %d, want 0", index, summary.SemanticErrors)
		}
		got := encodeFacts(facts)
		if index == 0 {
			want = got
			continue
		}
		if got != want {
			t.Fatalf("configuration %d changed canonical oracle output", index)
		}
	}
}

func TestGoMigrationOracleDependencyParsing(t *testing.T) {
	filename := filepath.Join("testdata", "oracle", "multi_module", "services", "api", "go.mod")
	got, err := parseGoDependencies(filename)
	if err != nil {
		t.Fatal(err)
	}
	want := []goDependency{
		{name: "example.com/oracle/shared", constraint: "v0.0.0", source: "go.mod:require", category: "runtime"},
		{name: "example.com/external", constraint: "v1.2.3", source: "go.mod:require", category: "runtime"},
		{name: "example.com/oracle/shared", replacement: "../../shared", source: "go.mod:replace", category: "runtime"},
		{name: "example.com/external", replacement: "example.com/fork@v1.4.0", source: "go.mod:replace", category: "runtime"},
	}
	if len(got) != len(want) {
		t.Fatalf("dependency observations = %#v, want %#v", got, want)
	}
	for index := range want {
		if got[index] != want[index] {
			t.Fatalf("dependency observation %d = %#v, want %#v", index, got[index], want[index])
		}
	}
}
