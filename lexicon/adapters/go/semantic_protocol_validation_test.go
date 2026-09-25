package main

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestGoSemanticProtocolRejectsUnsupportedVersion(t *testing.T) {
	request := validGoSemanticRequest(t)
	request.ProtocolVersion++
	encoded := marshalGoSemanticRequestUnchecked(t, request)
	if _, err := decodeGoSemanticRequest(encoded); err == nil || !strings.Contains(err.Error(), "unsupported") {
		t.Fatalf("unsupported version error = %v", err)
	}
}

func TestGoSemanticProtocolRejectsMalformedRepositoryPaths(t *testing.T) {
	request := validGoSemanticRequest(t)
	request.Files = []string{"../escape.go"}
	encoded := marshalGoSemanticRequestUnchecked(t, request)
	if _, err := decodeGoSemanticRequest(encoded); err == nil || !strings.Contains(err.Error(), "repository-relative") {
		t.Fatalf("malformed file path error = %v", err)
	}

	request = validGoSemanticRequest(t)
	request.RepositoryRoot = request.RepositoryRoot + string(filepath.Separator) + "."
	encoded = marshalGoSemanticRequestUnchecked(t, request)
	if _, err := decodeGoSemanticRequest(encoded); err == nil || !strings.Contains(err.Error(), "absolute cleaned") {
		t.Fatalf("unclean repository root error = %v", err)
	}
}

func TestGoSemanticProtocolRejectsUnknownRecordKind(t *testing.T) {
	payload := []byte(`{"protocol_version":1,"records":[{"record":"mystery","owner":"main.go"}]}`)
	if _, err := decodeGoSemanticResponse(payload); err == nil || !strings.Contains(err.Error(), "unknown record kind") {
		t.Fatalf("unknown record error = %v", err)
	}
}

func TestGoSemanticProtocolRejectsIncompleteSpan(t *testing.T) {
	payload := []byte(`{
		"protocol_version":1,
		"records":[{
			"record":"declaration",
			"identity":"function:example.com/demo:Run",
			"kind":"function",
			"name":"Run",
			"owner":"main.go",
			"span":{"start_line":1,"start_column":1,"end_line":1,"end_column":0}
		}]
	}`)
	if _, err := decodeGoSemanticResponse(payload); err == nil || !strings.Contains(err.Error(), "span is incomplete") {
		t.Fatalf("incomplete span error = %v", err)
	}
}

func TestGoSemanticProtocolRejectsUnknownFields(t *testing.T) {
	request := validGoSemanticRequest(t)
	encoded := marshalGoSemanticRequestUnchecked(t, request)
	encoded = []byte(strings.Replace(string(encoded), `"files":`, `"unexpected":true,"files":`, 1))
	if _, err := decodeGoSemanticRequest(encoded); err == nil || !strings.Contains(err.Error(), "unknown field") {
		t.Fatalf("unknown field error = %v", err)
	}
}

func TestGoSemanticProtocolRejectsFactsIDs(t *testing.T) {
	response := goSemanticResponse{
		ProtocolVersion: goSemanticProtocolVersion,
		Records: []goSemanticRecord{{
			Kind: goSemanticRecordDeclaration,
			Declaration: &goSemanticDeclaration{
				Identity: "sha256:deadbeef",
				Kind:     goSemanticDeclarationFunction,
				Name:     "Run",
				Owner:    "main.go",
				Span:     goSemanticSpan{StartLine: 1, StartColumn: 1, EndLine: 1, EndColumn: 4},
			},
		}},
	}
	if _, err := encodeGoSemanticResponse(response); err == nil || !strings.Contains(err.Error(), "invalid semantic identity") {
		t.Fatalf("facts ID error = %v", err)
	}
}

func validGoSemanticRequest(t *testing.T) goSemanticRequest {
	t.Helper()
	return goSemanticRequest{
		ProtocolVersion: goSemanticProtocolVersion,
		RepositoryRoot:  filepath.Clean(t.TempDir()),
		Files:           []string{"go.mod", "main.go"},
		Modules:         []goSemanticModule{{Root: ".", Path: "example.com/demo"}},
		Execution:       goSemanticExecution{Workers: 1, Shards: 1, MergeFanIn: 2},
	}
}

func marshalGoSemanticRequestUnchecked(t *testing.T, request goSemanticRequest) []byte {
	t.Helper()
	raw, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	return raw
}
