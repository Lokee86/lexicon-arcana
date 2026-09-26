package main

import (
	"go/types"
	"os"
	"path/filepath"
	"sort"
	"testing"
)

func TestSemanticIndexPreservesCanonicalTargetsAndMethodSets(t *testing.T) {
	root := t.TempDir()
	writeSemanticFile(t, root, "go.mod", "module example.com/typed\n\ngo 1.22\n")
	writeSemanticFile(t, root, "main.go", `package typed

type Runner interface { Run() }
type Value struct{}
func (Value) Run() {}
type Pointer struct{}
func (*Pointer) Run() {}
type Alias = Value

func Generic[T any](value T) T { return value }
`)
	writeSemanticFile(t, root, "external_test.go", `package typed_test

import "testing"

func TestExternal(t *testing.T) {}
`)

	index, diagnostics := loadSemanticIndex(semanticRequest(root,
		"go.mod", "main.go", "external_test.go"))
	if len(diagnostics) != 0 {
		t.Fatalf("diagnostics = %#v", diagnostics)
	}

	assertTarget(t, index, "function:example.com/typed:Generic",
		"function:example.com/typed:Generic", "main.go")
	assertTarget(t, index, "method:example.com/typed:Value.Run",
		"method:example.com/typed:Value.Run", "main.go")
	assertTarget(t, index, "method:example.com/typed:*Pointer.Run",
		"method:example.com/typed:*Pointer.Run", "main.go")
	assertTarget(t, index, "function:example.com/typed:TestExternal",
		"test:example.com/typed:TestExternal", "external_test.go")

	runner := requireType(t, index, "type:example.com/typed:Runner")
	if runner.Interface == nil || runner.Interface.NumMethods() != 1 {
		t.Fatalf("Runner interface = %#v", runner.Interface)
	}
	runnerMethods := methodSetIdentitiesForTest(index, types.NewMethodSet(runner.Named))
	if !containsString(runnerMethods, "interface-method:example.com/typed:Runner.Run") {
		t.Fatalf("Runner methods = %#v", runnerMethods)
	}

	value := requireType(t, index, "type:example.com/typed:Value")
	valueMethods := methodSetIdentitiesForTest(index, types.NewMethodSet(value.Named))
	valuePointerMethods := methodSetIdentitiesForTest(index, types.NewMethodSet(types.NewPointer(value.Named)))
	if !containsString(valueMethods, "method:example.com/typed:Value.Run") ||
		!containsString(valuePointerMethods, "method:example.com/typed:Value.Run") {
		t.Fatalf("Value method sets = %#v / %#v", valueMethods, valuePointerMethods)
	}
	pointer := requireType(t, index, "type:example.com/typed:Pointer")
	pointerMethods := methodSetIdentitiesForTest(index, types.NewMethodSet(pointer.Named))
	pointerPointerMethods := methodSetIdentitiesForTest(index, types.NewMethodSet(types.NewPointer(pointer.Named)))
	if containsString(pointerMethods, "method:example.com/typed:*Pointer.Run") ||
		!containsString(pointerPointerMethods, "method:example.com/typed:*Pointer.Run") {
		t.Fatalf("Pointer method sets = %#v / %#v", pointerMethods, pointerPointerMethods)
	}
	if _, exists := index.typesByID["type:example.com/typed:Alias"]; exists {
		t.Fatal("type alias should resolve to its named origin")
	}
}

func TestSemanticIndexReturnsPackageErrorsAsDiagnostics(t *testing.T) {
	root := t.TempDir()
	writeSemanticFile(t, root, "go.mod", "module example.com/broken\n\ngo 1.22\n")
	writeSemanticFile(t, root, "bad.go", `package broken
var _ string = 1
`)

	result, err := scanStructural(semanticRequest(root, "go.mod", "bad.go"))
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, record := range result.Records {
		value, ok := record.(diagnostic)
		if ok && value.Code == "go-package" && value.Severity == "error" {
			found = true
		}
	}
	if !found {
		t.Fatalf("missing structured package diagnostic: %#v", result.Records)
	}
}

func semanticRequest(root string, files ...string) request {
	return request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           files,
		Modules:         []module{{Root: ".", Path: modulePathForTest(files)}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	}
}

func modulePathForTest(files []string) string {
	for _, file := range files {
		if file == "bad.go" {
			return "example.com/broken"
		}
	}
	return "example.com/typed"
}

func assertTarget(t *testing.T, index *semanticIndex, semanticID, identity, owner string) {
	t.Helper()
	for _, target := range index.targetsByID[semanticID] {
		if target.Identity == identity && target.Owner == owner {
			return
		}
	}
	t.Fatalf("missing target %q -> %q in %#v", semanticID, identity, index.targetsByID[semanticID])
}

func requireType(t *testing.T, index *semanticIndex, identity string) typedType {
	t.Helper()
	value, exists := index.typesByID[identity]
	if !exists {
		t.Fatalf("missing type %q", identity)
	}
	return value
}

func methodSetIdentitiesForTest(index *semanticIndex, set *types.MethodSet) []string {
	result := make([]string, 0, set.Len())
	for position := 0; position < set.Len(); position++ {
		function, ok := set.At(position).Obj().(*types.Func)
		if !ok {
			continue
		}
		identity := semanticFunctionIdentity(index.request.Modules, function)
		if target, exists := index.targetsByObject[function]; exists {
			identity = target.Identity
		}
		result = append(result, identity)
	}
	sort.Strings(result)
	if len(result) < 2 {
		return result
	}
	unique := result[:1]
	for _, value := range result[1:] {
		if value != unique[len(unique)-1] {
			unique = append(unique, value)
		}
	}
	return unique
}

func containsString(values []string, expected string) bool {
	for _, value := range values {
		if value == expected {
			return true
		}
	}
	return false
}

func writeSemanticFile(t *testing.T, root, name, content string) {
	t.Helper()
	if err := os.WriteFile(filepath.Join(root, name), []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}
