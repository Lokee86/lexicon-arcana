package main

import (
	"path/filepath"
	"testing"
)

func TestTypedDirectCallsPreserveLegacyBasicClassification(t *testing.T) {
	root, err := filepath.Abs(filepath.Join("..", "go", "testdata", "oracle", "basic_calls"))
	if err != nil {
		t.Fatal(err)
	}
	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files: []string{
			"go.mod",
			"main.go",
			"internal/sub/sub.go",
		},
		Modules:   []module{{Root: ".", Path: "example.com/oracle/basic"}},
		Execution: execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}

	var definite, conversions, unresolved int
	classes := make(map[string]int)
	for _, record := range result.Records {
		switch value := record.(type) {
		case callObservation:
			classes[value.Class]++
			switch value.Kind {
			case "definite":
				definite++
			case "conversion":
				conversions++
			}
		case unresolvedObservation:
			unresolved++
			classes[value.Class]++
			if value.Expression != "dynamic" || value.Reason != "dynamic-target" {
				t.Fatalf("dynamic unresolved = %#v", value)
			}
		}
	}
	if definite != 7 || conversions != 1 || unresolved != 1 {
		t.Fatalf(
			"calls = definite %d conversion %d unresolved %d, want 7/1/1",
			definite, conversions, unresolved,
		)
	}
	if classes["builtin"] != 1 || classes["conversion"] != 1 ||
		classes["external"] != 1 || classes["dynamic"] != 1 {
		t.Fatalf("call classes = %#v", classes)
	}

	assertCallObservation(t, result.Records,
		"function:example.com/oracle/basic:recursive",
		"function:example.com/oracle/basic:recursive", "internal")
	assertCallObservation(t, result.Records,
		"function:example.com/oracle/basic:caller",
		"function:example.com/oracle/basic/internal/sub:Function", "internal")
	assertCallObservation(t, result.Records,
		"function:example.com/oracle/basic:caller",
		"method:example.com/oracle/basic/internal/sub:Thing.Method", "internal")
	assertCallObservation(t, result.Records,
		"function:example.com/oracle/basic:caller",
		"function:fmt:Println", "external")
	assertCallObservation(t, result.Records,
		"function:example.com/oracle/basic:caller",
		"function:go:builtins:len", "builtin")
}

func TestTypedDirectCallsClassifyInterfaceDispatchForSSAFollowup(t *testing.T) {
	root := t.TempDir()
	writeSemanticFile(t, root, "go.mod", "module example.com/typed\n\ngo 1.22\n")
	writeSemanticFile(t, root, "main.go", `package typed

type Runner interface { Run() }

func invoke(value Runner) {
	value.Run()
}
`)

	result, err := scanStructural(semanticRequest(root, "go.mod", "main.go"))
	if err != nil {
		t.Fatal(err)
	}
	for _, record := range result.Records {
		value, ok := record.(unresolvedObservation)
		if ok && value.Source == "function:example.com/typed:invoke" &&
			value.CandidateName == "Run" {
			if value.Class != "interface" || value.Reason != "dynamic-target" {
				t.Fatalf("interface call = %#v", value)
			}
			return
		}
	}
	t.Fatalf("missing interface call classification: %#v", result.Records)
}

func assertCallObservation(
	t *testing.T,
	records []semanticRecord,
	source, target, class string,
) {
	t.Helper()
	for _, record := range records {
		value, ok := record.(callObservation)
		if ok && value.Source == source && value.Target == target && value.Class == class {
			return
		}
	}
	t.Fatalf("missing %s call %s -> %s", class, source, target)
}
