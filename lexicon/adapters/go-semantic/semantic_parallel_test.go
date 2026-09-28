package main

import (
	"path/filepath"
	"reflect"
	"runtime"
	"testing"
)

func TestParallelSemanticCollectionMatchesSerialAndReductionShapes(t *testing.T) {
	root, err := filepath.Abs(filepath.Join("..", "..", "testdata", "go_oracle", "repositories", "parallel"))
	if err != nil {
		t.Fatal(err)
	}
	base := request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files: []string{
			"extra.go",
			"go.mod",
			"main.go",
			"service/service.go",
			"worker/worker.go",
		},
		Modules: []module{{Root: ".", Path: "example.com/oracle/parallel"}},
	}
	settings := []execution{
		{Workers: 1, Shards: 1, MergeFanIn: 2},
		{Workers: 2, Shards: 2, MergeFanIn: 2},
		{Workers: 2, Shards: 4, MergeFanIn: 2},
		{Workers: 4, Shards: 8, MergeFanIn: 4},
		{Workers: 3, Shards: 6, MergeFanIn: 8},
	}
	var want response
	for position, current := range settings {
		value := base
		value.Execution = current
		got, err := scanStructural(value)
		if err != nil {
			t.Fatal(err)
		}
		if position == 0 {
			want = got
			continue
		}
		if !reflect.DeepEqual(got, want) {
			t.Fatalf("execution %#v changed semantic output", current)
		}
	}
}

func TestExecutionNormalizationPreservesLegacyBounds(t *testing.T) {
	got := normalizedExecution(execution{Workers: 99, Shards: 99, MergeFanIn: 1}, 3)
	if got.Shards != 3 {
		t.Fatalf("shards = %d, want 3", got.Shards)
	}
	wantWorkers := min(3, runtime.GOMAXPROCS(0))
	if got.Workers != wantWorkers {
		t.Fatalf("workers = %d, want %d", got.Workers, wantWorkers)
	}
	if got.MergeFanIn != 2 {
		t.Fatalf("merge fan-in = %d, want 2", got.MergeFanIn)
	}
}
