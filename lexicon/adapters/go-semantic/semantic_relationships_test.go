package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestSemanticRelationshipsMatchEmbeddedAndInterfaceSemantics(t *testing.T) {
	root, err := filepath.Abs(filepath.Join("..", "..", "testdata", "go_oracle", "repositories", "relationships"))
	if err != nil {
		t.Fatal(err)
	}
	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           []string{"go.mod", "main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/oracle/relationships"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}

	expected := []struct{ source, target, kind string }{
		{"type:example.com/oracle/relationships:Embedded", "type:example.com/oracle/relationships:Base", "extends"},
		{"type:example.com/oracle/relationships:Extended", "type:example.com/oracle/relationships:Contract", "extends"},
		{"type:example.com/oracle/relationships:Base", "type:example.com/oracle/relationships:Contract", "implements"},
		{"type:example.com/oracle/relationships:Embedded", "type:example.com/oracle/relationships:Contract", "implements"},
		{"type:example.com/oracle/relationships:Direct", "type:example.com/oracle/relationships:Contract", "implements"},
		{"method:example.com/oracle/relationships:Base.Run", "interface-method:example.com/oracle/relationships:Contract.Run", "implements"},
		{"method:example.com/oracle/relationships:Direct.Run", "interface-method:example.com/oracle/relationships:Contract.Run", "implements"},
	}
	for _, want := range expected {
		if !hasRelationship(result.Observations, want.source, want.target, want.kind) {
			t.Fatalf("missing %s %s -> %s", want.kind, want.source, want.target)
		}
	}
	for _, value := range result.Observations {
		item, ok := value.(relationshipObservation)
		if ok && item.Kind == "implements" && item.SourceKey == item.TargetKey {
			t.Fatalf("implements self-edge: %#v", item)
		}
	}
}

func TestSemanticRelationshipsPreserveExternalEmbeddedType(t *testing.T) {
	root := t.TempDir()
	writeRelationshipFile(t, root, "go.mod", "module example.com/external\n\ngo 1.22\n")
	writeRelationshipFile(t, root, "main.go", `package external

import "io"

type Wrapped struct{ io.Reader }
`)

	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           []string{"go.mod", "main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/external"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !hasRelationship(
		result.Observations,
		"type:example.com/external:Wrapped",
		"type:io:Reader",
		"extends",
	) {
		t.Fatalf("missing external extends relationship: %#v", relationships(result.Observations))
	}
}

func TestSemanticRelationshipsPreserveEmbeddedOverride(t *testing.T) {
	root := t.TempDir()
	writeRelationshipFile(t, root, "go.mod", "module example.com/override\n\ngo 1.22\n")
	writeRelationshipFile(t, root, "main.go", `package override

type Base struct{}
func (Base) Run() {}

type Derived struct{ Base }
func (Derived) Run() {}
`)

	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           []string{"go.mod", "main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/override"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !hasRelationship(
		result.Observations,
		"method:example.com/override:Derived.Run",
		"method:example.com/override:Base.Run",
		"overrides",
	) {
		t.Fatalf("missing override relationship: %#v", relationships(result.Observations))
	}
}

func hasRelationship(values []observation, source, target, kind string) bool {
	for _, value := range values {
		item, ok := value.(relationshipObservation)
		if ok && item.SourceKey == source && item.TargetKey == target && item.Kind == kind {
			return true
		}
	}
	return false
}

func relationships(values []observation) []relationshipObservation {
	var result []relationshipObservation
	for _, value := range values {
		if item, ok := value.(relationshipObservation); ok {
			result = append(result, item)
		}
	}
	return result
}

func writeRelationshipFile(t *testing.T, root, name, content string) {
	t.Helper()
	if err := os.WriteFile(filepath.Join(root, name), []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}
