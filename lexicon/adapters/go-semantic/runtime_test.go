package main

import (
	"strings"
	"testing"
)

func TestHelperVersionMatchesPackagedVersionFile(t *testing.T) {
	if helperVersion == "" || strings.TrimSpace(helperVersion) != helperVersion {
		t.Fatalf("helper version is not canonical: %q", helperVersion)
	}
	if err := validateHelperVersion(helperVersion); err != nil {
		t.Fatal(err)
	}
	if err := validateHelperVersion(helperVersion + "-stale"); err == nil {
		t.Fatal("stale helper version was accepted")
	}
}

func TestMissingGoToolchainHasActionableDiagnostic(t *testing.T) {
	t.Setenv("PATH", "")
	err := requireGoToolchain()
	if err == nil {
		t.Fatal("missing Go toolchain was accepted")
	}
	message := err.Error()
	for _, expected := range []string{"Go executable 'go'", "PATH", "go/packages", "installed Go toolchain"} {
		if !strings.Contains(message, expected) {
			t.Fatalf("diagnostic %q does not contain %q", message, expected)
		}
	}
}
