package objectstore

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestRequiresFullAnalysisAllowsResolvedRelationshipTopology(t *testing.T) {
	store := Store{Root: t.TempDir()}
	configID := "sha256:config"
	aID, err := store.WriteObject(FactObject{
		Language: "python", Owner: "a.py", SourceContentID: ContentID([]byte("a")),
		AdapterVersion: "test", SchemaVersion: 1, AnalysisConfigID: configID,
		Records: records(
			`{"id":"node-a","kind":"function","owner":"a.py","record":"node"}`,
			`{"owner":"a.py","record":"edge","relation":"calls","source":"node-a","target":"node-x"}`,
		),
	})
	if err != nil {
		t.Fatal(err)
	}
	xID, err := store.WriteObject(FactObject{
		Language: "python", Owner: "x.py", SourceContentID: ContentID([]byte("x")),
		AdapterVersion: "test", SchemaVersion: 1, AnalysisConfigID: configID,
		Records: records(`{"id":"node-x","kind":"function","owner":"x.py","record":"node"}`),
	})
	if err != nil {
		t.Fatal(err)
	}
	_, err = store.Publish(Manifest{StateCommit: "state", Languages: []LanguageEntry{{
		Language: "python", AdapterVersion: "test", SchemaVersion: 1,
		Repository: "repo", AnalysisConfigID: configID,
		Files: []FileEntry{
			{Path: "a.py", Language: "python", ObjectID: aID},
			{Path: "x.py", Language: "python", ObjectID: xID},
		},
	}}})
	if err != nil {
		t.Fatal(err)
	}

	path := filepath.Join(t.TempDir(), "incremental.jsonl")
	writeTopologyStream(t, path, "node-x")
	analysis, err := ReadAnalysis(path, "python")
	if err != nil {
		t.Fatal(err)
	}
	full, err := store.RequiresFullAnalysis("python", []string{"a.py"}, analysis)
	if err != nil || full {
		t.Fatalf("existing topology required full analysis: full=%v err=%v", full, err)
	}
	writeTopologyStream(t, path, "node-y")
	analysis, err = ReadAnalysis(path, "python")
	if err != nil {
		t.Fatal(err)
	}
	full, err = store.RequiresFullAnalysis("python", []string{"a.py"}, analysis)
	if err != nil || full {
		t.Fatalf("new resolved topology required full analysis: full=%v err=%v", full, err)
	}
}

func TestRequiresFullAnalysisAllowsNewMissingTarget(t *testing.T) {
	store := topologyStoreWithFile(t, "a.py")
	path := filepath.Join(t.TempDir(), "incremental-missing.jsonl")
	writeUnresolvedTopologyStream(t, path, "missing-target")
	analysis, err := ReadAnalysis(path, "python")
	if err != nil {
		t.Fatal(err)
	}
	full, err := store.RequiresFullAnalysis("python", []string{"a.py"}, analysis)
	if err != nil || full {
		t.Fatalf("new missing target required full analysis: full=%v err=%v", full, err)
	}
}

func TestRequiresFullAnalysisForAmbiguousOrGeneratedTarget(t *testing.T) {
	for _, reason := range []string{"ambiguous-target", "generated-target"} {
		t.Run(reason, func(t *testing.T) {
			store := topologyStoreWithFile(t, "a.py")
			path := filepath.Join(t.TempDir(), "incremental-sensitive.jsonl")
			writeUnresolvedTopologyStream(t, path, reason)
			analysis, err := ReadAnalysis(path, "python")
			if err != nil {
				t.Fatal(err)
			}
			full, err := store.RequiresFullAnalysis("python", []string{"a.py"}, analysis)
			if err != nil || !full {
				t.Fatalf("new %s did not require full analysis: full=%v err=%v", reason, full, err)
			}
		})
	}
}

func topologyStoreWithFile(t *testing.T, path string) Store {
	t.Helper()
	store := Store{Root: t.TempDir()}
	configID := "sha256:config"
	objectID, err := store.WriteObject(FactObject{
		Language: "python", Owner: path, SourceContentID: ContentID([]byte(path)),
		AdapterVersion: "test", SchemaVersion: 1, AnalysisConfigID: configID,
		Records: records(`{"id":"node-a","kind":"function","owner":"a.py","record":"node"}`),
	})
	if err != nil {
		t.Fatal(err)
	}
	_, err = store.Publish(Manifest{StateCommit: "state", Languages: []LanguageEntry{{
		Language: "python", AdapterVersion: "test", SchemaVersion: 1,
		Repository: "repo", AnalysisConfigID: configID,
		Files: []FileEntry{{Path: path, Language: "python", ObjectID: objectID}},
	}}})
	if err != nil {
		t.Fatal(err)
	}
	return store
}

