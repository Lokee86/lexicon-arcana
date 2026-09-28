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
	for _, record := range result.records {
		call, ok := record.(callObservation)
		if !ok || call.Source != "function:example.com/interface-impl:invoke" {
			continue
		}
		if call.Kind == "possible" && call.Class == "interface" {
			delete(want, call.Target)
		}
	}
	if len(want) != 0 {
		t.Fatalf("missing typed interface implementations: %#v", want)
	}
}
