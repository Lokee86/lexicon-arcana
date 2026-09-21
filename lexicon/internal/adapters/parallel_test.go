package adapters

import (
	"reflect"
	"testing"
)

func TestGoAdapterArgumentsIncludeSemanticParallelism(t *testing.T) {
	request := Request{
		Language: "go", Repository: "repo", Output: "facts.jsonl",
		Workers: 16, Shards: 64, MergeFanIn: 4,
	}
	want := []string{
		"--repo", "repo", "--output", "facts.jsonl",
		"--workers", "16", "--shards", "64", "--merge-fan-in", "4",
	}
	if got := adapterArguments(request); !reflect.DeepEqual(got, want) {
		t.Fatalf("arguments = %#v, want %#v", got, want)
	}
}

func TestPythonAdapterArgumentsIncludePartitionedExecution(t *testing.T) {
	request := Request{
		Language: "python", Repository: "repo", Output: "facts.jsonl",
		Workers: 16, Shards: 64, MergeFanIn: 4,
	}
	want := []string{"--repo", "repo", "--output", "facts.jsonl", "--workers", "16", "--shards", "64", "--merge-fan-in", "4"}
	if got := adapterArguments(request); !reflect.DeepEqual(got, want) {
		t.Fatalf("arguments = %#v, want %#v", got, want)
	}
}
