package repostate

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"io"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
)

// RepositoryFingerprint returns the source identity used by repository state
// and knowledge-link freshness checks.
func RepositoryFingerprint(root string) (string, error) {
	return sourceFingerprint(root)
}

func sourceFingerprint(root string) (string, error) {
	if fingerprint, ok, err := gitFingerprint(root); ok || err != nil {
		return fingerprint, err
	}
	paths, err := walkedSourcePaths(root)
	if err != nil {
		return "", err
	}
	sort.Strings(paths)
	hash := sha256.New()
	for _, relative := range paths {
		if err := hashWorkingFile(hash, root, relative, "file"); err != nil {
			return "", err
		}
	}
	return "sha256:" + hex.EncodeToString(hash.Sum(nil)), nil
}

func gitFingerprint(root string) (string, bool, error) {
	indexData, err := exec.Command("git", "-C", root, "ls-files", "-s", "-z").Output()
	if err != nil {
		return "", false, nil
	}
	hash := sha256.New()
	for _, raw := range strings.Split(string(indexData), "\x00") {
		metadata, relative, ok := strings.Cut(raw, "\t")
		if !ok || !fingerprintPath(relative) {
			continue
		}
		fields := strings.Fields(metadata)
		if len(fields) != 3 || fields[2] != "0" || strings.HasPrefix(fields[0], "120") {
			continue
		}
		_, _ = io.WriteString(hash, "index\x00"+filepath.ToSlash(relative)+"\x00"+fields[1]+"\x00")
	}

	changed, err := gitPathList(root, "diff", "--name-only", "-z")
	if err != nil {
		return "", true, err
	}
	untracked, err := gitPathList(root, "ls-files", "--others", "--exclude-standard", "-z")
	if err != nil {
		return "", true, err
	}
	for _, group := range []struct {
		name  string
		paths []string
	}{{"worktree", changed}, {"untracked", untracked}} {
		sort.Strings(group.paths)
		for _, relative := range group.paths {
			if !fingerprintPath(relative) {
				continue
			}
			if err := hashWorkingFile(hash, root, relative, group.name); err != nil {
				return "", true, err
			}
		}
	}
	return "sha256:" + hex.EncodeToString(hash.Sum(nil)), true, nil
}

func gitPathList(root string, arguments ...string) ([]string, error) {
	data, err := exec.Command("git", append([]string{"-C", root}, arguments...)...).Output()
	if err != nil {
		return nil, err
	}
	seen := make(map[string]bool)
	result := make([]string, 0)
	for _, raw := range strings.Split(string(data), "\x00") {
		relative := filepath.ToSlash(strings.TrimSpace(raw))
		if relative != "" && !seen[relative] {
			seen[relative] = true
			result = append(result, relative)
		}
	}
	return result, nil
}

func hashWorkingFile(hash io.Writer, root, relative, kind string) error {
	path := filepath.Join(root, filepath.FromSlash(relative))
	info, err := os.Lstat(path)
	if os.IsNotExist(err) {
		_, _ = io.WriteString(hash, kind+"\x00"+filepath.ToSlash(relative)+"\x00deleted\x00")
		return nil
	}
	if err != nil {
		return err
	}
	if !info.Mode().IsRegular() || info.Mode()&os.ModeSymlink != 0 {
		return nil
	}
	file, err := os.Open(path)
	if err != nil {
		return err
	}
	defer file.Close()
	fileHash := sha256.New()
	if _, err := io.Copy(fileHash, file); err != nil {
		return err
	}
	_, _ = io.WriteString(hash, kind+"\x00"+filepath.ToSlash(relative)+"\x00")
	_, _ = hash.Write(fileHash.Sum(nil))
	return nil
}

func gitSourcePaths(root string) ([]string, bool) {
	command := exec.Command("git", "-C", root, "ls-files", "-z", "--cached", "--others", "--exclude-standard")
	data, err := command.Output()
	if err != nil {
		return nil, false
	}
	seen := make(map[string]bool)
	paths := make([]string, 0, 256)
	for _, raw := range strings.Split(string(data), "\x00") {
		relative := filepath.ToSlash(strings.TrimSpace(raw))
		if relative == "" || !fingerprintPath(relative) || seen[relative] {
			continue
		}
		seen[relative] = true
		paths = append(paths, relative)
	}
	return paths, true
}

func walkedSourcePaths(root string) ([]string, error) {
	paths := make([]string, 0, 256)
	err := filepath.WalkDir(root, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if path != root && entry.IsDir() && excludedDirectory(entry.Name()) {
			return filepath.SkipDir
		}
		if path == root || entry.IsDir() || entry.Type()&os.ModeSymlink != 0 || !entry.Type().IsRegular() {
			return nil
		}
		relative, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		relative = filepath.ToSlash(relative)
		if fingerprintPath(relative) {
			paths = append(paths, relative)
		}
		return nil
	})
	return paths, err
}

