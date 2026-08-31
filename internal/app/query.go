package app

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"strings"
	"time"

	"github.com/Lokee86/grimoire/internal/agentquery"
	"github.com/Lokee86/grimoire/internal/agentruntime"
	"github.com/Lokee86/grimoire/internal/repostate"
)

type queryListFlag []string

func (values *queryListFlag) String() string {
	return strings.Join(*values, ",")
}

func (values *queryListFlag) Set(value string) error {
	value = strings.TrimSpace(value)
	if value == "" {
		return errors.New("value must not be empty")
	}
	*values = append(*values, value)
	return nil
}

func runQuery(args []string, stdout, stderr io.Writer) error {
	positionalMode := ""
	if len(args) > 0 && !strings.HasPrefix(args[0], "-") {
		positionalMode = args[0]
		args = args[1:]
	}
	flags := flag.NewFlagSet("discovery", flag.ContinueOnError)
	flags.SetOutput(stderr)
	mode := flags.String("mode", positionalMode, "discovery mode: orient, search, trace, impact, or inspect")
	root := flags.String("root", ".", "repository root")
	state := flags.String("state", "", "Grimoire state directory")
	stateMode := flags.String("state-mode", string(repostate.RefreshIfNeeded), "current-only, refresh-if-needed, or force-refresh")
	session := flags.String("session", "", "optional investigation session name")
	query := flags.String("query", "", "literal, symbol, behavior, or documentation query")
	anchor := flags.String("anchor", "", "name, query anchor, or stable returned handle")
	target := flags.String("target", "", "optional trace target name or handle")
	limit := flags.Int("limit", 0, "maximum results; search defaults to 12 per lane or 4 combined with --breadth narrow")
	breadth := flags.String("breadth", "", "search budgeting: balanced or narrow")
	depth := flags.Int("depth", 3, "maximum graph traversal depth")
	direction := flags.String("direction", "", "graph direction: incoming, outgoing, or both")
	adjacent := flags.Int("adjacent-context", 0, "source lines adjacent to an inspected declaration")
	codeOnly := flags.Bool("code-only", false, "omit the documentation lane")
	includeDocumentsValue := true
	flags.BoolVar(&includeDocumentsValue, "include-documents", true, "include separately ranked documentation matches")
	documentVectors := flags.Bool("document-vectors", false, "use available documentation vectors in addition to BM25")
	detail := flags.String("detail", "", "response detail: handles, summary previews, or full inline evidence")
	requestJSON := flags.String("request", "", "complete "+agentquery.SchemaVersion+" JSON request object")
	lexiconFacts := flags.String("lexicon-facts", "", "explicit directory containing exported Lexicon JSONL libraries")
	lexiconState := flags.String("lexicon-state", "", "Lexicon state directory; defaults to <root>/.lexicon")
	lexiconCommand := flags.String("lexicon-command", "", "Lexicon executable override; discovered when omitted")
	arcanaState := flags.String("arcana-state", "", "Arcana state directory; defaults to <root>/.arcana")
	arcanaCommand := flags.String("arcana-command", "", "Arcana executable override; discovered when omitted")
	timeout := flags.Duration("timeout", 2*time.Minute, "complete discovery timeout")
	var handles queryListFlag
	var relations queryListFlag
	flags.Var(&handles, "handle", "stable handle to inspect; may be repeated")
	flags.Var(&relations, "relation", "graph relation filter; may be repeated")
	if err := flags.Parse(args); err != nil {
		return err
	}
	if flags.NArg() != 0 {
		return fmt.Errorf("unexpected discovery arguments: %s", strings.Join(flags.Args(), " "))
	}
	if *timeout <= 0 {
		return errors.New("positive --timeout is required")
	}

	var includeDocuments *bool
	flags.Visit(func(value *flag.Flag) {
		if value.Name == "include-documents" {
			selected := includeDocumentsValue
			includeDocuments = &selected
		}
	})

	var request agentruntime.Request
	if strings.TrimSpace(*requestJSON) != "" {
		if err := json.Unmarshal([]byte(*requestJSON), &request); err != nil {
			return fmt.Errorf("decode --request: %w", err)
		}
		if positionalMode != "" {
			request.Mode = positionalMode
		}
	} else {
		request = agentruntime.Request{
			Request: agentquery.Request{
				Schema:       agentquery.SchemaVersion,
				Mode:         *mode,
				Root:         *root,
				State:        *state,
				Query:        *query,
				Anchor:       *anchor,
				Target:       *target,
				Handles:      handles,
				Limit:        *limit,
				Depth:        *depth,
				Direction:    *direction,
				Relations:    relations,
				Adjacent:     *adjacent,
				CodeOnly:     *codeOnly,
				Detail:       *detail,
				Breadth:      *breadth,
				LexiconFacts: *lexiconFacts,
				LexiconState: *lexiconState,
				LexiconCmd:   *lexiconCommand,
				ArcanaState:  *arcanaState,
				ArcanaCmd:    *arcanaCommand,
			},
			Session:            *session,
			StateMode:          repostate.Mode(*stateMode),
			IncludeDocuments:   includeDocuments,
			UseDocumentVectors: documentVectors,
		}
	}
	if request.CodeOnly {
		include := false
		request.IncludeDocuments = &include
	}

	ctx, cancel := context.WithTimeout(context.Background(), *timeout)
	defer cancel()
	response, err := agentruntime.Execute(ctx, request, agentruntime.Options{
		EnsureRepository: ensureDiscoveryRepository,
	})
	if err != nil {
		return err
	}
	return writeJSON(stdout, response)
}
