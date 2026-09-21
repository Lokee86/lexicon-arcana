package adapters

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestDefinitionsDescribeExistingAdapters(t *testing.T) {
	want := []Definition{
		{Language: "c-family", Directory: "c-family", Extensions: []string{".c", ".cc", ".cp", ".cpp", ".cxx", ".c++", ".h", ".hh", ".hpp", ".hxx", ".h++", ".inc", ".inl", ".ipp", ".tpp"}, ConfigFiles: []string{"compile_commands.json", "CMakeLists.txt"}},
		{Language: "gdscript", Directory: "gdscript", Extensions: []string{".gd"}, ConfigFiles: []string{"project.godot"}},
		{Language: "go", Directory: "go", Extensions: []string{".go"}, ConfigFiles: []string{"go.mod", "go.sum"}, PartitionedExecution: true},
		{Language: "csharp", Directory: "csharp", Extensions: []string{".cs"}, ConfigFiles: []string{".sln", ".csproj", "Directory.Build.props", "Directory.Build.targets", "global.json"}},
		{Language: "java", Directory: "java", Extensions: []string{".java"}, ConfigFiles: []string{"pom.xml", "build.gradle", "settings.gradle", "gradlew", "gradlew.bat", "mvnw", "mvnw.cmd"}},
		{Language: "kotlin", Directory: "kotlin", Extensions: []string{".kt", ".kts"}, ConfigFiles: []string{"build.gradle.kts", "settings.gradle.kts"}},
		{Language: "lotusscript", Directory: "lotusscript", Extensions: []string{".ls", ".lsa", ".lsdb", ".lss"}},
		{Language: "python", Directory: "python", Extensions: []string{".py"}, ConfigFiles: []string{"pyproject.toml", "setup.cfg", "requirements.txt"}, PartitionedExecution: true, StreamingOutput: true},
		{Language: "ruby", Directory: "ruby", Extensions: []string{".rb", ".gemspec"}, ConfigFiles: []string{"Gemfile", "Gemfile.lock"}},
		{Language: "rust", Directory: "rust", Extensions: []string{".rs"}, ConfigFiles: []string{"Cargo.toml", "Cargo.lock"}},
		{Language: "typescript", Directory: "typescript", Extensions: []string{".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs", ".svelte"}, ConfigFiles: []string{"package.json", "package-lock.json", "tsconfig.json", "jsconfig.json"}},
		{Language: "generic", Directory: "generic"},
	}

	if got := Definitions(); !reflect.DeepEqual(got, want) {
		t.Fatalf("Definitions() = %#v, want %#v", got, want)
	}
}

func TestDefinitionsAreCopied(t *testing.T) {
	definitions := Definitions()
	definitions[0].Extensions[0] = ".changed"
	definitions[0].ConfigFiles[0] = "changed"

	got, ok := Lookup("gdscript")
	if !ok {
		t.Fatal("Lookup(gdscript) did not find a definition")
	}
	if got.Extensions[0] != ".gd" || got.ConfigFiles[0] != "project.godot" {
		t.Fatalf("registry was mutated through Definitions(): %#v", got)
	}
}

func TestFingerprintIsDeterministicAndScoped(t *testing.T) {
	root := t.TempDir()
	writeAdapterFile(t, root, "python", "z.py", "z = 1\n")
	writeAdapterFile(t, root, "python", "nested/a.py", "a = 1\n")
	writeAdapterFile(t, root, "go", "main.go", "package main\n")

	first, err := Fingerprint(root, "python")
	if err != nil {
		t.Fatal(err)
	}
	second, err := Fingerprint(root, "python")
	if err != nil {
		t.Fatal(err)
	}
	if first != second {
		t.Fatalf("Fingerprint changed between identical runs: %q != %q", first, second)
	}

	writeAdapterFile(t, root, "go", "main.go", "package main\n\nfunc main() {}\n")
	third, err := Fingerprint(root, "python")
	if err != nil {
		t.Fatal(err)
	}
	if third != first {
		t.Fatalf("Python fingerprint changed for a Go adapter change: %q != %q", third, first)
	}

	writeAdapterFile(t, root, "python", "z.py", "z = 2\n")
	changed, err := Fingerprint(root, "python")
	if err != nil {
		t.Fatal(err)
	}
	if changed == first {
		t.Fatal("Python fingerprint did not change when an adapter file changed")
	}
}

