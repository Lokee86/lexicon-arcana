package agentquery

import (
	"context"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Lokee86/grimoire/internal/index"
)

func TestPreparedSnapshotOmissionDisablesProviderRediscovery(t *testing.T) {
	root, facts := queryFixture(t)
	source, err := index.Load(filepath.Join(root, ".grimoire"))
	if err != nil {
		t.Fatal(err)
	}

	response, err := Execute(context.Background(), Request{
		Schema:       SchemaVersion,
		Mode:         "search",
		Root:         root,
		Query:        "SubmitLogin",
		Limit:        10,
		LexiconFacts: facts,
		PreparedSnapshot: Snapshot{
			Source:    source.Identity(),
			Providers: map[string]string{},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(response.SymbolMatches) != 0 {
		t.Fatalf("prepared provider omission was ignored: %+v", response.SymbolMatches)
	}
	if response.Snapshot.Providers != nil {
		t.Fatalf("omitted providers were rediscovered: %+v", response.Snapshot.Providers)
	}
}

func TestPreparedSourceSnapshotMustMatchActiveIndex(t *testing.T) {
	root, _ := queryFixture(t)
	_, err := Execute(context.Background(), Request{
		Schema: SchemaVersion,
		Mode:   "search",
		Root:   root,
		Query:  "SubmitLogin",
		Limit:  10,
		PreparedSnapshot: Snapshot{
			Source: "sha256:not-the-active-source",
		},
	})
	if err == nil || !strings.Contains(err.Error(), "is not active source snapshot") {
		t.Fatalf("prepared source mismatch error = %v", err)
	}
}

func TestPreparedProviderExpectationUsesOmissionAsConstraint(t *testing.T) {
	request := Request{PreparedSnapshot: Snapshot{Source: "source", Providers: map[string]string{"lexicon": "lexicon-current"}}}
	if expected, constrained := preparedProviderExpectation(request, "lexicon"); !constrained || expected != "lexicon-current" {
		t.Fatalf("lexicon expectation = %q constrained=%v", expected, constrained)
	}
	if expected, constrained := preparedProviderExpectation(request, "arcana"); !constrained || expected != "" {
		t.Fatalf("omitted Arcana expectation = %q constrained=%v", expected, constrained)
	}
	if expected, constrained := preparedProviderExpectation(Request{}, "arcana"); constrained || expected != "" {
		t.Fatalf("direct query unexpectedly constrained provider: %q constrained=%v", expected, constrained)
	}
}
