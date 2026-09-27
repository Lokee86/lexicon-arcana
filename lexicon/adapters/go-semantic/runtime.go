package main

import (
	_ "embed"
	"fmt"
	"os/exec"
)

//go:embed VERSION
var helperVersion string

func validateHelperVersion(expected string) error {
	if expected != helperVersion {
		return fmt.Errorf("Go semantic helper version mismatch: got %s, expected %s", helperVersion, expected)
	}
	return nil
}

func requireGoToolchain() error {
	if _, err := exec.LookPath("go"); err != nil {
		return fmt.Errorf("Go executable 'go' was not found on PATH; native Go semantic analysis uses go/packages and currently requires an installed Go toolchain")
	}
	return nil
}
