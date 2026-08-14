package agentruntime

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"unicode/utf8"

	"github.com/Lokee86/grimoire/internal/agentquery"
	"github.com/Lokee86/grimoire/internal/embedding"
	"github.com/Lokee86/grimoire/internal/investigation"
	"github.com/Lokee86/grimoire/internal/knowledge"
	"github.com/Lokee86/grimoire/internal/knowledgevector"
	"github.com/Lokee86/grimoire/internal/repostate"
)

func Execute(ctx context.Context, request Request, options Options) (Response, error) {
	request = normalizeRequest(request, options)
	statePath, err := resolveStatePath(request.Root, request.State)
	if err != nil {
		return Response{}, err
	}

	ensure := options.EnsureRepository
	if ensure == nil {
		ensure = repostate.Ensure
	}
	grimoireCommand := strings.TrimSpace(options.GrimoireCommand)
	if grimoireCommand == "" {
		if executable, executableErr := os.Executable(); executableErr == nil {
			grimoireCommand = executable
		} else {
			grimoireCommand = "grimoire"
		}
	}
	preparation, err := ensure(ctx, repostate.Options{
		Root: request.Root, Mode: request.StateMode,
		LexiconState: request.LexiconState, ArcanaState: request.ArcanaState,
		GrimoireState:  statePath,
		LexiconCommand: request.LexiconCmd, ArcanaCommand: request.ArcanaCmd,
		GrimoireCommand: grimoireCommand,
	})
	response := Response{Schema: SchemaVersion, Mode: request.Mode, Breadth: request.Breadth}
	if len(preparation.Actions) > 0 || len(preparation.Warnings) > 0 || preparation.Error != "" {
		copy := preparation
		response.Preparation = &copy
	}
	if err != nil {
		return response, err
	}
	if !preparation.DeterministicQueryReady {
		return response, errors.New("repository analysis state is not current; use state_mode refresh-if-needed")
	}
	request.Root = preparation.Repository.Root
	request.State = statePath
	if err := resolveSessionHandles(statePath, &request); err != nil {
		return response, err
	}

	if request.Mode == "inspect" && strings.HasPrefix(strings.TrimSpace(request.Anchor), "knowledge://") {
		request.Handles = append(request.Handles, strings.TrimSpace(request.Anchor))
		request.Anchor = ""
	}
	documentHandles, codeHandles := splitHandles(request.Handles)
	queryRequest := request.Request
	providers := make(map[string]string, 2)
	if preparation.Lexicon.Status == "current" && preparation.Lexicon.Snapshot != "" {
		providers["lexicon"] = preparation.Lexicon.Snapshot
	}
	if preparation.Lexicon.Status == "current" && preparation.Arcana.Status == "current" && preparation.Arcana.Snapshot != "" {
		providers["arcana"] = preparation.Arcana.Snapshot
	}
	queryRequest.PreparedSnapshot = agentquery.Snapshot{
		Source:    preparation.Grimoire.Snapshot,
		Providers: providers,
	}
	queryRequest.Handles = codeHandles
	queryOnlyForSnapshot := request.Mode == "inspect" && len(codeHandles) == 0 && len(documentHandles) > 0
	if queryOnlyForSnapshot {
		queryRequest.Mode = "orient"
		queryRequest.Limit = 1
	}
	queryExecutor := options.ExecuteQuery
	if queryExecutor == nil {
		queryExecutor = agentquery.Execute
	}
	queryResponse, err := queryExecutor(ctx, queryRequest)
	if err != nil {
		return response, err
	}
	if queryOnlyForSnapshot {
		queryResponse = agentquery.Response{
			Schema:   agentquery.SchemaVersion,
			Mode:     request.Mode,
			Snapshot: queryResponse.Snapshot,
		}
	}
	response.Snapshot = queryResponse.Snapshot
	response.Breadth = queryResponse.Breadth
	response.Suggestions = queryResponse.Suggestions
	response.Assessment = queryResponse.Assessment
	response.Warnings = append(response.Warnings, preparation.Warnings...)
	response.Warnings = append(response.Warnings, queryResponse.Warnings...)
	response.Coverage = append([]agentquery.LaneCoverage(nil), queryResponse.Coverage...)
	response.DeferredExpansions = append([]agentquery.DeferredExpansion(nil), queryResponse.DeferredExpansions...)
	response.Truncated = queryResponse.Truncated
	response.TruncatedLanes = append(response.TruncatedLanes, queryResponse.TruncatedLanes...)

	documentResults, documentAvailable, documentWarnings, err := retrieveDocuments(ctx, request, statePath, preparation, documentHandles)
	response.Warnings = append(response.Warnings, documentWarnings...)
	if err != nil {
		return response, err
	}
	recordRuntimeEvidenceCoverage(&queryResponse, documentResults, documentAvailable)
	response.Coverage = append([]agentquery.LaneCoverage(nil), queryResponse.Coverage...)
	response.DeferredExpansions = append([]agentquery.DeferredExpansion(nil), queryResponse.DeferredExpansions...)
	response.Truncated = queryResponse.Truncated
	response.TruncatedLanes = append([]string(nil), queryResponse.TruncatedLanes...)

	if strings.TrimSpace(request.Session) == "" {
		if !queryOnlyForSnapshot {
			copyDiscoveryResponse(&response, queryResponse)
		}
		response.DocumentMatches = documentResults
		return response, nil
	}

	ledgerSnapshot := investigation.Snapshot{
		Repository: queryResponse.Snapshot.Source,
		Providers:  queryResponse.Snapshot.Providers,
	}
	ledger, err := investigation.Open(statePath, request.Session, ledgerSnapshot)
	if errors.Is(err, investigation.ErrSessionNotFound) {
		ledger, err = investigation.Create(statePath, request.Session, ledgerSnapshot)
		if errors.Is(err, investigation.ErrSessionExists) {
			ledger, err = investigation.Open(statePath, request.Session, ledgerSnapshot)
		}
	}
	if err != nil {
		return response, fmt.Errorf("open investigation session: %w", err)
	}
	ledgerResponse := investigationResponse(queryResponse, documentResults)
	ledgerResponse, investigationTruncated, err := boundInvestigationResponse(ledger, ledgerResponse, request.Limit)
	if err != nil {
		return response, fmt.Errorf("budget investigation response: %w", err)
	}
	if investigationTruncated {
		response.Truncated = true
		response.TruncatedLanes = appendUniqueString(response.TruncatedLanes, "investigation_delta")
		response.Warnings = append(response.Warnings, "investigation delta compacted to serialized evidence budget")
	}
	delta, err := ledger.RecordResponse(ledgerResponse)
	if err != nil {
		return response, fmt.Errorf("record investigation response: %w", err)
	}
	response.Delta = &delta
	return response, nil
}

