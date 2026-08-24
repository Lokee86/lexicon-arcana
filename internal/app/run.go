package app

import (
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	"github.com/Lokee86/grimoire/internal/index"
	"github.com/Lokee86/grimoire/internal/repostatefs"
)

// Version is overridden for release builds with Go's -ldflags -X option.
var Version = "0.1.0-dev"

type stringListFlag []string

func (values *stringListFlag) String() string {
	return strings.Join(*values, ",")
}

func (values *stringListFlag) Set(value string) error {
	value = strings.TrimSpace(value)
	if value == "" {
		return errors.New("exclude path must not be empty")
	}
	*values = append(*values, value)
	return nil
}

func Run(args []string, stdout, stderr io.Writer) error {
	if len(args) == 0 {
		return writeRootHelp(stdout)
	}

	switch args[0] {
	case "help", "-h", "--help":
		return writeRootHelp(stdout)
	case "index":
		return runIndex(args[1:], stdout, stderr)
	case "status":
		return runStatus(args[1:], stdout, stderr)
	case "knowledge":
		return runKnowledge(args[1:], stdout, stderr)
	case "lexicon":
		return runEngineNamespace(lexiconEngine, args[1:], stdout, stderr)
	case "arcana":
		return runEngineNamespace(arcanaEngine, args[1:], stdout, stderr)
	case "orient", "search", "trace", "impact", "inspect":
		return runQuery(args, stdout, stderr)
	case "query":
		return runQuery(args[1:], stdout, stderr)
	case "eval":
		if len(args) > 1 && args[1] == "arcana" {
			return runEvalArcana(args[2:], stdout, stderr)
		}
		if len(args) > 1 && args[1] == "knowledge" {
			return runEvalKnowledge(args[2:], stdout, stderr)
		}
		return errors.New("expected evaluation command: arcana or knowledge")
	case "model":
		return runModel(args[1:], stdout, stderr)
	case "vector":
		return runKnowledgeVector(args[1:], stdout, stderr)
	case "investigation":
		return runInvestigation(args[1:], stdout, stderr)
	case "mcp":
		return runMCP(args[1:], os.Stdin, stdout, stderr)
	case "version":
		_, err := fmt.Fprintln(stdout, Version)
		return err
	default:
		return fmt.Errorf("unknown command %q; run `grimoire help` for usage", args[0])
	}
}

func writeRootHelp(writer io.Writer) error {
	_, err := fmt.Fprint(writer, `Grimoire is a unified repository discovery interface over source, documentation, Lexicon symbols, and Arcana relationships.

Usage:
  grimoire <command> [flags]

Core workflow:
  grimoire model setup                 Install the local embedding runtime
  grimoire model start                 Start the managed embedding service
  grimoire index --root .              Prepare source and Lexicon-aligned chunks
  grimoire knowledge index --root .    Index repository rationale and documentation
  grimoire knowledge search --query .  Retrieve cited documentation with BM25 and vectors
  grimoire vector build --root .       Build or refresh documentation vectors
  grimoire eval knowledge --cases ...  Judge documentation retrieval against a frozen corpus
  grimoire eval arcana --cases ...     Compare Arcana graph retrieval with and without vectors
  grimoire orient --root .             Discover compact repository anchors
  grimoire search --root . --query ... Search independent evidence lanes
  grimoire trace --anchor <id>         Expand an exact structural handle
  grimoire inspect --handle <id>       Read exact source or documentation
  grimoire lexicon doctor --repo .     Run language-analysis diagnostics
  grimoire arcana sync                 Synchronize structural graph state
  grimoire mcp --root .                Serve the unified discovery tool over stdio

Commands:
  orient    Discover compact source and symbol anchors
  search    Search exact, source, documentation, and symbol lanes; defer graph expansion
  trace     Expand bounded structural paths from a returned handle
  impact    Find bounded incoming or outgoing dependents
  inspect   Read exact evidence for returned handles
  query     Compatibility entry point for the discovery modes
  index     Prepare repository source state
  status    Inspect or prepare repository analysis state
  knowledge Index, search, inspect, or vectorize repository knowledge
  lexicon   Run direct Lexicon analysis, adapter, snapshot, and diagnostic commands
  arcana    Run direct Arcana graph, synchronization, and structural commands
  vector    Build or inspect documentation vector state
  investigation  Create, inspect, or close an agent investigation ledger session
  mcp       Serve the unified repository discovery tool over stdio
  model     Set up and manage the embedding runtime
  eval      Run judged retrieval evaluation
  version   Print the Grimoire version
  help      Show this help

Lexicon and Arcana remain independently usable components. Grimoire uses their
repository-local state automatically during discovery and also exposes their
specialist commands under grimoire lexicon and grimoire arcana.
`)
	return err
}

