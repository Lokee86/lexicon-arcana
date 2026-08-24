package repostatefs

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPrepareCreatesAndIgnoresStateIdempotently(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, ".gitignore"), []byte("node_modules/\r\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	state := filepath.Join(root, ".warlock", "tools", "lexicon")
	if err := Prepare(root, state); err != nil {
		t.Fatal(err)
	}
	if err := Prepare(root, state); err != nil {
		t.Fatal(err)
	}
	if info, err := os.Stat(state); err != nil || !info.IsDir() {
		t.Fatalf("state directory missing: %v", err)
	}
	data, err := os.ReadFile(filepath.Join(root, ".gitignore"))
	if err != nil {
		t.Fatal(err)
	}
	if strings.Count(string(data), "/.warlock/") != 1 {
		t.Fatalf("unexpected .gitignore contents: %q", data)
	}
	if !strings.Contains(string(data), "\r\n/.warlock/\r\n") {
		t.Fatalf("expected CRLF preservation: %q", data)
	}
}

func TestPrepareCollapsesNestedTopLevelState(t *testing.T) {
	root := t.TempDir()
	state := filepath.Join(root, ".grimoire", "knowledge")
	if err := Prepare(root, state); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(filepath.Join(root, ".gitignore"))
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "/.grimoire/\n" {
		t.Fatalf("nested state should ignore owning root: %q", data)
	}
}

func TestPrepareKeepsNestedNonStateParentScoped(t *testing.T) {
	root := t.TempDir()
	state := filepath.Join(root, "docs", ".ddocs")
	if err := Prepare(root, state); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(filepath.Join(root, ".gitignore"))
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "/docs/.ddocs/\n" {
		t.Fatalf("nested state should not ignore its ordinary parent: %q", data)
	}
}

func TestPrepareDoesNotIgnoreExternalState(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	state := filepath.Join(outside, ".lexicon")
	if err := Prepare(root, state); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(root, ".gitignore")); !os.IsNotExist(err) {
		t.Fatalf("external state changed .gitignore: %v", err)
	}
}

func TestPrepareWithoutRepositoryOnlyCreatesState(t *testing.T) {
	state := filepath.Join(t.TempDir(), ".ddocs")
	if err := Prepare("", state); err != nil {
		t.Fatal(err)
	}
	if info, err := os.Stat(state); err != nil || !info.IsDir() {
		t.Fatalf("state directory missing: %v", err)
	}
}
