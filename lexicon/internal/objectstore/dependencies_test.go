package objectstore

import (
	"encoding/json"
	"reflect"
	"testing"
)

func TestIncrementalScopeUsesDirectDependentsWithoutGlobalUnresolvedSeeds(t *testing.T) {
	store := Store{Root: t.TempDir()}
	metadata := FactObject{Language: "python", AdapterVersion: "test", SchemaVersion: 1, AnalysisConfigID: "sha256:config"}
	files := []struct {
		path    string
		records []json.RawMessage
	}{
		{"a.py", records(`{"id":"node-a","kind":"function","record":"node"}`)},
		{"b.py", records(`{"id":"node-b","kind":"function","record":"node"}`, `{"record":"edge","relation":"calls","source":"node-b","target":"node-a"}`)},
		{"c.py", records(`{"id":"node-c","kind":"function","record":"node"}`, `{"record":"edge","relation":"calls","source":"node-c","target":"node-b"}`)},
		{"d.py", records(`{"id":"node-d","kind":"function","record":"node"}`, `{"reason":"missing-target","record":"unresolved","source":"node-d"}`)},
		{"e.py", records(`{"id":"node-e","kind":"function","record":"node"}`, `{"reason":"builtin-target","record":"unresolved","source":"node-e"}`)},
	}
	entries := make([]FileEntry, 0, len(files))
	for _, file := range files {
		object := metadata
		object.Owner = file.path
		object.SourceContentID = ContentID([]byte(file.path))
		object.Records = file.records
		id, err := store.WriteObject(object)
		if err != nil {
			t.Fatal(err)
		}
		entries = append(entries, FileEntry{Path: file.path, Language: "python", ContentID: object.SourceContentID, ObjectID: id})
	}
	_, err := store.Publish(Manifest{StateCommit: "state", Languages: []LanguageEntry{{Language: "python", AdapterVersion: "test", SchemaVersion: 1, Repository: "repo", AnalysisConfigID: "sha256:config", Files: entries}}})
	if err != nil {
		t.Fatal(err)
	}
	impacted, err := store.ImpactedFiles("python", []string{"a.py"})
	if err != nil {
		t.Fatal(err)
	}
	if want := []string{"a.py", "b.py"}; !reflect.DeepEqual(impacted, want) {
		t.Fatalf("impacted=%v want=%v", impacted, want)
	}
	emit, context, err := store.DependencyScope("python", []string{"b.py"})
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(emit, []string{"b.py", "c.py"}) {
		t.Fatalf("emit=%v", emit)
	}
	if !reflect.DeepEqual(context, []string{"a.py", "b.py", "c.py"}) {
		t.Fatalf("context=%v", context)
	}
	for _, path := range []string{"a.py", "b.py", "d.py", "e.py"} {
		full, err := store.DirectChangesRequireFull("python", []string{path})
		if err != nil || full {
			t.Fatalf("%s unexpectedly required full analysis: full=%v err=%v", path, full, err)
		}
	}
	full, err := store.DirectChangesRequireFull("python", []string{"missing.py"})
	if err != nil || !full {
		t.Fatalf("missing root full=%v err=%v", full, err)
	}
}

func TestDependencyScopeOwnsSharedModuleNodesByPath(t *testing.T) {
	store := Store{Root: t.TempDir()}
	metadata := FactObject{Language: "python", AdapterVersion: "test", SchemaVersion: 1, AnalysisConfigID: "sha256:config"}
	writeFile := func(path string, values ...string) FileEntry {
		object := metadata
		object.Owner = path
		object.SourceContentID = ContentID([]byte(path))
		object.Records = records(values...)
		id, err := store.WriteObject(object)
		if err != nil {
			t.Fatal(err)
		}
		return FileEntry{Path: path, Language: "python", ContentID: object.SourceContentID, ObjectID: id}
	}
	a := writeFile("a.py", `{"id":"node-a","kind":"function","record":"node"}`)
	b := writeFile("b.py", `{"id":"node-b","kind":"function","record":"node"}`, `{"record":"edge","relation":"imports","source":"node-b","target":"module-a"}`)
	shared := metadata
	shared.Records = records(`{"id":"module-a","kind":"module","path":"a.py","record":"node"}`)
	sharedID, err := store.WriteObject(shared)
	if err != nil {
		t.Fatal(err)
	}
	_, err = store.Publish(Manifest{StateCommit: "state", Languages: []LanguageEntry{{Language: "python", AdapterVersion: "test", SchemaVersion: 1, Repository: "repo", AnalysisConfigID: "sha256:config", SharedObjectID: sharedID, Files: []FileEntry{a, b}}}})
	if err != nil {
		t.Fatal(err)
	}
	impacted, err := store.ImpactedFiles("python", []string{"a.py"})
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(impacted, []string{"a.py", "b.py"}) {
		t.Fatalf("impacted=%v", impacted)
	}
}

func TestIncrementalScopeWithAdditionsChecksUnresolvedPythonModules(t *testing.T) {
	store := Store{Root: t.TempDir()}
	object := FactObject{
		Language: "python", Owner: "existing.py", SourceContentID: ContentID([]byte("existing")),
		AdapterVersion: "test", SchemaVersion: 1, AnalysisConfigID: "sha256:config",
		Records: records(
			`{"id":"node-existing","kind":"function","record":"node"}`,
			`{"candidate_name":"plugin_runtime.capabilities","reason":"missing-target","record":"unresolved","relation":"imports","source":"node-existing"}`,
		),
	}
	id, err := store.WriteObject(object)
	if err != nil {
		t.Fatal(err)
	}
	_, err = store.Publish(Manifest{StateCommit: "state", Languages: []LanguageEntry{{
		Language: "python", AdapterVersion: "test", SchemaVersion: 1,
		Repository: "repo", AnalysisConfigID: "sha256:config",
		Files: []FileEntry{{Path: "existing.py", Language: "python", ObjectID: id}},
	}}})
	if err != nil {
		t.Fatal(err)
	}

	full, _, _, err := store.IncrementalScopeWithAdditions("python", nil, []string{"plugin_runtime/state.py"})
	if err != nil || full {
		t.Fatalf("unrelated addition required full analysis: full=%v err=%v", full, err)
	}
	full, _, _, err = store.IncrementalScopeWithAdditions("python", nil, []string{"plugin_runtime/capabilities.py"})
	if err != nil || !full {
		t.Fatalf("matching unresolved module did not require full analysis: full=%v err=%v", full, err)
	}
}

func TestPythonModuleCandidate(t *testing.T) {
	cases := map[string]string{
		"plugin_runtime/state.py":    "plugin_runtime.state",
		"plugin_runtime/__init__.py": "plugin_runtime",
		"module.py":                  "module",
	}
	for path, want := range cases {
		got, ok := pythonModuleCandidate(path)
		if !ok || got != want {
			t.Fatalf("pythonModuleCandidate(%q) = %q, %v; want %q, true", path, got, ok, want)
		}
	}
}

func records(values ...string) []json.RawMessage {
	result := make([]json.RawMessage, len(values))
	for i, v := range values {
		result[i] = json.RawMessage(v)
	}
	return result
}
