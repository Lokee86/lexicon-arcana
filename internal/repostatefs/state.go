package repostatefs

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// Prepare creates a tool state directory, hides it on Windows, and ignores
// repository-local state without affecting state stored outside the repository.
func Prepare(repositoryRoot, stateDir string) error {
	state, err := filepath.Abs(stateDir)
	if err != nil {
		return fmt.Errorf("resolve state directory: %w", err)
	}
	if err := os.MkdirAll(state, 0o755); err != nil {
		return fmt.Errorf("create state directory: %w", err)
	}
	if err := markHidden(state); err != nil {
		return fmt.Errorf("hide state directory: %w", err)
	}
	if strings.TrimSpace(repositoryRoot) == "" {
		return nil
	}
	root, err := filepath.Abs(repositoryRoot)
	if err != nil {
		return fmt.Errorf("resolve repository root: %w", err)
	}

	relative, inside := relativeTo(root, state)
	if !inside {
		return nil
	}
	first := strings.Split(filepath.ToSlash(relative), "/")[0]
	if strings.HasPrefix(first, ".") {
		top := filepath.Join(root, filepath.FromSlash(first))
		if top != state {
			if err := markHidden(top); err != nil {
				return fmt.Errorf("hide state root: %w", err)
			}
		}
	}
	return ensureIgnored(root, relative)
}

func relativeTo(root, state string) (string, bool) {
	relative, err := filepath.Rel(root, state)
	if err != nil || relative == "." || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
		return "", false
	}
	return relative, true
}

func ensureIgnored(root, relative string) error {
	entry := ignoreEntry(relative)
	path := filepath.Join(root, ".gitignore")
	data, err := os.ReadFile(path)
	if err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("read .gitignore: %w", err)
	}
	for _, line := range strings.Split(strings.ReplaceAll(string(data), "\r\n", "\n"), "\n") {
		if equivalentIgnore(strings.TrimSpace(line), entry) {
			return nil
		}
	}

	newline := "\n"
	if bytes.Contains(data, []byte("\r\n")) {
		newline = "\r\n"
	}
	updated := append([]byte(nil), data...)
	if len(updated) > 0 && updated[len(updated)-1] != '\n' {
		updated = append(updated, newline...)
	}
	updated = append(updated, entry...)
	updated = append(updated, newline...)
	if err := os.WriteFile(path, updated, 0o644); err != nil {
		return fmt.Errorf("update .gitignore: %w", err)
	}
	return nil
}

func ignoreEntry(relative string) string {
	value := strings.Trim(filepath.ToSlash(relative), "/")
	parts := strings.Split(value, "/")
	if len(parts) > 1 && strings.HasPrefix(parts[0], ".") {
		value = parts[0]
	}
	return "/" + value + "/"
}

func equivalentIgnore(line, entry string) bool {
	if line == "" || strings.HasPrefix(line, "#") || strings.HasPrefix(line, "!") {
		return false
	}
	normalize := func(value string) string {
		value = strings.TrimSpace(value)
		value = strings.TrimPrefix(value, "/")
		value = strings.TrimSuffix(value, "/")
		return value
	}
	return normalize(line) == normalize(entry)
}
