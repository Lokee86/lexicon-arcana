package state

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

func TestMirrorSyncAllCopiesManyRelevantFiles(t *testing.T) {
	source := t.TempDir()
	mirrorRoot := t.TempDir()
	for index := 0; index < 64; index++ {
		relative := filepath.Join("pkg", fmt.Sprintf("file_%02d.go", index))
		path := filepath.Join(source, relative)
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		content := []byte(fmt.Sprintf("package pkg\nconst Value%d = %d\n", index, index))
		if err := os.WriteFile(path, content, 0o644); err != nil {
			t.Fatal(err)
		}
	}

	if err := (Mirror{Root: mirrorRoot}).SyncAll(source); err != nil {
		t.Fatal(err)
	}
	for index := 0; index < 64; index++ {
		relative := filepath.Join("pkg", fmt.Sprintf("file_%02d.go", index))
		expected, err := os.ReadFile(filepath.Join(source, relative))
		if err != nil {
			t.Fatal(err)
		}
		actual, err := os.ReadFile(filepath.Join(mirrorRoot, relative))
		if err != nil {
			t.Fatal(err)
		}
		if string(actual) != string(expected) {
			t.Fatalf("mirror contents differ for %s", relative)
		}
	}
}
