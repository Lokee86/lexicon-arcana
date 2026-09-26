package main

import (
	"path/filepath"
	"reflect"
	"testing"
)

func pointerTo(value int) *int { return &value }

func TestGoSemanticProtocolRequestRoundTrip(t *testing.T) {
	request := goSemanticRequest{
		ProtocolVersion: goSemanticProtocolVersion,
		RepositoryRoot:  filepath.Clean(t.TempDir()),
		Files:           []string{"go.mod", "cmd/app/main.go", "internal/service/service.go"},
		Modules: []goSemanticModule{
			{Root: ".", Path: "example.com/app"},
			{Root: "tools", Path: "example.com/tools"},
		},
		Execution: goSemanticExecution{Workers: 4, Shards: 8, MergeFanIn: 4},
	}
	encoded, err := encodeGoSemanticRequest(request)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := decodeGoSemanticRequest(encoded)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(decoded, request) {
		t.Fatalf("round trip changed request:\n got: %#v\nwant: %#v", decoded, request)
	}
}

func TestGoSemanticProtocolResponseRoundTrip(t *testing.T) {
	span := goSemanticSpan{StartLine: 10, StartColumn: 2, EndLine: 10, EndColumn: 18}
	response := goSemanticResponse{
		ProtocolVersion: goSemanticProtocolVersion,
		Records: []goSemanticRecord{
			{
				Kind: goSemanticRecordDeclaration,
				Declaration: &goSemanticDeclaration{
					Identity: "method:example.com/foo:Thing.Run",
					Kind:     goSemanticDeclarationMethod,
					Name:     "Run",
					Owner:    "thing.go",
					Span:     span,
					Metadata: map[string]string{"container": "package:example.com/foo:foo"},
				},
			},
			{
				Kind: goSemanticRecordDeclaration,
				Declaration: &goSemanticDeclaration{
					Identity: "import:external:fmt",
					Kind:     goSemanticDeclarationImport,
					Name:     "fmt",
					Owner:    "thing.go",
					Span:     span,
					Metadata: map[string]string{
						"container":    "package:example.com/foo:foo",
						"import_class": "external",
						"import_path":  "fmt",
					},
				},
			},
			{
				Kind: goSemanticRecordRelationship,
				Relationship: &goSemanticRelationship{
					Source: "type:example.com/foo:Thing",
					Target: "type:example.com/foo:Runner",
					Kind:   goSemanticRelationshipImplements,
					Owner:  "thing.go",
					Span:   &span,
				},
			},
			{
				Kind: goSemanticRecordRelationship,
				Relationship: &goSemanticRelationship{
					Source:       "closure:example.com/foo:caller.go:12:3",
					Kind:         goSemanticRelationshipReferences,
					TargetName:   "value",
					CaptureIndex: pointerTo(0),
					Owner:        "caller.go",
				},
			},
			{
				Kind: goSemanticRecordCall,
				Call: &goSemanticCallObservation{
					Source: "function:example.com/foo:caller",
					Target: "method:example.com/foo:Thing.Run",
					Kind:            goSemanticCallDefinite,
					Class:           goSemanticCallClassDynamic,
					TargetName:      "Run$bound",
					TargetNamespace: "example.com/foo",
					TargetContainer: "package:example.com/foo:foo",
					Owner:           "caller.go",
					Span:            span,
				},
			},
			{
				Kind: goSemanticRecordDataflow,
				Dataflow: &goSemanticDataflowObservation{
					Source: "function:example.com/foo:caller",
					Target: "variable:example.com/foo:caller.go:10:2:value",
					Kind:   goSemanticDataflowRead,
					Owner:  "caller.go",
					Span:   span,
				},
			},
			{
				Kind: goSemanticRecordUnresolved,
				Unresolved: &goSemanticUnresolvedObservation{
					Source:             "function:example.com/foo:caller",
					Relation:           "calls",
					Expression:         "dynamic",
					CandidateNamespace: "example.com/foo",
					CandidateName:      "dynamic",
					Reason:             goSemanticUnresolvedDynamic,
					Class:              goSemanticCallClassDynamic,
					Owner:              "caller.go",
					Span:               span,
				},
			},
			{
				Kind: goSemanticRecordDiagnostic,
				Diagnostic: &goSemanticDiagnostic{
					Severity: goSemanticDiagnosticWarning,
					Code:     "go-package-error",
					Message:  "package loaded with a recoverable diagnostic",
					Owner:    "caller.go",
					Span:     &span,
				},
			},
		},
	}
	encoded, err := encodeGoSemanticResponse(response)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := decodeGoSemanticResponse(encoded)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(decoded, response) {
		t.Fatalf("round trip changed response:\n got: %#v\nwant: %#v", decoded, response)
	}
}
