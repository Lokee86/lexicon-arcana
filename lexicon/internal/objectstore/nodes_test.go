package objectstore

import (
	"encoding/json"
	"testing"
)

func TestLanguageNodesReadsNodesWithoutRequiringEdgeMaterialization(t *testing.T) {
	store := Store{Root: t.TempDir()}
	node := json.RawMessage(`{"record":"node","id":"node-1","kind":"function","name":"run","path":"a.py","qualified_name":"a.run","attributes":{"role":"entry"},"span":{"path":"a.py","start_line":2,"start_column":1,"end_line":4,"end_column":2}}`)
	edge := json.RawMessage(`{"record":"edge","source":"node-1","target":"node-2","relation":"calls"}`)
	id, err := store.WriteObject(FactObject{
		Language: "python", AdapterVersion: "test", SchemaVersion: 1,
		AnalysisConfigID: "config", Records: []json.RawMessage{node, edge},
	})
	if err != nil {
		t.Fatal(err)
	}
	entry := LanguageEntry{
		Language: "python", AdapterVersion: "test", SchemaVersion: 1,
		Repository: "repo", AnalysisConfigID: "config", SharedObjectID: id,
	}
	nodes, err := store.LanguageNodes(entry)
	if err != nil {
		t.Fatal(err)
	}
	if len(nodes) != 1 {
		t.Fatalf("LanguageNodes returned %d nodes, want 1", len(nodes))
	}
	got := nodes[0]
	if got.ID != "node-1" || got.Kind != "function" || got.Path != "a.py" || got.QualifiedName != "a.run" {
		t.Fatalf("unexpected node: %#v", got)
	}
	if got.Span == nil || got.Span.StartLine != 2 || got.Span.EndLine != 4 {
		t.Fatalf("unexpected span: %#v", got.Span)
	}
	var attributes map[string]any
	if err := json.Unmarshal(got.Attributes, &attributes); err != nil {
		t.Fatal(err)
	}
	if attributes["role"] != "entry" {
		t.Fatalf("unexpected attributes: %#v", attributes)
	}
}
