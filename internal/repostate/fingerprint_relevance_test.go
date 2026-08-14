package repostate

import "testing"

func TestFingerprintPathCoversLexiconInputs(t *testing.T) {
	for _, path := range []string{
		"src/main.kt",
		"src/main.kts",
		"src/main.zig",
		"src/app.svelte",
		"native/source.cxx",
		"native/header.hh",
		"go.mod",
		"go.sum",
		"Cargo.lock",
		"project.godot",
		"Directory.Build.props",
		"app.csproj",
		"build.gradle",
		"setup.cfg",
		"Gemfile.lock",
		".lexiconignore",
		".gitignore",
	} {
		if !fingerprintPath(path) {
			t.Errorf("Lexicon-relevant path %q is missing from repository freshness", path)
		}
	}
	if fingerprintPath("art/logo.png") {
		t.Fatal("unrelated binary asset unexpectedly participates in repository freshness")
	}
}
