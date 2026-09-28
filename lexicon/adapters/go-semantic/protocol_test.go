package main

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestSemanticProtocolRequestRoundTripAndValidation(t *testing.T) {
	value := request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  filepath.Clean(t.TempDir()),
		Files:           []string{"go.mod", "cmd/app/main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/app"}},
		Execution:       execution{Workers: 4, Shards: 8, MergeFanIn: 4},
	}
	raw, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := decodeRequest(raw)
	if err != nil {
		t.Fatal(err)
	}
	if decoded.ProtocolVersion != value.ProtocolVersion ||
		decoded.RepositoryRoot != value.RepositoryRoot ||
		len(decoded.Files) != len(value.Files) ||
		len(decoded.Modules) != len(value.Modules) ||
		decoded.Execution != value.Execution {
		t.Fatalf("round trip changed request: got %#v want %#v", decoded, value)
	}
}

func TestSemanticProtocolRejectsUnsupportedVersion(t *testing.T) {
	value := validProtocolRequest(t)
	value.ProtocolVersion = 1
	assertRequestErrorContains(t, value, "unsupported")
}

func TestSemanticProtocolRejectsMalformedRepositoryPaths(t *testing.T) {
	value := validProtocolRequest(t)
	value.Files = []string{"../escape.go"}
	assertRequestErrorContains(t, value, "repository-relative")

	value = validProtocolRequest(t)
	value.RepositoryRoot += string(filepath.Separator) + "."
	assertRequestErrorContains(t, value, "absolute cleaned")
}

func TestSemanticProtocolRejectsUnknownFields(t *testing.T) {
	value := validProtocolRequest(t)
	raw, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	raw = []byte(strings.Replace(string(raw), `"files":`, `"unexpected":true,"files":`, 1))
	if _, err := decodeRequest(raw); err == nil || !strings.Contains(err.Error(), "unknown field") {
		t.Fatalf("unknown field error = %v", err)
	}
}

func TestSemanticProtocolRejectsInvalidExecution(t *testing.T) {
	value := validProtocolRequest(t)
	value.Execution.MergeFanIn = 1
	assertRequestErrorContains(t, value, "invalid semantic execution")
}

func validProtocolRequest(t *testing.T) request {
	t.Helper()
	return request{
		ProtocolVersion: protocolVersion,
		RepositoryRoot:  filepath.Clean(t.TempDir()),
		Files:           []string{"go.mod", "main.go"},
		Modules:         []module{{Root: ".", Path: "example.com/demo"}},
		Execution:       execution{Workers: 1, Shards: 1, MergeFanIn: 2},
	}
}

func assertRequestErrorContains(t *testing.T, value request, expected string) {
	t.Helper()
	raw, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := decodeRequest(raw); err == nil || !strings.Contains(err.Error(), expected) {
		t.Fatalf("request error = %v, want substring %q", err, expected)
	}
}

func TestSemanticProtocolResponseEmitsObservationsNotLegacyRecords(t *testing.T) {
	value := responseFromRecords([]semanticRecord{
		callObservation{
			Record:          "call",
			Source:          "function:example.com/demo:caller",
			Target:          "function:example.com/demo:target",
			Kind:            "possible",
			Class:           "dynamic",
			TargetName:      "target",
			TargetNamespace: "example.com/demo",
			Owner:           "main.go",
			Span:            span{StartLine: 2, StartColumn: 1, EndLine: 2, EndColumn: 9},
		},
	})
	raw, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	text := string(raw)
	for _, forbidden := range []string{`"records"`, `"record"`, `"reason"`, `"class"`, `"kind":"possible"`} {
		if strings.Contains(text, forbidden) {
			t.Fatalf("legacy wire field %s leaked into %s", forbidden, text)
		}
	}
	for _, required := range []string{`"protocol_version":2`, `"observations"`, `"observation":"callsite"`, `"resolution":"resolved"`} {
		if !strings.Contains(text, required) {
			t.Fatalf("missing v2 wire field %s in %s", required, text)
		}
	}
}
