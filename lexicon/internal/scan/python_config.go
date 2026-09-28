package scan

import (
	"os"
	"path/filepath"
	"strings"
)

func (s *Scanner) configChangeRequiresFull(language, path string) bool {
	if language != "python" || filepath.ToSlash(path) != "pyproject.toml" || s.Git == nil {
		return true
	}
	previous, err := s.Git.HeadSource(path)
	if err != nil {
		return true
	}
	current, err := os.ReadFile(filepath.Join(s.StateRoot, "source", filepath.FromSlash(path)))
	if err != nil {
		return true
	}
	return pythonProjectAnalysisConfig(previous) != pythonProjectAnalysisConfig(current)
}

func pythonProjectAnalysisConfig(data []byte) string {
	text := strings.ReplaceAll(string(data), "\r\n", "\n")
	text = strings.ReplaceAll(text, "\r", "\n")
	lines := strings.Split(text, "\n")
	var result strings.Builder
	capture := false
	for _, line := range lines {
		trimmed := strings.TrimSpace(line)
		if table, ok := tomlTableHeader(trimmed); ok {
			capture = table == "project" || table == "project.optional-dependencies"
			if capture {
				result.WriteString("[")
				result.WriteString(table)
				result.WriteString("]\n")
			}
			continue
		}
		if capture || strings.HasPrefix(trimmed, "project.") {
			result.WriteString(strings.TrimRight(line, " \t"))
			result.WriteByte('\n')
		}
	}
	return result.String()
}

func tomlTableHeader(line string) (string, bool) {
	if !strings.HasPrefix(line, "[") || strings.HasPrefix(line, "[[") {
		return "", false
	}
	end := strings.IndexByte(line, ']')
	if end <= 1 {
		return "", false
	}
	rest := strings.TrimSpace(line[end+1:])
	if rest != "" && !strings.HasPrefix(rest, "#") {
		return "", false
	}
	return strings.TrimSpace(line[1:end]), true
}
