package main

import "testing"

func TestExternalInterfaceMethodRemainsResolvedContract(t *testing.T) {
	root := t.TempDir()
	writeSemanticFile(t, root, "go.mod", "module example.com/external-interface\n\ngo 1.22\n")
	writeSemanticFile(t, root, "main.go", `package sample

import "io/fs"

func modTime(info fs.FileInfo) {
	_ = info.ModTime()
}
`)
	result, err := scanStructural(request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  root,
		Files:           []string{"go.mod", "main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/external-interface"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	})
	if err != nil {
		t.Fatal(err)
	}
	for _, record := range result.Records {
		call, ok := record.(callObservation)
		if !ok || call.Source != "function:example.com/external-interface:modTime" {
			continue
		}
		if call.Target == "method:io/fs:FileInfo.ModTime" && call.Class == "interface" {
			return
		}
	}
	t.Fatalf("external interface contract not resolved: %#v", callRecords(result.Records))
}
