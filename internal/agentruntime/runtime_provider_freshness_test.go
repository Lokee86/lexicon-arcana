package agentruntime

import (
	"context"
	"testing"

	"github.com/Lokee86/grimoire/internal/agentquery"
	"github.com/Lokee86/grimoire/internal/repostate"
)

func TestExecuteDoesNotExposeStaleArcanaSnapshot(t *testing.T) {
	root := t.TempDir()
	var captured agentquery.Request
	_, err := Execute(context.Background(), Request{
		Request:   agentquery.Request{Mode: "search", Root: root, Query: "graph", CodeOnly: true},
		StateMode: repostate.CurrentOnly,
	}, Options{
		EnsureRepository: func(context.Context, repostate.Options) (repostate.Status, error) {
			return repostate.Status{
				Repository:              repostate.RepositoryStatus{Root: root},
				Lexicon:                 repostate.ComponentStatus{Status: "current", Snapshot: "lexicon-current"},
				Arcana:                  repostate.ComponentStatus{Status: "stale", Snapshot: "arcana-old"},
				Grimoire:                repostate.ComponentStatus{Status: "current", Snapshot: "source-current", Prepared: true},
				DeterministicQueryReady: true,
			}, nil
		},
		ExecuteQuery: func(_ context.Context, request agentquery.Request) (agentquery.Response, error) {
			captured = request
			return agentquery.Response{Schema: agentquery.SchemaVersion, Mode: "search", Snapshot: request.PreparedSnapshot}, nil
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if captured.PreparedSnapshot.Providers["lexicon"] != "lexicon-current" {
		t.Fatalf("current Lexicon snapshot was not exposed: %#v", captured.PreparedSnapshot.Providers)
	}
	if _, ok := captured.PreparedSnapshot.Providers["arcana"]; ok {
		t.Fatalf("stale Arcana snapshot was exposed to discovery: %#v", captured.PreparedSnapshot.Providers)
	}
}

func TestExecuteDoesNotExposeProvidersWhenLexiconIsStale(t *testing.T) {
	root := t.TempDir()
	var captured agentquery.Request
	_, err := Execute(context.Background(), Request{
		Request:   agentquery.Request{Mode: "search", Root: root, Query: "graph", CodeOnly: true},
		StateMode: repostate.CurrentOnly,
	}, Options{
		EnsureRepository: func(context.Context, repostate.Options) (repostate.Status, error) {
			return repostate.Status{
				Repository:              repostate.RepositoryStatus{Root: root},
				Lexicon:                 repostate.ComponentStatus{Status: "stale", Snapshot: "lexicon-old"},
				Arcana:                  repostate.ComponentStatus{Status: "current", Snapshot: "lexicon-old"},
				Grimoire:                repostate.ComponentStatus{Status: "current", Snapshot: "source-current", Prepared: true},
				DeterministicQueryReady: true,
			}, nil
		},
		ExecuteQuery: func(_ context.Context, request agentquery.Request) (agentquery.Response, error) {
			captured = request
			return agentquery.Response{Schema: agentquery.SchemaVersion, Mode: "search", Snapshot: request.PreparedSnapshot}, nil
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(captured.PreparedSnapshot.Providers) != 0 {
		t.Fatalf("stale provider snapshots were exposed to discovery: %#v", captured.PreparedSnapshot.Providers)
	}
}