func runIndex(args []string, stdout, stderr io.Writer) error {
	flags := flag.NewFlagSet("index", flag.ContinueOnError)
	flags.SetOutput(stderr)
	root := flags.String("root", ".", "repository root")
	state := flags.String("state", "", "prepared index repository path")
	ignoreFile := flags.String("ignore-file", "", "root-relative or absolute ignore file; defaults to .gitignore hierarchy")
	maxFileBytes := flags.Int64("max-file-bytes", 0, "maximum indexed file size")
	includeGenerated := flags.Bool("include-generated", false, "include generated, vendored, lock, bundled, and minified content")
	lexiconFacts := flags.String("lexicon-facts", "", "explicit Lexicon JSONL export directory for semantic source spans")
	lexiconState := flags.String("lexicon-state", "", "Lexicon state directory; defaults to <root>/.lexicon")
	lexiconCommand := flags.String("lexicon-command", "lexicon", "Lexicon executable used for immutable snapshot export")
	var excludePaths stringListFlag
	flags.Var(&excludePaths, "exclude", "root-relative or absolute path to exclude; may be repeated")
	if err := flags.Parse(args); err != nil {
		return err
	}

	statePath, err := resolveState(*root, *state)
	if err != nil {
		return err
	}
	if err := repostatefs.Prepare(*root, statePath); err != nil {
		return err
	}
	previous, err := loadOptional(statePath)
	if err != nil {
		return err
	}

	sourceSpans, lexiconSnapshot, spanErr := resolveIndexSourceSpans(
		*root, statePath, *lexiconFacts, *lexiconState, *lexiconCommand,
	)
	if spanErr != nil {
		if strings.TrimSpace(*lexiconFacts) != "" || strings.TrimSpace(*lexiconState) != "" {
			return fmt.Errorf("resolve Lexicon semantic spans: %w", spanErr)
		}
		_, _ = fmt.Fprintf(stderr, "warning: Lexicon semantic spans unavailable; using line-window chunking: %v\n", spanErr)
	}

	excluded := append([]string{statePath}, excludePaths...)
	snapshot, stats, err := index.Build(*root, previous, index.BuildOptions{
		MaxFileBytes:     *maxFileBytes,
		IgnoreFile:       *ignoreFile,
		ExcludePaths:     excluded,
		IncludeGenerated: *includeGenerated,
		SourceSpans:      sourceSpans,
	})
	if err != nil {
		return err
	}
	if err := index.Save(statePath, snapshot); err != nil {
		return err
	}

	chunking := "fallback"
	if stats.SemanticFiles > 0 {
		chunking = "lexicon"
	}
	response := struct {
		State           string           `json:"state"`
		Files           int              `json:"files"`
		Chunking        string           `json:"chunking"`
		LexiconSnapshot string           `json:"lexicon_snapshot,omitempty"`
		Stats           index.BuildStats `json:"stats"`
	}{
		State: statePath, Files: len(snapshot.Files), Chunking: chunking,
		LexiconSnapshot: lexiconSnapshot, Stats: stats,
	}
	return writeJSON(stdout, response)
}

func resolveState(root, state string) (string, error) {
	absoluteRoot, err := filepath.Abs(root)
	if err != nil {
		return "", fmt.Errorf("resolve root: %w", err)
	}
	if state == "" {
		state = strings.TrimSpace(os.Getenv("GRIMOIRE_STATE_DIR"))
		if state == "" {
			return filepath.Join(absoluteRoot, ".grimoire"), nil
		}
	}
	if filepath.IsAbs(state) {
		return filepath.Clean(state), nil
	}
	return filepath.Join(absoluteRoot, state), nil
}

func loadOptional(path string) (*index.Snapshot, error) {
	snapshot, err := index.Load(path)
	if errors.Is(err, os.ErrNotExist) {
		return nil, nil
	}
	if errors.Is(err, index.ErrIncompatibleIndex) {
		base, rebuildErr := index.RebuildBase(path)
		if rebuildErr != nil {
			return nil, fmt.Errorf("prepare index rebuild: %w", rebuildErr)
		}
		return &base, nil
	}
	if err != nil {
		return nil, fmt.Errorf("load existing index: %w", err)
	}
	return &snapshot, nil
}

func writeJSON(writer io.Writer, value any) error {
	encoder := json.NewEncoder(writer)
	encoder.SetIndent("", "  ")
	return encoder.Encode(value)
}
