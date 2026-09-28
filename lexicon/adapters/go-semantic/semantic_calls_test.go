package main

import (
	"path/filepath"
	"testing"
)

func TestTypedDirectCallsEmitCompilerEvidence(t *testing.T) {
	root, err := filepath.Abs(filepath.Join("..", "..", "testdata", "go_oracle", "repositories", "basic_calls"))
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

	var resolved, conversions, unresolved int
	forms := make(map[string]int)
	for _, value := range result.Observations {
		call, ok := value.(callsiteObservation)
		if !ok {
			continue
		}
		forms[call.Form]++
		if hasResolvedCall(call) {
			resolved++
			if call.Form == "conversion" {
				conversions++
			}
			continue
		}
		unresolved++
		if call.Expression != "dynamic" || call.Resolution != "missing" || call.Form != "dynamic" {
			t.Fatalf("dynamic unresolved = %#v", call)
		}
	}
	if resolved != 8 || conversions != 1 || unresolved != 1 {
		t.Fatalf(
			"calls = resolved %d conversion %d unresolved %d, want 8/1/1",
			resolved, conversions, unresolved,
		)
	}
	if forms["builtin"] != 1 || forms["conversion"] != 1 || forms["dynamic"] != 1 {
		t.Fatalf("call forms = %#v", forms)
	}

	assertCallsiteTarget(t, result.Observations,
		"function:example.com/oracle/basic:recursive",
		"function:example.com/oracle/basic:recursive", "direct")
	assertCallsiteTarget(t, result.Observations,
		"function:example.com/oracle/basic:caller",
		"function:example.com/oracle/basic/internal/sub:Function", "direct")
	assertCallsiteTarget(t, result.Observations,
		"function:example.com/oracle/basic:caller",
		"method:example.com/oracle/basic/internal/sub:Thing.Method", "direct")
	assertCallsiteTarget(t, result.Observations,
		"function:example.com/oracle/basic:caller",
		"function:fmt:Println", "direct")
	assertCallsiteTarget(t, result.Observations,
		"function:example.com/oracle/basic:caller",
		"function:go:builtins:len", "builtin")
}

func TestTypedDirectCallsEmitInterfaceDispatchEvidenceForSSAFollowup(t *testing.T) {
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
	for _, value := range result.Observations {
		call, ok := value.(callsiteObservation)
		if ok && call.SourceKey == "function:example.com/typed:invoke" &&
			call.CandidateName == "Run" {
			if call.Form != "interface" || call.Resolution != "missing" {
				t.Fatalf("interface call = %#v", call)
			}
			return
		}
	}
	t.Fatalf("missing interface call evidence: %#v", result.Observations)
}

func assertCallsiteTarget(
	t *testing.T,
	values []observation,
	source, target, form string,
) {
	t.Helper()
	for _, value := range values {
		call, ok := value.(callsiteObservation)
		if !ok || call.SourceKey != source || call.Form != form {
			continue
		}
		for _, candidate := range call.Targets {
			if candidate.SemanticKey == target {
				return
			}
		}
	}
	t.Fatalf("missing %s call %s -> %s", form, source, target)
}