func writeUnresolvedTopologyStream(t *testing.T, path, reason string) {
	t.Helper()
	values := []map[string]any{
		{"adapter_version": "test", "changed_files": []string{"a.py"}, "language": "python", "mode": "incremental", "record": "lexicon", "removed_files": []string{}, "repository": "repo", "schema_version": 1, "shared_complete": false},
		{"id": "node-a", "kind": "function", "owner": "a.py", "record": "node"},
		{"candidate_name": "new.module", "expression": "import new.module", "owner": "a.py", "reason": reason, "record": "unresolved", "relation": "imports", "source": "node-a"},
	}
	file, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	encoder := json.NewEncoder(file)
	for _, value := range values {
		if err := encoder.Encode(value); err != nil {
			_ = file.Close()
			t.Fatal(err)
		}
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}
}

func writeTopologyStream(t *testing.T, path, target string) {
	t.Helper()
	values := []map[string]any{
		{"adapter_version": "test", "changed_files": []string{"a.py"}, "language": "python", "mode": "incremental", "record": "lexicon", "removed_files": []string{}, "repository": "repo", "schema_version": 1, "shared_complete": false},
		{"id": "node-a", "kind": "function", "owner": "a.py", "record": "node"},
		{"owner": "a.py", "record": "edge", "relation": "calls", "source": "node-a", "target": target},
	}
	file, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	encoder := json.NewEncoder(file)
	for _, value := range values {
		if err := encoder.Encode(value); err != nil {
			_ = file.Close()
			t.Fatal(err)
		}
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}
}

func TestRequiresFullAnalysisAllowsUnrelatedPythonAddition(t *testing.T) {
	store := Store{Root: t.TempDir()}
	configID := "sha256:config"
	existingID, err := store.WriteObject(FactObject{
		Language: "python", Owner: "existing.py", SourceContentID: ContentID([]byte("existing")),
		AdapterVersion: "test", SchemaVersion: 1, AnalysisConfigID: configID,
		Records: records(
			`{"id":"node-existing","kind":"function","owner":"existing.py","record":"node"}`,
			`{"candidate_name":"other.module","expression":"import other.module","owner":"existing.py","reason":"missing-target","record":"unresolved","relation":"imports","source":"node-existing"}`,
		),
	})
	if err != nil {
		t.Fatal(err)
	}
	_, err = store.Publish(Manifest{StateCommit: "state", Languages: []LanguageEntry{{
		Language: "python", AdapterVersion: "test", SchemaVersion: 1,
		Repository: "repo", AnalysisConfigID: configID,
		Files: []FileEntry{{Path: "existing.py", Language: "python", ObjectID: existingID}},
	}}})
	if err != nil {
		t.Fatal(err)
	}

	path := filepath.Join(t.TempDir(), "added.jsonl")
	writeAddedPythonStream(t, path, "new.module")
	analysis, err := ReadAnalysis(path, "python")
	if err != nil {
		t.Fatal(err)
	}
	full, err := store.RequiresFullAnalysis("python", []string{"new.py"}, analysis)
	if err != nil || full {
		t.Fatalf("unrelated addition required full analysis: full=%v err=%v", full, err)
	}
}

func writeAddedPythonStream(t *testing.T, path, module string) {
	t.Helper()
	values := []map[string]any{
		{"adapter_version": "test", "changed_files": []string{"new.py"}, "language": "python", "mode": "incremental", "record": "lexicon", "removed_files": []string{}, "repository": "repo", "schema_version": 1, "shared_complete": true},
		{"id": "file-new", "kind": "file", "name": "new.py", "path": "new.py", "qualified_name": "new.py", "record": "node"},
		{"id": "module-new", "kind": "module", "name": "module", "path": "new.py", "qualified_name": module, "record": "node"},
		{"record": "edge", "relation": "contains", "source": "file-new", "target": "module-new"},
	}
	file, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	encoder := json.NewEncoder(file)
	for _, value := range values {
		if err := encoder.Encode(value); err != nil {
			_ = file.Close()
			t.Fatal(err)
		}
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}
}
