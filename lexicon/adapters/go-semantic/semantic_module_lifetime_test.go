package main

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestModuleLocalSemanticLifetimesPreserveCrossModuleFacts(t *testing.T) {
	root := t.TempDir()
	writeModuleFixture(t, root, "contracts/go.mod", `module example.com/contracts

go 1.22
`)
	writeModuleFixture(t, root, "contracts/runner.go", `package contracts

type Runner interface {
	Run()
}
`)
	writeModuleFixture(t, root, "app/go.mod", `module example.com/app

go 1.22

require example.com/contracts v0.0.0
replace example.com/contracts => ../contracts
`)
	writeModuleFixture(t, root, "app/main.go", `package app

import "example.com/contracts"

type Worker struct{}

func (Worker) Run() {}

func Call(r contracts.Runner) {
	r.Run()
}
`)

	base := request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files: []string{
			"app/go.mod",
			"app/main.go",
			"contracts/go.mod",
			"contracts/runner.go",
		},
		Modules: []module{
			{Root: "app", Path: "example.com/app"},
			{Root: "contracts", Path: "example.com/contracts"},
		},
		Execution: execution{Workers: 2, Shards: 4, MergeFanIn: 2},
	}

	got, profile, err := scanStructuralProfiled(base)
	if err != nil {
		t.Fatal(err)
	}
	if profile.ProcessedModules != 2 {
		t.Fatalf("processed modules = %d, want 2", profile.ProcessedModules)
	}
	if profile.LoadedPackages <= profile.PeakLivePackages {
		t.Fatalf(
			"loaded packages = %d, peak live packages = %d; module lifetimes were not bounded",
			profile.LoadedPackages,
			profile.PeakLivePackages,
		)
	}
	if !hasRelationship(
		got.Observations,
		"type:example.com/app:Worker",
		"type:example.com/contracts:Runner",
		"implements",
	) {
		t.Fatalf("missing cross-module implements relationship: %#v", got.Observations)
	}
	if !hasSemanticCallTarget(
		callRecords(got.Observations),
		"function:example.com/app:Call",
		"method:example.com/app:Worker.Run",
	) {
		t.Fatalf("missing cross-module interface implementation call: %#v", got.Observations)
	}

	reversed := base
	reversed.Modules = []module{
		{Root: "contracts", Path: "example.com/contracts"},
		{Root: "app", Path: "example.com/app"},
	}
	reordered, err := scanStructural(reversed)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(reordered, got) {
		t.Fatal("module processing order changed compact semantic output")
	}

	contracts, diagnostics := loadSemanticModuleIndexProfiled(
		base,
		base.Modules[1],
		nil,
	)
	if len(diagnostics) != 0 {
		t.Fatalf("contracts diagnostics = %#v", diagnostics)
	}
	if _, exists := contracts.typesByID["type:example.com/app:Worker"]; exists {
		t.Fatal("contracts module index retained unrelated app type state")
	}
}

func writeModuleFixture(t *testing.T, root, relative, content string) {
	t.Helper()
	path := filepath.Join(root, filepath.FromSlash(relative))
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}
