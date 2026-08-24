package app

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"strings"
	"time"

	"github.com/Lokee86/grimoire/internal/embedding"
	"github.com/Lokee86/grimoire/internal/knowledge"
	"github.com/Lokee86/grimoire/internal/knowledgevector"
	"github.com/Lokee86/grimoire/internal/repostate"
	"github.com/Lokee86/grimoire/internal/repostatefs"
)

func runKnowledge(args []string, stdout, stderr io.Writer) error {
	if len(args) == 0 {
		return errors.New("expected knowledge command: index, search, inspect, or vector")
	}
	switch args[0] {
	case "index":
		return runKnowledgeIndex(args[1:], stdout, stderr)
	case "search":
		return runKnowledgeSearch(args[1:], stdout, stderr)
	case "inspect":
		return runKnowledgeInspect(args[1:], stdout, stderr)
	case "vector":
		return runKnowledgeVector(args[1:], stdout, stderr)
	default:
		return fmt.Errorf("unknown knowledge command %q", args[0])
	}
}

func resolveKnowledgeState(root, state string) (string, error) {
	if state != "" {
		return resolveState(root, state)
	}
	return knowledge.DefaultState(root)
}

func runKnowledgeIndex(args []string, stdout, stderr io.Writer) error {
	flags := flag.NewFlagSet("knowledge index", flag.ContinueOnError)
	flags.SetOutput(stderr)
	root := flags.String("root", ".", "repository root")
	state := flags.String("state", "", "knowledge state directory")
	ignoreFile := flags.String("ignore-file", "", "root-relative or absolute ignore file")
	maxFileBytes := flags.Int64("max-file-bytes", 0, "maximum knowledge file size")
	includeConfig := flags.Bool("include-config", false, "include all supported configuration files")
	var excludePaths stringListFlag
	flags.Var(&excludePaths, "exclude", "root-relative or absolute path to exclude; may be repeated")
	if err := flags.Parse(args); err != nil {
		return err
	}
	statePath, err := resolveKnowledgeState(*root, *state)
	if err != nil {
		return err
	}
	if err := repostatefs.Prepare(*root, statePath); err != nil {
		return err
	}
	var previous *knowledge.Index
	loaded, loadErr := knowledge.Load(statePath)
	if loadErr == nil {
		previous = &loaded
	} else if !errors.Is(loadErr, os.ErrNotExist) {
		return fmt.Errorf("load existing knowledge index: %w", loadErr)
	}
	built, stats, err := knowledge.Build(*root, previous, knowledge.BuildOptions{
		IgnoreFile: *ignoreFile, ExcludePaths: excludePaths, MaxFileBytes: *maxFileBytes, IncludeConfig: *includeConfig,
	})
	if err != nil {
		return err
	}
	built.SourceFingerprint, err = repostate.RepositoryFingerprint(*root)
	if err != nil {
		return fmt.Errorf("fingerprint repository for knowledge index: %w", err)
	}
	if err := knowledge.Save(statePath, built); err != nil {
		return err
	}
	return writeJSON(stdout, struct {
		State     string               `json:"state"`
		GitCommit string               `json:"git_commit,omitempty"`
		Documents int                  `json:"documents"`
		Stats     knowledge.BuildStats `json:"stats"`
	}{statePath, built.GitCommit, len(built.Documents), stats})
}

func runKnowledgeSearch(args []string, stdout, stderr io.Writer) error {
	flags := flag.NewFlagSet("knowledge search", flag.ContinueOnError)
	flags.SetOutput(stderr)
	root := flags.String("root", ".", "repository root")
	state := flags.String("state", "", "knowledge state directory")
	query := flags.String("query", "", "knowledge query")
	topK := flags.Int("top-k", 20, "maximum cited sections")
	path := flags.String("path", "", "path prefix filter")
	kind := flags.String("kind", "", "document kind filter")
	heading := flags.String("heading", "", "heading filter")
	commit := flags.String("commit", "", "commit identity filter")
	since := flags.String("since", "", "minimum commit time in RFC3339 format")
	until := flags.String("until", "", "maximum commit time in RFC3339 format")
	useVectors := flags.Bool("vectors", false, "supplement BM25 with the current documentation vector snapshot")
	endpoint := flags.String("endpoint", embedding.DefaultEndpoint, "OpenAI-compatible embeddings endpoint")
	enginePath := flags.String("engine", "", "Rust vector engine DLL")
	timeout := flags.Duration("timeout", 2*time.Minute, "knowledge search timeout")
	if err := flags.Parse(args); err != nil {
		return err
	}
	if strings.TrimSpace(*query) == "" || *topK <= 0 || *timeout <= 0 {
		return errors.New("--query, positive --top-k, and positive --timeout are required")
	}
	statePath, err := resolveKnowledgeState(*root, *state)
	if err != nil {
		return err
	}
	index, err := knowledge.Load(statePath)
	if err != nil {
		return fmt.Errorf("load knowledge index: %w", err)
	}
	options := knowledge.SearchOptions{TopK: *topK, Path: *path, Kind: knowledge.Kind(*kind), Heading: *heading, CommitID: *commit}
	if options.Since, err = parseKnowledgeTime(*since); err != nil {
		return fmt.Errorf("--since: %w", err)
	}
	if options.Until, err = parseKnowledgeTime(*until); err != nil {
		return fmt.Errorf("--until: %w", err)
	}
	if *useVectors && knowledgevector.Available(statePath) {
		options.Vector = knowledgevector.Ranker{State: statePath, Index: index, Endpoint: *endpoint, EnginePath: *enginePath}
	}
	ctx, cancel := context.WithTimeout(context.Background(), *timeout)
	defer cancel()
	response, err := knowledge.Search(ctx, index, *query, options)
	if err != nil {
		return err
	}
	return writeJSON(stdout, response)
}

func runKnowledgeInspect(args []string, stdout, stderr io.Writer) error {
	flags := flag.NewFlagSet("knowledge inspect", flag.ContinueOnError)
	flags.SetOutput(stderr)
	root := flags.String("root", ".", "repository root")
	state := flags.String("state", "", "knowledge state directory")
	path := flags.String("path", "", "document path")
	handle := flags.String("handle", "", "stable section handle")
	if err := flags.Parse(args); err != nil {
		return err
	}
	indexPath, err := resolveKnowledgeState(*root, *state)
	if err != nil {
		return err
	}
	index, err := knowledge.Load(indexPath)
	if err != nil {
		return fmt.Errorf("load knowledge index: %w", err)
	}
	value, err := knowledge.Inspect(index, *path, *handle)
	if err != nil {
		return err
	}
	return writeJSON(stdout, value)
}

func parseKnowledgeTime(value string) (time.Time, error) {
	if strings.TrimSpace(value) == "" {
		return time.Time{}, nil
	}
	parsed, err := time.Parse(time.RFC3339, value)
	if err != nil {
		return time.Time{}, fmt.Errorf("expected RFC3339 time: %w", err)
	}
	return parsed.UTC(), nil
}
