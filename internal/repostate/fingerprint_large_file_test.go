package repostate

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func TestSourceFingerprintIncludesLargeEligibleFiles(t *testing.T) {
	root := t.TempDir()
	before, err := sourceFingerprint(root)
	if err != nil {
		t.Fatal(err)
	}

	content := bytes.Repeat([]byte("a"), (2<<20)+1)
	if err := os.WriteFile(filepath.Join(root, "large.go"), content, 0o644); err != nil {
		t.Fatal(err)
	}
	after, err := sourceFingerprint(root)
	if err != nil {
		t.Fatal(err)
	}
	if before == after {
		t.Fatal("large eligible source file did not change the repository fingerprint")
	}
}

func TestQuickSourceFingerprintIncludesLargeEligibleFileMetadata(t *testing.T) {
	root := t.TempDir()
	before, err := quickSourceFingerprint(root)
	if err != nil {
		t.Fatal(err)
	}

	content := bytes.Repeat([]byte("a"), (2<<20)+1)
	if err := os.WriteFile(filepath.Join(root, "large.go"), content, 0o644); err != nil {
		t.Fatal(err)
	}
	after, err := quickSourceFingerprint(root)
	if err != nil {
		t.Fatal(err)
	}
	if before == after {
		t.Fatal("large eligible source file did not change the quick repository fingerprint")
	}
}
