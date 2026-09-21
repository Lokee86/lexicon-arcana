package scan

import (
	"context"
	"errors"
	"io"
	"strings"
	"testing"

	"github.com/Lokee86/lexicon/internal/adapters"
)

type streamOnlyAnalyzer struct {
	runCalled    bool
	streamCalled bool
}

func (a *streamOnlyAnalyzer) Run(context.Context, adapters.Request) error {
	a.runCalled = true
	return errors.New("file-backed analyzer path should not run")
}

func (a *streamOnlyAnalyzer) RunStream(
	_ context.Context,
	request adapters.Request,
	consume func(io.Reader) error,
) error {
	a.streamCalled = true
	payload := `{"adapter_version":"test","language":"` + request.Language +
		`","mode":"full","record":"lexicon","repository":"repo","schema_version":1}` + "\n" +
		`{"id":"repo","kind":"repository","name":"repo","path":".","qualified_name":"repo","record":"node"}` + "\n"
	return consume(strings.NewReader(payload))
}

func TestRunAnalysisPrefersStreamingAnalyzer(t *testing.T) {
	analyzer := &streamOnlyAnalyzer{}
	scanner := &Scanner{Analyzer: analyzer}
	analysis, err := scanner.runAnalysis(context.Background(), adapters.Request{
		Language: "python", Output: "unused.jsonl",
	})
	if err != nil {
		t.Fatal(err)
	}
	if !analyzer.streamCalled || analyzer.runCalled {
		t.Fatalf("stream=%t run=%t", analyzer.streamCalled, analyzer.runCalled)
	}
	if analysis.Header.Language != "python" || analysis.Header.Repository != "repo" {
		t.Fatalf("unexpected header: %#v", analysis.Header)
	}
}

func TestRunAnalysisKeepsFilePathForNonStreamingLanguage(t *testing.T) {
	analyzer := &streamOnlyAnalyzer{}
	scanner := &Scanner{Analyzer: analyzer}
	_, err := scanner.runAnalysis(context.Background(), adapters.Request{
		Language: "go", Output: "unused.jsonl",
	})
	if err == nil || err.Error() != "file-backed analyzer path should not run" {
		t.Fatalf("error = %v", err)
	}
	if analyzer.streamCalled || !analyzer.runCalled {
		t.Fatalf("stream=%t run=%t", analyzer.streamCalled, analyzer.runCalled)
	}
}
