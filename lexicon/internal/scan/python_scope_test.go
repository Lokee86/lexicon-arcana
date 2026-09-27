package scan

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestPythonChangedContextIncludesLocalImportsAndPackageInitializers(t *testing.T) {
	root := t.TempDir()
	files := map[string]string{
		"plugin_runtime/__init__.py":      "",
		"plugin_runtime/capabilities.py":  "from plugin_runtime.config_bridge import load_plugin_config\nfrom hermes_cli.config import load_config\n",
		"plugin_runtime/config_bridge.py": "",
		"hermes_cli/__init__.py":          "",
		"hermes_cli/config.py":            "",
	}
	for path, body := range files {
		full := filepath.Join(root, filepath.FromSlash(path))
		if err := os.MkdirAll(filepath.Dir(full), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(full, []byte(body), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	got, err := pythonChangedContext(root, []string{"plugin_runtime/capabilities.py"})
	if err != nil {
		t.Fatal(err)
	}
	want := []string{
		"hermes_cli/__init__.py",
		"hermes_cli/config.py",
		"plugin_runtime/__init__.py",
		"plugin_runtime/capabilities.py",
		"plugin_runtime/config_bridge.py",
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("context = %v, want %v", got, want)
	}
}