func fingerprintPath(relative string) bool {
	for _, part := range strings.Split(filepath.ToSlash(relative), "/") {
		if excludedDirectory(part) {
			return false
		}
	}
	name := strings.ToLower(filepath.Base(relative))
	extension := strings.ToLower(filepath.Ext(name))
	// This is intentionally a conservative superset of Grimoire-indexed files
	// and every source/config input currently consumed by Lexicon. False positives
	// may cause an extra refresh; false negatives can leave Arcana on a stale graph.
	switch extension {
	case ".go", ".rs", ".py", ".rb", ".gemspec", ".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx", ".mts", ".cts", ".svelte",
		".java", ".kt", ".kts", ".c", ".cc", ".cp", ".cpp", ".cxx", ".c++", ".h", ".hh", ".hpp", ".hxx", ".h++", ".inc", ".inl", ".ipp", ".tpp", ".cs", ".gd",
		".ls", ".lsa", ".lsdb", ".lss",
		".md", ".txt", ".toml", ".yaml", ".yml", ".json", ".xml", ".html", ".css", ".scss",
		".mod", ".sum", ".sln", ".csproj", ".props", ".targets", ".gradle", ".cfg", ".godot",
		".asm", ".bash", ".bat", ".clj", ".cljs", ".cmd", ".cr", ".dart", ".elm", ".erl", ".ex", ".exs", ".f03", ".f90", ".f95", ".fish", ".fs", ".fsx", ".groovy", ".hs", ".jl", ".lhs", ".lua", ".m", ".ml", ".mli", ".mm", ".nim", ".nims", ".pas", ".php", ".pl", ".pm", ".proto", ".ps1", ".r", ".scala", ".sc", ".s", ".sh", ".sol", ".sql", ".swift", ".sv", ".v", ".vb", ".vbs", ".vim", ".zig":
		return true
	}
	switch name {
	case ".gitignore", ".lexiconignore", "readme", "license", "makefile", "dockerfile", "gemfile", "gemfile.lock", "rakefile",
		"gradlew", "mvnw", "cargo.lock", "directory.build.props", "directory.build.targets":
		return true
	default:
		return false
	}
}

func gitRepositoryStatus(ctx context.Context, root string) (head string, dirty, available bool) {
	command := exec.CommandContext(ctx, "git", "-C", root, "status", "--porcelain=v2", "-z", "--branch", "--untracked-files=normal", "--ignore-submodules=all")
	data, err := command.Output()
	if err != nil {
		return "", false, false
	}
	available = true
	for _, raw := range strings.Split(string(data), "\x00") {
		record := strings.TrimSpace(raw)
		if record == "" {
			continue
		}
		if value, ok := strings.CutPrefix(record, "# branch.oid "); ok {
			value = strings.TrimSpace(value)
			if value != "(initial)" {
				head = value
			}
			continue
		}
		if strings.HasPrefix(record, "# ") || excludedPorcelainRecord(record) {
			continue
		}
		dirty = true
	}
	return head, dirty, available
}

func excludedPorcelainRecord(record string) bool {
	path := ""
	switch {
	case strings.HasPrefix(record, "? "), strings.HasPrefix(record, "! "):
		path = strings.TrimSpace(record[2:])
	case strings.HasPrefix(record, "1 "):
		fields := strings.SplitN(record, " ", 9)
		if len(fields) == 9 {
			path = fields[8]
		}
	case strings.HasPrefix(record, "2 "):
		fields := strings.SplitN(record, " ", 10)
		if len(fields) == 10 {
			path = fields[9]
		}
	}
	if path == "" {
		return false
	}
	path = filepath.ToSlash(strings.Trim(strings.TrimSpace(path), `"`))
	for _, prefix := range []string{".git/", ".worktrees/", ".workingtrees/", ".lexicon/", ".arcana/", ".grimoire/", ".ddocs/", ".warlock/", ".obsidian/"} {
		if strings.HasPrefix(path, prefix) || path == strings.TrimSuffix(prefix, "/") {
			return true
		}
	}
	return false
}

func gitOutput(ctx context.Context, root string, arguments ...string) string {
	command := exec.CommandContext(ctx, "git", append([]string{"-C", root}, arguments...)...)
	data, err := command.Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(data))
}

func excludedStatusLine(line string) bool {
	value := strings.TrimSpace(line)
	if len(value) > 3 {
		value = value[3:]
	}
	value = strings.TrimSpace(strings.Trim(value, `"`))
	for _, part := range strings.Split(value, " -> ") {
		clean := filepath.ToSlash(strings.TrimSpace(part))
		for _, prefix := range []string{".git/", ".worktrees/", ".workingtrees/", ".lexicon/", ".arcana/", ".grimoire/", ".ddocs/", ".warlock/", ".obsidian/"} {
			if strings.HasPrefix(clean, prefix) || clean == strings.TrimSuffix(prefix, "/") {
				return true
			}
		}
	}
	return false
}

func excludedDirectory(name string) bool {
	switch strings.ToLower(name) {
	case ".git", ".worktrees", ".workingtrees", ".lexicon", ".arcana", ".grimoire", ".ddocs", ".warlock", ".obsidian", ".godot",
		"node_modules", "vendor", "target", "dist", "build", "coverage", ".next", ".astro", ".cache":
		return true
	default:
		return false
	}
}
