package main

import "testing"

func TestTypedInterfaceCallsUseRepositoryImplementations(t *testing.T) {
	root := t.TempDir()
	writeSemanticFile(t, root, "go.mod", "module example.com/interface-impl\n\ngo 1.22\n")
	writeSemanticFile(t, root, "main.go", `package sample

type Runner interface { Run() }
type First struct{}
type Second struct{}
func (First) Run() {}
func (Second) Run() {}
func invoke(value Runner) { value.Run() }
`)
	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           []string{"go.mod", "main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/interface-impl"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}
	want := map[string]bool{
		"method:example.com/interface-impl:First.Run":  true,
		"method:example.com/interface-impl:Second.Run": true,
	}
	for _, call := range callRecords(result.Observations) {
		if call.SourceKey != "function:example.com/interface-impl:invoke" ||
			call.Form != "interface" || !hasResolvedCall(call) {
			continue
		}
		for _, target := range call.Targets {
			delete(want, target.SemanticKey)
		}
	}
	if len(want) != 0 {
		t.Fatalf("missing typed interface implementations: %#v", want)
	}
}
