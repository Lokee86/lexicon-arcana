package repostate

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
)

func TestEnsurePromotesCompatibilityWarningCreatedDuringRefresh(t *testing.T) {
	root := t.TempDir()
	writeSource(t, root, "package main\n")
	calls := make([]string, 0, 4)
	var mu sync.Mutex
	runner := fixtureRunner(t, root, &mu, &calls, false)

	status, err := Ensure(context.Background(), Options{
		Root: root,
		Mode: RefreshIfNeeded,
		Run: func(ctx context.Context, command ProcessCommand) error {
			if err := runner(ctx, command); err != nil {
				return err
			}
			if command.Executable == "arcana" && command.Arguments[0] == "sync" {
				id := currentLexicon(t, root)
				directory := filepath.Join(root, ".arcana", "snapshots", strings.TrimPrefix(id, "sha256:"))
				warning := "unrecognized Lexicon unresolved reason future-signal; preserving label\n"
				if err := os.WriteFile(filepath.Join(directory, "compatibility.warnings"), []byte(warning), 0o644); err != nil {
					t.Fatal(err)
				}
			}
			return nil
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(status.Arcana.Warnings) != 1 || !strings.Contains(status.Arcana.Warnings[0], "future-signal") {
		t.Fatalf("Arcana warning was not retained after refresh: %+v", status.Arcana.Warnings)
	}
	matches := 0
	for _, warning := range status.Warnings {
		if strings.Contains(warning, "Arcana compatibility warning") && strings.Contains(warning, "future-signal") {
			matches++
		}
	}
	if matches != 1 {
		t.Fatalf("promoted compatibility warning count = %d, warnings=%+v", matches, status.Warnings)
	}
}
