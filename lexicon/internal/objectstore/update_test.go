package objectstore

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestIncrementalLanguageUpdateReusesUnchangedObjectsAndSharedFacts(t *testing.T) {
	store := Store{Root: t.TempDir()}
	source := t.TempDir()
	for path, contents := range map[string]string{
		"a.py": "value = 1\n",
		"b.py": "other = 1\n",
	} {
		if err := os.WriteFile(filepath.Join(source, path), []byte(contents), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	fullPath := filepath.Join(t.TempDir(), "full.jsonl")
	writeAnalysisStream(t, fullPath, []string{
		`{"adapter_version":"test","language":"python","mode":"full","record":"lexicon","repository":"repo","schema_version":1}`,
		`{"id":"repo","kind":"repository","name":"repo","path":".","qualified_name":"repo","record":"node"}`,
		`{"id":"a-old","kind":"file","name":"a.py","owner":"a.py","path":"a.py","qualified_name":"a.py","record":"node"}`,
		`{"id":"b","kind":"file","name":"b.py","owner":"b.py","path":"b.py","qualified_name":"b.py","record":"node"}`,
	})
	full, err := ReadAnalysis(fullPath, "python")
	if err != nil {
		t.Fatal(err)
	}
	entry, err := store.BuildFullLanguage(full, source, "python", "sha256:config", "sha256:adapter")
	if err != nil {
		t.Fatal(err)
	}
	oldA := fileEntry(t, entry, "a.py")
	oldB := fileEntry(t, entry, "b.py")
	oldShared := entry.SharedObjectID

	if err := os.WriteFile(filepath.Join(source, "a.py"), []byte("value = 2\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	incrementalPath := filepath.Join(t.TempDir(), "incremental.jsonl")
	writeAnalysisStream(t, incrementalPath, []string{
		`{"adapter_version":"test","changed_files":["a.py"],"language":"python","mode":"incremental","record":"lexicon","removed_files":[],"repository":"repo","schema_version":1,"shared_complete":true}`,
		`{"id":"scoped-repo","kind":"repository","name":"repo","path":".","qualified_name":"repo","record":"node"}`,
		`{"id":"a-new","kind":"file","name":"a.py","owner":"a.py","path":"a.py","qualified_name":"a.py","record":"node"}`,
	})
	incremental, err := ReadAnalysis(incrementalPath, "python")
	if err != nil {
		t.Fatal(err)
	}
	updated, err := store.BuildIncrementalLanguage(
		entry,
		incremental,
		source,
		"sha256:config",
		"sha256:adapter",
		[]string{"a.py"},
		[]string{},
		false,
	)
	if err != nil {
		t.Fatal(err)
	}
	newA := fileEntry(t, updated, "a.py")
	newB := fileEntry(t, updated, "b.py")
	if newA.ObjectID == oldA.ObjectID || newA.ContentID == oldA.ContentID {
		t.Fatalf("changed object was reused: old=%#v new=%#v", oldA, newA)
	}
	if newB != oldB {
		t.Fatalf("unchanged object changed: old=%#v new=%#v", oldB, newB)
	}
	if updated.SharedObjectID != oldShared {
		t.Fatalf("partial shared facts replaced: old=%s new=%s", oldShared, updated.SharedObjectID)
	}
}

func TestIncrementalLanguageMergesScopedSharedFacts(t *testing.T) {
	store := Store{Root: t.TempDir()}
	source := t.TempDir()
	for path, contents := range map[string]string{
		"existing.py": "value = 1\n",
		"new.py":      "other = 2\n",
	} {
		if err := os.WriteFile(filepath.Join(source, path), []byte(contents), 0o644); err != nil {
			t.Fatal(err)
		}
	}

	fullPath := filepath.Join(t.TempDir(), "full.jsonl")
	writeAnalysisStream(t, fullPath, []string{
		`{"adapter_version":"test","language":"python","mode":"full","record":"lexicon","repository":"repo","schema_version":1}`,
		`{"id":"repo","kind":"repository","name":"repo","path":".","qualified_name":"repo","record":"node"}`,
		`{"id":"file-existing","kind":"file","name":"existing.py","path":"existing.py","qualified_name":"existing.py","record":"node"}`,
		`{"id":"module-existing","kind":"module","name":"existing","path":"existing.py","qualified_name":"existing","record":"node"}`,
		`{"record":"edge","relation":"contains","source":"file-existing","target":"module-existing"}`,
	})
	full, err := ReadAnalysis(fullPath, "python")
	if err != nil {
		t.Fatal(err)
	}
	entry, err := store.BuildFullLanguage(full, source, "python", "sha256:config", "sha256:adapter")
	if err != nil {
		t.Fatal(err)
	}

	incrementalPath := filepath.Join(t.TempDir(), "incremental.jsonl")
	writeAnalysisStream(t, incrementalPath, []string{
		`{"adapter_version":"test","changed_files":["new.py"],"language":"python","mode":"incremental","record":"lexicon","removed_files":[],"repository":"repo","schema_version":1,"shared_complete":true}`,
		`{"id":"repo","kind":"repository","name":"repo","path":".","qualified_name":"repo","record":"node"}`,
		`{"id":"file-new","kind":"file","name":"new.py","path":"new.py","qualified_name":"new.py","record":"node"}`,
		`{"id":"module-new","kind":"module","name":"new","path":"new.py","qualified_name":"new","record":"node"}`,
		`{"record":"edge","relation":"contains","source":"file-new","target":"module-new"}`,
	})
	incremental, err := ReadAnalysis(incrementalPath, "python")
	if err != nil {
		t.Fatal(err)
	}
	updated, err := store.BuildIncrementalLanguage(
		entry, incremental, source, "sha256:config", "sha256:adapter",
		[]string{"new.py"}, []string{}, true,
	)
	if err != nil {
		t.Fatal(err)
	}
	shared, err := store.LoadObject(updated.SharedObjectID)
	if err != nil {
		t.Fatal(err)
	}
	ids := map[string]bool{}
	for _, raw := range shared.Records {
		var record struct {
			Record string `json:"record"`
			ID     string `json:"id"`
		}
		if err := json.Unmarshal(raw, &record); err != nil {
			t.Fatal(err)
		}
		if record.Record == "node" {
			ids[record.ID] = true
		}
	}
	for _, id := range []string{"repo", "module-existing", "module-new"} {
		if !ids[id] {
			t.Fatalf("merged shared object missing %s", id)
		}
	}
}

func TestIncrementalLanguageRemovesSharedRelationshipsToRenamedFileNodes(t *testing.T) {
	store := Store{Root: t.TempDir()}
	source := t.TempDir()
	oldPath := "hermes_cli/plugin_capabilities.py"
	newPath := "plugin_runtime/capabilities.py"
	if err := os.MkdirAll(filepath.Join(source, "hermes_cli"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(source, filepath.FromSlash(oldPath)), []byte("value = 1\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	fullPath := filepath.Join(t.TempDir(), "full-rename.jsonl")
	writeAnalysisStream(t, fullPath, []string{
		`{"adapter_version":"test","language":"python","mode":"full","record":"lexicon","repository":"repo","schema_version":1}`,
		`{"id":"dir-hermes","kind":"directory","name":"hermes_cli","path":"hermes_cli","qualified_name":"hermes_cli","record":"node"}`,
		`{"id":"file-old","kind":"file","name":"plugin_capabilities.py","path":"hermes_cli/plugin_capabilities.py","qualified_name":"hermes_cli/plugin_capabilities.py","record":"node"}`,
		`{"id":"module-old","kind":"module","name":"plugin_capabilities","path":"hermes_cli/plugin_capabilities.py","qualified_name":"hermes_cli.plugin_capabilities","record":"node"}`,
		`{"id":"dependency","kind":"module","name":"shared-dependency","path":"@dependencies/python/shared-dependency","qualified_name":"shared-dependency","record":"node"}`,
		`{"record":"edge","relation":"contains","source":"dir-hermes","target":"file-old"}`,
		`{"record":"edge","relation":"contains","source":"file-old","target":"module-old"}`,
		`{"record":"edge","relation":"depends-on","source":"module-old","target":"dependency"}`,
	})
	full, err := ReadAnalysis(fullPath, "python")
	if err != nil {
		t.Fatal(err)
	}
	entry, err := store.BuildFullLanguage(full, source, "python", "sha256:config", "sha256:adapter")
	if err != nil {
		t.Fatal(err)
	}

	if err := os.Remove(filepath.Join(source, filepath.FromSlash(oldPath))); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(source, "plugin_runtime"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(source, filepath.FromSlash(newPath)), []byte("value = 1\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	incrementalPath := filepath.Join(t.TempDir(), "incremental-rename.jsonl")
	writeAnalysisStream(t, incrementalPath, []string{
		`{"adapter_version":"test","changed_files":["plugin_runtime/capabilities.py"],"language":"python","mode":"incremental","record":"lexicon","removed_files":["hermes_cli/plugin_capabilities.py"],"repository":"repo","schema_version":1,"shared_complete":true}`,
		`{"id":"dir-plugin-runtime","kind":"directory","name":"plugin_runtime","path":"plugin_runtime","qualified_name":"plugin_runtime","record":"node"}`,
		`{"id":"file-new","kind":"file","name":"capabilities.py","path":"plugin_runtime/capabilities.py","qualified_name":"plugin_runtime/capabilities.py","record":"node"}`,
		`{"id":"module-new","kind":"module","name":"capabilities","path":"plugin_runtime/capabilities.py","qualified_name":"plugin_runtime.capabilities","record":"node"}`,
		`{"id":"dependency","kind":"module","name":"shared-dependency","path":"@dependencies/python/shared-dependency","qualified_name":"shared-dependency","record":"node"}`,
		`{"record":"edge","relation":"contains","source":"dir-plugin-runtime","target":"file-new"}`,
		`{"record":"edge","relation":"contains","source":"file-new","target":"module-new"}`,
		`{"record":"edge","relation":"depends-on","source":"module-new","target":"dependency"}`,
	})
	incremental, err := ReadAnalysis(incrementalPath, "python")
	if err != nil {
		t.Fatal(err)
	}
	updated, err := store.BuildIncrementalLanguage(
		entry, incremental, source, "sha256:config", "sha256:adapter",
		[]string{newPath}, []string{oldPath}, true,
	)
	if err != nil {
		t.Fatal(err)
	}
	shared, err := store.LoadObject(updated.SharedObjectID)
	if err != nil {
		t.Fatal(err)
	}
	for _, raw := range shared.Records {
		var record struct {
			Record string `json:"record"`
			ID     string `json:"id"`
			Source string `json:"source"`
			Target string `json:"target"`
		}
		if err := json.Unmarshal(raw, &record); err != nil {
			t.Fatal(err)
		}
		if record.ID == "module-old" || record.Source == "module-old" ||
			record.Target == "module-old" || record.Target == "file-old" {
			t.Fatalf("stale shared fact survived rename: %s", raw)
		}
	}
	if _, ok := fileEntryOptional(updated, oldPath); ok {
		t.Fatalf("removed file entry survived rename: %s", oldPath)
	}
	if _, ok := fileEntryOptional(updated, newPath); !ok {
		t.Fatalf("renamed file entry missing: %s", newPath)
	}
}

func fileEntryOptional(entry LanguageEntry, path string) (FileEntry, bool) {
	for _, file := range entry.Files {
		if file.Path == path {
			return file, true
		}
	}
	return FileEntry{}, false
}

func TestFullLanguagePreservesSharedRecordOrder(t *testing.T) {
	store := Store{Root: t.TempDir()}
	source := t.TempDir()
	if err := os.WriteFile(filepath.Join(source, "a.py"), []byte("value = 1\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "full.jsonl")
	writeAnalysisStream(t, path, []string{
		`{"adapter_version":"test","language":"python","mode":"full","record":"lexicon","repository":"repo","schema_version":1}`,
		`{"id":"a-unknown","kind":"file","name":"missing.py","owner":"missing.py","path":"missing.py","qualified_name":"missing.py","record":"node"}`,
		`{"id":"b-repository","kind":"repository","name":"repo","path":".","qualified_name":"repo","record":"node"}`,
		`{"id":"c-unknown","kind":"file","name":"other.py","owner":"other.py","path":"other.py","qualified_name":"other.py","record":"node"}`,
		`{"id":"d-owned","kind":"file","name":"a.py","owner":"a.py","path":"a.py","qualified_name":"a.py","record":"node"}`,
	})
	analysis, err := ReadAnalysis(path, "python")
	if err != nil {
		t.Fatal(err)
	}
	entry, err := store.BuildFullLanguage(analysis, source, "python", "sha256:config", "sha256:adapter")
	if err != nil {
		t.Fatal(err)
	}
	shared, err := store.LoadObject(entry.SharedObjectID)
	if err != nil {
		t.Fatal(err)
	}
	want := []string{"a-unknown", "b-repository", "c-unknown"}
	if len(shared.Records) != len(want) {
		t.Fatalf("shared records = %d, want %d", len(shared.Records), len(want))
	}
	for index, raw := range shared.Records {
		var record struct {
			ID string `json:"id"`
		}
		if err := json.Unmarshal(raw, &record); err != nil {
			t.Fatal(err)
		}
		if record.ID != want[index] {
			t.Fatalf("shared record %d = %q, want %q", index, record.ID, want[index])
		}
	}
}

func fileEntry(t *testing.T, entry LanguageEntry, path string) FileEntry {
	t.Helper()
	for _, file := range entry.Files {
		if file.Path == path {
			return file
		}
	}
	t.Fatalf("missing file entry %s", path)
	return FileEntry{}
}

func writeAnalysisStream(t *testing.T, path string, lines []string) {
	t.Helper()
	data := []byte{}
	for _, line := range lines {
		data = append(data, line...)
		data = append(data, '\n')
	}
	if err := os.WriteFile(path, data, 0o644); err != nil {
		t.Fatal(err)
	}
}
