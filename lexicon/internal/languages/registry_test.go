package languages

import (
	"reflect"
	"testing"
)

func TestDedicatedAdaptersTakePrecedence(t *testing.T) {
	for path, want := range map[string][]string{
		"include/api.hpp":      {"c-family"},
		"src/App.svelte":       {"typescript"},
		"src/main.c":           {"c-family"},
		"src/main.cpp":         {"c-family"},
		"src/main.go":          {"go"},
		"src/Main.cs":          {"csharp"},
		"src/Main.java":        {"java"},
		"src/Main.kt":          {"kotlin"},
		"src/build.gradle.kts": {"kotlin"},
		"src/Agent.lsa":        {"lotusscript"},
		"src/Database.lsdb":    {"lotusscript"},
		"src/Library.ls":       {"lotusscript"},
		"src/Library.lss":      {"lotusscript"},
		"src/main.py":          {"python"},
	} {
		if got := ForPath(path); !reflect.DeepEqual(got, want) {
			t.Fatalf("ForPath(%s) = %v, want %v", path, got, want)
		}
	}
}

func TestGenericFallbackUsesExtensionIdentity(t *testing.T) {
	if got, want := ForPath("src/Main.scala"), []string{"generic-scala"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("ForPath(Main.scala) = %v, want %v", got, want)
	}
	definition, ok := Lookup("generic-scala")
	if !ok || definition.Directory != "generic" || !reflect.DeepEqual(definition.Extensions, []string{".scala"}) {
		t.Fatalf("generic definition = %#v, %v", definition, ok)
	}
	if !OwnsSource("generic-scala", "src/Main.scala") || OwnsSource("generic-scala", "src/main.c") {
		t.Fatal("generic extension ownership is not exact")
	}
}

func TestStreamingOutputIsExplicit(t *testing.T) {
	if !SupportsStreamingOutput("python") {
		t.Fatal("python adapter should support streaming output")
	}
	for _, language := range []string{"go", "gdscript", "generic-scala"} {
		if SupportsStreamingOutput(language) {
			t.Fatalf("%s unexpectedly supports streaming output", language)
		}
	}
}

func TestGenericFallbackExcludesNonSourceFiles(t *testing.T) {
	for _, path := range []string{"README.md", "config.json", "settings.yaml", "data.csv", "image.png"} {
		if got := ForPath(path); got != nil {
			t.Fatalf("ForPath(%s) = %v, want nil", path, got)
		}
	}
	if IsGeneric("generic-md") || IsGeneric("generic") {
		t.Fatal("unsupported generic identities must be rejected")
	}
}
