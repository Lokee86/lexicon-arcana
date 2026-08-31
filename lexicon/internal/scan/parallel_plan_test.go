package scan

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestExecutionPlanScalesGoSemanticShards(t *testing.T) {
	previousGOMAXPROCS := runtime.GOMAXPROCS(4)
	t.Cleanup(func() { runtime.GOMAXPROCS(previousGOMAXPROCS) })

	stateRoot := t.TempDir()
	sourceRoot := filepath.Join(stateRoot, "source")
	if err := os.MkdirAll(sourceRoot, 0o755); err != nil {
		t.Fatal(err)
	}
	for index := 0; index < 160; index++ {
		path := filepath.Join(sourceRoot, fmt.Sprintf("file_%03d.go", index))
		if err := os.WriteFile(path, []byte("package sample\n"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	t.Setenv("LEXICON_MAX_WORKERS", "3")
	scanner := &Scanner{StateRoot: stateRoot}
	plan, err := scanner.executionPlan(analysisPlan{Language: "go", Full: true})
	if err != nil {
		t.Fatal(err)
	}
	if plan.LogicalShards != 4 {
		t.Fatalf("logical shards = %d, want 4", plan.LogicalShards)
	}
	if plan.ActiveWorkers != 2 {
		t.Fatalf("active workers = %d, want 2", plan.ActiveWorkers)
	}
	if plan.MergeFanIn != 2 {
		t.Fatalf("merge fan-in = %d, want 2", plan.MergeFanIn)
	}
	if plan.SourceFiles != 160 {
		t.Fatalf("source files = %d, want 160", plan.SourceFiles)
	}
}

func TestLogicalShardCountScalesToEnterpriseRepositories(t *testing.T) {
	if got := logicalShardCount(536, 0); got != 16 {
		t.Fatalf("medium logical shards = %d, want 16", got)
	}
	if got := logicalShardCount(10_000, 0); got != 512 {
		t.Fatalf("logical shards = %d, want 512", got)
	}
	if got := logicalShardCount(250_000, 0); got != maxLogicalShards {
		t.Fatalf("logical shards = %d, want cap %d", got, maxLogicalShards)
	}
}

func TestExecutionPlanKeepsUnsupportedAdaptersSingleWorker(t *testing.T) {
	scanner := &Scanner{StateRoot: filepath.Join(t.TempDir(), "missing-state")}
	plan, err := scanner.executionPlan(analysisPlan{Language: "python", Full: true})
	if err != nil {
		t.Fatal(err)
	}
	if plan.LogicalShards != 1 || plan.ActiveWorkers != 1 || plan.ReservedWeight != 1 {
		t.Fatalf("python execution plan = %#v", plan)
	}
	if plan.SourceFiles != 0 || plan.SourceBytes != 0 {
		t.Fatalf("python execution plan performed unnecessary inventory: %#v", plan)
	}
}
