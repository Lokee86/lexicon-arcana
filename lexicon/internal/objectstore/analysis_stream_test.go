package objectstore

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestStreamedAnalysisBuildsSameObjectsAsFileAnalysis(t *testing.T) {
	lines := []string{
		`{"adapter_version":"test","language":"python","mode":"full","record":"lexicon","repository":"repo","schema_version":1}`,
		`{"id":"repo","kind":"repository","name":"repo","path":".","qualified_name":"repo","record":"node"}`,
		`{"id":"file-a","kind":"file","name":"a.py","owner":"a.py","path":"a.py","qualified_name":"a.py","record":"node"}`,
		`{"id":"fn-a","kind":"function","name":"run","owner":"a.py","path":"a.py","qualified_name":"a.run","record":"node"}`,
		`{"record":"edge","relation":"defines","source":"file-a","target":"fn-a"}`,
		`{"candidate_name":"missing","expression":"missing()","reason":"missing-target","record":"unresolved","relation":"calls","source":"fn-a"}`,
	}
	payload := strings.Join(lines, "\n") + "\n"
	source := t.TempDir()
	if err := os.WriteFile(filepath.Join(source, "a.py"), []byte("def run(): pass\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	path := filepath.Join(t.TempDir(), "facts.jsonl")
	if err := os.WriteFile(path, []byte(payload), 0o644); err != nil {
		t.Fatal(err)
	}
	fileAnalysis, err := ReadAnalysis(path, "python")
	if err != nil {
		t.Fatal(err)
	}
	streamAnalysis, err := ReadAnalysisReader(strings.NewReader(payload), "python", "test stream")
	if err != nil {
		t.Fatal(err)
	}
	if len(streamAnalysis.records) != 0 || streamAnalysis.partitions == nil {
		t.Fatal("streamed analysis retained legacy raw records")
	}

	fileEntry, err := (Store{Root: t.TempDir()}).BuildFullLanguage(
		fileAnalysis, source, "python", "sha256:config", "sha256:adapter",
	)
	if err != nil {
		t.Fatal(err)
	}
	streamEntry, err := (Store{Root: t.TempDir()}).BuildFullLanguage(
		streamAnalysis, source, "python", "sha256:config", "sha256:adapter",
	)
	if err != nil {
		t.Fatal(err)
	}
	if fileEntry.SharedObjectID != streamEntry.SharedObjectID {
		t.Fatalf("shared object differs: file=%s stream=%s", fileEntry.SharedObjectID, streamEntry.SharedObjectID)
	}
	if len(fileEntry.Files) != 1 || len(streamEntry.Files) != 1 ||
		fileEntry.Files[0].ObjectID != streamEntry.Files[0].ObjectID {
		t.Fatalf("file objects differ: file=%#v stream=%#v", fileEntry.Files, streamEntry.Files)
	}
}

func TestStreamedAnalysisRejectsHeaderOnlyOutput(t *testing.T) {
	payload := `{"adapter_version":"test","language":"python","mode":"full","record":"lexicon","repository":"repo","schema_version":1}` + "\n"
	if _, err := ReadAnalysisReader(strings.NewReader(payload), "python", "test stream"); err == nil {
		t.Fatal("expected header-only stream to be rejected")
	}
}

func TestStreamedAnalysisRejectsNodesAfterRelationships(t *testing.T) {
	payload := strings.Join([]string{
		`{"adapter_version":"test","language":"python","mode":"full","record":"lexicon","repository":"repo","schema_version":1}`,
		`{"record":"edge","relation":"calls","source":"a","target":"b"}`,
		`{"id":"a","kind":"function","name":"a","path":"a.py","qualified_name":"a.a","record":"node"}`,
	}, "\n") + "\n"
	if _, err := ReadAnalysisReader(strings.NewReader(payload), "python", "test stream"); err == nil {
		t.Fatal("expected non-canonical stream to be rejected")
	}
}
