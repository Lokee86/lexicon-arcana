package scan

import (
	"bufio"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

var (
	pythonFromImport = regexp.MustCompile(`^\s*from\s+([.A-Za-z_][A-Za-z0-9_.]*)\s+import\s+(.+)$`)
	pythonImport     = regexp.MustCompile(`^\s*import\s+(.+)$`)
)

func pythonAdditionContext(sourceRoot string, addedFiles []string) ([]string, error) {
	selected := make(map[string]struct{}, len(addedFiles))
	for _, path := range addedFiles {
		path = filepath.ToSlash(path)
		if path == "" {
			continue
		}
		selected[path] = struct{}{}
		addPythonPackageInitializers(sourceRoot, path, selected)
		modules, err := pythonImportedModules(filepath.Join(sourceRoot, filepath.FromSlash(path)), path)
		if err != nil {
			return nil, err
		}
		for _, module := range modules {
			for _, candidate := range pythonModuleFiles(sourceRoot, module) {
				selected[candidate] = struct{}{}
				addPythonPackageInitializers(sourceRoot, candidate, selected)
			}
		}
	}
	result := make([]string, 0, len(selected))
	for path := range selected {
		result = append(result, path)
	}
	return uniqueSorted(result), nil
}

func pythonImportedModules(path, relative string) ([]string, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	modules := make(map[string]struct{})
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.TrimSpace(strings.SplitN(scanner.Text(), "#", 2)[0])
		if line == "" {
			continue
		}
		if match := pythonFromImport.FindStringSubmatch(line); len(match) == 3 {
			base := resolvePythonRelativeModule(relative, strings.TrimSpace(match[1]))
			if base != "" {
				modules[base] = struct{}{}
				for _, item := range strings.Split(match[2], ",") {
					name := strings.Fields(strings.TrimSpace(item))
					if len(name) == 0 || name[0] == "*" {
						continue
					}
					modules[base+"."+name[0]] = struct{}{}
				}
			}
			continue
		}
		if match := pythonImport.FindStringSubmatch(line); len(match) == 2 {
			for _, item := range strings.Split(match[1], ",") {
				fields := strings.Fields(strings.TrimSpace(item))
				if len(fields) > 0 {
					modules[fields[0]] = struct{}{}
				}
			}
		}
	}
	if err := scanner.Err(); err != nil {
		return nil, err
	}
	result := make([]string, 0, len(modules))
	for module := range modules {
		result = append(result, module)
	}
	return result, nil
}

func resolvePythonRelativeModule(relative, module string) string {
	if !strings.HasPrefix(module, ".") {
		return module
	}
	dots := 0
	for dots < len(module) && module[dots] == '.' {
		dots++
	}
	packagePath := filepath.ToSlash(filepath.Dir(relative))
	parts := strings.Split(packagePath, "/")
	for i := 1; i < dots && len(parts) > 0; i++ {
		parts = parts[:len(parts)-1]
	}
	suffix := strings.TrimPrefix(module, strings.Repeat(".", dots))
	if suffix != "" {
		parts = append(parts, strings.Split(suffix, ".")...)
	}
	return strings.Join(parts, ".")
}

func pythonModuleFiles(sourceRoot, module string) []string {
	if module == "" {
		return nil
	}
	base := filepath.Join(sourceRoot, filepath.FromSlash(strings.ReplaceAll(module, ".", "/")))
	candidates := []string{base + ".py", filepath.Join(base, "__init__.py")}
	result := make([]string, 0, 2)
	for _, candidate := range candidates {
		info, err := os.Stat(candidate)
		if err != nil || info.IsDir() {
			continue
		}
		relative, err := filepath.Rel(sourceRoot, candidate)
		if err == nil {
			result = append(result, filepath.ToSlash(relative))
		}
	}
	return result
}

func addPythonPackageInitializers(sourceRoot, path string, selected map[string]struct{}) {
	dir := filepath.Dir(filepath.FromSlash(path))
	for dir != "." && dir != "" {
		initPath := filepath.Join(sourceRoot, dir, "__init__.py")
		if info, err := os.Stat(initPath); err == nil && !info.IsDir() {
			relative, err := filepath.Rel(sourceRoot, initPath)
			if err == nil {
				selected[filepath.ToSlash(relative)] = struct{}{}
			}
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
}
