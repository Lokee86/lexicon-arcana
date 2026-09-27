package scan

import "testing"

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