func normalizeRequest(request Request, options Options) Request {
	if strings.TrimSpace(request.Root) == "" {
		request.Root = options.DefaultRoot
	}
	if strings.TrimSpace(request.Root) == "" {
		request.Root = "."
	}
	if strings.TrimSpace(request.State) == "" {
		request.State = options.DefaultState
	}
	if request.StateMode == "" {
		request.StateMode = options.DefaultMode
	}
	if request.StateMode == "" {
		request.StateMode = repostate.RefreshIfNeeded
	}
	if request.Mode == "search" && strings.EqualFold(strings.TrimSpace(request.Breadth), "narrow") && strings.TrimSpace(request.Detail) == "" {
		request.Detail = "handles"
	}
	if request.Limit == 0 {
		switch {
		case request.Mode == "search" && strings.EqualFold(strings.TrimSpace(request.Breadth), "narrow"):
			request.Limit = 4
		case request.Mode == "trace":
			request.Limit = 8
		default:
			request.Limit = 12
		}
	}
	if strings.TrimSpace(request.LexiconCmd) == "" {
		request.LexiconCmd = resolveProviderCommand(request.Root, request.LexiconCmd, "lexicon")
	}
	if strings.TrimSpace(request.ArcanaCmd) == "" {
		request.ArcanaCmd = resolveProviderCommand(request.Root, request.ArcanaCmd, "arcana")
	}
	return request
}

func resolveStatePath(root, state string) (string, error) {
	absoluteRoot, err := filepath.Abs(root)
	if err != nil {
		return "", fmt.Errorf("resolve repository root: %w", err)
	}
	if strings.TrimSpace(state) == "" {
		return filepath.Join(absoluteRoot, ".grimoire"), nil
	}
	if filepath.IsAbs(state) {
		return filepath.Clean(state), nil
	}
	return filepath.Join(absoluteRoot, state), nil
}

func splitHandles(handles []string) (knowledgeHandles, codeHandles []string) {
	for _, handle := range handles {
		if strings.HasPrefix(handle, "knowledge://") {
			knowledgeHandles = append(knowledgeHandles, handle)
		} else {
			codeHandles = append(codeHandles, handle)
		}
	}
	return knowledgeHandles, codeHandles
}

func useDocumentVectors(request Request) bool {
	return request.UseDocumentVectors != nil && *request.UseDocumentVectors
}

func includeDocuments(request Request) bool {
	if request.CodeOnly {
		return false
	}
	if request.IncludeDocuments != nil {
		return *request.IncludeDocuments
	}
	return request.Mode == "orient" || request.Mode == "search"
}