func TestFingerprintIgnoresGeneratedCacheVCSAndStateDirectories(t *testing.T) {
	root := t.TempDir()
	writeAdapterFile(t, root, "python", "adapter.py", "value = 1\n")
	ignored := []string{
		".git", ".worktrees", ".workingtrees", ".lexicon", "node_modules", "target",
		"dist", "build", "__pycache__", ".pytest_cache", "vendor", ".venv",
	}
	for _, directory := range ignored {
		writeAdapterFile(t, root, "python", filepath.Join(directory, "generated.txt"), "one\n")
	}

	first, err := Fingerprint(root, "python")
	if err != nil {
		t.Fatal(err)
	}
	for _, directory := range ignored {
		writeAdapterFile(t, root, "python", filepath.Join(directory, "generated.txt"), "two\n")
	}
	second, err := Fingerprint(root, "python")
	if err != nil {
		t.Fatal(err)
	}
	if second != first {
		t.Fatal("fingerprint changed after ignored directory contents changed")
	}

	writeAdapterFile(t, root, "python", "adapter.py", "value = 2\n")
	third, err := Fingerprint(root, "python")
	if err != nil {
		t.Fatal(err)
	}
	if third == first {
		t.Fatal("fingerprint did not change after an adapter source changed")
	}
}

func TestFingerprintIgnoresTestOnlyFiles(t *testing.T) {
	root := t.TempDir()
	writeAdapterFile(t, root, "python", "lexicon_python/adapter.py", "value = 1\n")
	writeAdapterFile(t, root, "python", "tests/test_adapter.py", "one\n")
	writeAdapterFile(t, root, "python", "lexicon_python/helper_test.py", "one\n")
	writeAdapterFile(t, root, "python", "lexicon_python/test_helper.py", "one\n")

	first, err := Fingerprint(root, "python")
	if err != nil {
		t.Fatal(err)
	}

	writeAdapterFile(t, root, "python", "tests/test_adapter.py", "two\n")
	writeAdapterFile(t, root, "python", "lexicon_python/helper_test.py", "two\n")
	writeAdapterFile(t, root, "python", "lexicon_python/test_helper.py", "two\n")
	second, err := Fingerprint(root, "python")
	if err != nil {
		t.Fatal(err)
	}
	if second != first {
		t.Fatal("fingerprint changed after test-only files changed")
	}

	writeAdapterFile(t, root, "python", "lexicon_python/adapter.py", "value = 2\n")
	third, err := Fingerprint(root, "python")
	if err != nil {
		t.Fatal(err)
	}
	if third == first {
		t.Fatal("fingerprint did not change after runtime adapter source changed")
	}
}

func TestFingerprintIncludesSchemaAndConfigVersions(t *testing.T) {
	root := t.TempDir()
	writeAdapterFile(t, root, "ruby", "adapter.rb", "puts 'ok'\n")

	base, err := FingerprintWithVersions(root, "ruby", 1, 1)
	if err != nil {
		t.Fatal(err)
	}
	for _, test := range []struct {
		name          string
		schemaVersion int
		configVersion int
	}{
		{name: "schema", schemaVersion: 2, configVersion: 1},
		{name: "config", schemaVersion: 1, configVersion: 2},
	} {
		t.Run(test.name, func(t *testing.T) {
			got, err := FingerprintWithVersions(root, "ruby", test.schemaVersion, test.configVersion)
			if err != nil {
				t.Fatal(err)
			}
			if got == base {
				t.Fatalf("fingerprint did not include %s version", test.name)
			}
		})
	}
}

func TestFingerprintRejectsUnknownOrMissingAdapter(t *testing.T) {
	root := t.TempDir()
	if _, err := Fingerprint(root, "unknown-language"); err == nil {
		t.Fatal("Fingerprint accepted an unknown language")
	}
	if _, err := Fingerprint(root, "go"); err == nil {
		t.Fatal("Fingerprint accepted a missing adapter directory")
	}
}

func writeAdapterFile(t *testing.T, root, language, relative, contents string) {
	t.Helper()
	path := filepath.Join(root, language, filepath.FromSlash(relative))
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(contents), 0o644); err != nil {
		t.Fatal(err)
	}
}
