package scan

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/Lokee86/lexicon/internal/state"
)

func TestTypeScriptPlanOwnsJavaScriptSources(t *testing.T) {
	for _, path := range []string{
		"src/app.js",
		"src/view.jsx",
		"src/config.mjs",
		"src/legacy.cjs",
		"src/app.ts",
		"src/view.tsx",
		"src/config.mts",
		"src/legacy.cts",
	} {
		if !languageOwnsSource("typescript", path) {
			t.Fatalf("expected TypeScript adapter to own %q", path)
		}
	}
	if languageOwnsSource("typescript", "src/page.astro") {
		t.Fatal("Astro files must remain outside the JavaScript/TypeScript adapter")
	}
}

func TestAddIncrementalPathAllowsPythonAddition(t *testing.T) {
	scanner := Scanner{EnabledLanguages: []string{"python", "typescript"}}
	plans := map[string]*analysisPlan{}

	scanner.addIncrementalPath(plans, "plugin_runtime/state.py", true)
	python := plans["python"]
	if python == nil || python.Full {
		t.Fatalf("python addition unexpectedly required full analysis: %#v", python)
	}
	if len(python.AddedFiles) != 1 || python.AddedFiles[0] != "plugin_runtime/state.py" {
		t.Fatalf("python added files = %v", python.AddedFiles)
	}

	scanner.addIncrementalPath(plans, "web/new.ts", true)
	typescript := plans["typescript"]
	if typescript == nil || !typescript.Full {
		t.Fatalf("typescript addition must remain conservative: %#v", typescript)
	}
}

func TestPyprojectNonAnalysisChangeDoesNotForceFull(t *testing.T) {
	scanner := testScannerWithPyproject(t,
		"[project]\nname = \"example\"\ndependencies = [\"requests\"]\n\n[tool.setuptools.packages.find]\ninclude = [\"hermes_cli\"]\n",
		"[project]\nname = \"example\"\ndependencies = [\"requests\"]\n\n[tool.setuptools.packages.find]\ninclude = [\"hermes_cli\", \"plugin_runtime\"]\n",
	)
	plans := map[string]*analysisPlan{}
	scanner.addIncrementalPath(plans, "pyproject.toml", false)
	if plan := plans["python"]; plan != nil {
		t.Fatalf("irrelevant pyproject change unexpectedly planned Python analysis: %#v", plan)
	}
}

func TestPyprojectDependencyChangeStillForcesFull(t *testing.T) {
	scanner := testScannerWithPyproject(t,
		"[project]\nname = \"example\"\ndependencies = [\"requests\"]\n",
		"[project]\nname = \"example\"\ndependencies = [\"requests\", \"httpx\"]\n",
	)
	plans := map[string]*analysisPlan{}
	scanner.addIncrementalPath(plans, "pyproject.toml", false)
	plan := plans["python"]
	if plan == nil || !plan.Full {
		t.Fatalf("dependency-bearing pyproject change must force full analysis: %#v", plan)
	}
}

func testScannerWithPyproject(t *testing.T, previous, current string) Scanner {
	t.Helper()
	stateRoot := t.TempDir()
	repository, err := state.Ensure(stateRoot)
	if err != nil {
		t.Fatal(err)
	}
	sourceRoot := filepath.Join(stateRoot, "source")
	if err := os.MkdirAll(sourceRoot, 0o755); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(sourceRoot, "pyproject.toml")
	if err := os.WriteFile(path, []byte(previous), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := repository.StageAll(); err != nil {
		t.Fatal(err)
	}
	if err := repository.CommitState(); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(current), 0o644); err != nil {
		t.Fatal(err)
	}
	return Scanner{
		StateRoot:        stateRoot,
		Git:              repository,
		EnabledLanguages: []string{"python"},
	}
}

func TestAddRenameKeepsPythonIncremental(t *testing.T) {
	scanner := Scanner{EnabledLanguages: []string{"python", "typescript"}}
	plans := map[string]*analysisPlan{}
	scanner.addRename(plans, "hermes_cli/plugin_capabilities.py", "plugin_runtime/capabilities.py")
	plan := plans["python"]
	if plan == nil || plan.Full {
		t.Fatalf("python rename unexpectedly required full analysis: %#v", plan)
	}
	if len(plan.RemovedFiles) != 1 || plan.RemovedFiles[0] != "hermes_cli/plugin_capabilities.py" {
		t.Fatalf("removed files = %v", plan.RemovedFiles)
	}
	if len(plan.AddedFiles) != 1 || plan.AddedFiles[0] != "plugin_runtime/capabilities.py" {
		t.Fatalf("added files = %v", plan.AddedFiles)
	}
}
