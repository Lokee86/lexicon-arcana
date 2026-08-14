package repostate

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestArcanaIsStaleWhenLexiconIsStale(t *testing.T) {
	root := t.TempDir()
	writeSource(t, root, "package main\n")
	id := testID('a')
	writeLexicon(t, root, id)
	writeArcana(t, root, id)
	if err := os.WriteFile(filepath.Join(root, ".lexicon", ".repostate.json"), []byte(`{"source_fingerprint":"sha256:stale"}`), 0o644); err != nil {
		t.Fatal(err)
	}

	location, err := normalize(Options{Root: root})
	if err != nil {
		t.Fatal(err)
	}
	fingerprint, err := sourceFingerprint(root)
	if err != nil {
		t.Fatal(err)
	}
	status, err := inspectWithFingerprint(context.Background(), location, fingerprint)
	if err != nil {
		t.Fatal(err)
	}
	if status.Lexicon.Status != "stale" {
		t.Fatalf("Lexicon status = %+v", status.Lexicon)
	}
	if status.Arcana.Status != "stale" || status.Arcana.Expected != "" {
		t.Fatalf("Arcana was treated as aligned to stale Lexicon: %+v", status.Arcana)
	}
	if len(status.Arcana.StaleReasons) == 0 || !strings.Contains(status.Arcana.StaleReasons[0], "Lexicon snapshot is unavailable") {
		t.Fatalf("Arcana stale reason = %+v", status.Arcana.StaleReasons)
	}
}