func retrieveDocuments(ctx context.Context, request Request, statePath string, preparation repostate.Status, handles []string) ([]knowledge.Result, int, []string, error) {
	if !includeDocuments(request) && len(handles) == 0 {
		return nil, 0, nil, nil
	}
	knowledgeState := filepath.Join(statePath, "knowledge")
	current, loadErr := knowledge.Load(knowledgeState)
	rebuild := loadErr != nil || preparation.Knowledge.Status != "current"
	if rebuild {
		var previous *knowledge.Index
		if loadErr == nil {
			previous = &current
		} else if !errors.Is(loadErr, os.ErrNotExist) && !errors.Is(loadErr, knowledge.ErrIncompatibleIndex) {
			return nil, 0, nil, fmt.Errorf("load knowledge index: %w", loadErr)
		}
		built, _, err := knowledge.Build(request.Root, previous, knowledge.BuildOptions{})
		if err != nil {
			return nil, 0, nil, fmt.Errorf("build knowledge index: %w", err)
		}
		built.SourceFingerprint = preparation.Repository.SourceFingerprint
		if err := knowledge.Save(knowledgeState, built); err != nil {
			return nil, 0, nil, fmt.Errorf("save knowledge index: %w", err)
		}
		current = built
	}

	results := make([]knowledge.Result, 0)
	for _, handle := range handles {
		value, err := knowledge.Inspect(current, "", handle)
		if err != nil {
			return nil, 0, nil, err
		}
		result, ok := value.(knowledge.Result)
		if !ok {
			return nil, 0, nil, fmt.Errorf("knowledge handle %q did not resolve to a section", handle)
		}
		results = append(results, result)
	}
	explicitCount := len(results)
	if request.Mode == "inspect" && len(handles) > 0 && strings.TrimSpace(request.Query) == "" {
		return results, len(results), nil, nil
	}
	query := strings.TrimSpace(request.Query)
	if query == "" {
		query = strings.TrimSpace(request.Anchor)
	}
	if query == "" && request.Mode == "orient" {
		query = "repository architecture overview entry points design rationale"
	}
	if query == "" {
		return results, len(results), nil, nil
	}
	topK := request.Limit
	if topK <= 0 {
		if request.Mode == "trace" {
			topK = 8
		} else {
			topK = 12
		}
	}
	if topK > 200 {
		topK = 200
	}
	candidateLimit := min(200, max(topK*4, topK))
	searchOptions := knowledge.SearchOptions{TopK: candidateLimit}
	if useDocumentVectors(request) && knowledgevector.Available(knowledgeState) {
		searchOptions.Vector = knowledgevector.Ranker{State: knowledgeState, Index: current, Endpoint: embedding.DefaultEndpoint}
	}
	searched, err := knowledge.Search(ctx, current, query, searchOptions)
	if err != nil {
		return nil, 0, nil, fmt.Errorf("search knowledge: %w", err)
	}
	compacted := compactKnowledgeResults(searched.Results)
	available := uniqueKnowledgeResultCount(results, compacted)
	if len(compacted) > topK {
		compacted = compacted[:topK]
	}
	results = appendUniqueKnowledge(results, compacted...)
	results = applyKnowledgePreviews(results, explicitCount, request.Detail)
	warnings := []string(nil)
	if searched.VectorError != "" {
		warnings = append(warnings, "knowledge vector ranking unavailable: "+searched.VectorError)
	}
	return results, available, warnings, nil
}

func uniqueKnowledgeResultCount(existing, candidates []knowledge.Result) int {
	seen := make(map[string]bool, len(existing)+len(candidates))
	for _, result := range existing {
		seen[result.Handle] = true
	}
	for _, result := range candidates {
		seen[result.Handle] = true
	}
	return len(seen)
}

func compactKnowledgeResults(results []knowledge.Result) []knowledge.Result {
	const maxTextBytes = 1200
	const maxCodeLinks = 8
	const maxReasons = 4
	compacted := append([]knowledge.Result(nil), results...)
	for index := range compacted {
		result := &compacted[index]
		result.Text = compactKnowledgeText(result.Text, maxTextBytes)
		if len(result.CodeLinks) > maxCodeLinks {
			result.CodeLinks = result.CodeLinks[:maxCodeLinks]
		}
		if len(result.Reasons) > maxReasons {
			result.Reasons = result.Reasons[:maxReasons]
		}
	}
	return compacted
}

func compactKnowledgeText(text string, maxBytes int) string {
	text = strings.TrimSpace(text)
	if len(text) <= maxBytes {
		return text
	}
	cut := maxBytes
	for cut > 0 && !utf8.RuneStart(text[cut]) {
		cut--
	}
	text = strings.TrimSpace(text[:cut])
	if boundary := strings.LastIndexAny(text, "\n.!? "); boundary >= maxBytes/2 {
		text = strings.TrimSpace(text[:boundary])
	}
	return text + "…"
}

func appendUniqueKnowledge(existing []knowledge.Result, values ...knowledge.Result) []knowledge.Result {
	seen := make(map[string]bool, len(existing)+len(values))
	for _, value := range existing {
		seen[value.Handle] = true
	}
	for _, value := range values {
		if !seen[value.Handle] {
			seen[value.Handle] = true
			existing = append(existing, value)
		}
	}
	return existing
}
