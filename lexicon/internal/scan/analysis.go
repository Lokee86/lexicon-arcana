package scan

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"sync"

	"github.com/Lokee86/lexicon/internal/adapters"
	"github.com/Lokee86/lexicon/internal/config"
	"github.com/Lokee86/lexicon/internal/objectstore"
	analysisscope "github.com/Lokee86/lexicon/internal/scope"
)

func (s *Scanner) analyzeFull(
	ctx context.Context,
	manifest objectstore.Manifest,
	languages []string,
) (objectstore.Manifest, error) {
	plans := make([]analysisPlan, 0, len(languages))
	for _, language := range languages {
		plans = append(plans, analysisPlan{Language: language, Full: true, KnownPresent: true})
	}
	return s.analyzePlans(ctx, manifest, plans)
}

func (s *Scanner) analyzePlans(
	ctx context.Context,
	manifest objectstore.Manifest,
	plans []analysisPlan,
) (objectstore.Manifest, error) {
	temporary := filepath.Join(config.StateRoot(s.Repository), "tmp")
	if err := os.MkdirAll(temporary, 0o755); err != nil {
		return objectstore.Manifest{}, err
	}
	prepared := append([]analysisPlan(nil), plans...)
	for index := range prepared {
		execution, err := s.executionPlan(prepared[index])
		if err != nil {
			return objectstore.Manifest{}, err
		}
		prepared[index].Execution = execution
	}

	type planResult struct {
		manifest objectstore.Manifest
		err      error
	}
	results := make([]planResult, len(prepared))
	scheduler := newWeightedScheduler(runtime.GOMAXPROCS(0))
	var group sync.WaitGroup
	for index := range prepared {
		index := index
		plan := prepared[index]
		group.Add(1)
		go func() {
			defer group.Done()
			scheduler.acquire(plan.Execution.ReservedWeight)
			defer scheduler.release(plan.Execution.ReservedWeight)
			results[index].manifest, results[index].err = s.analyzePlan(ctx, manifest, plan, temporary)
		}()
	}
	group.Wait()

	for index, result := range results {
		if result.err != nil {
			return objectstore.Manifest{}, result.err
		}
		language := prepared[index].Language
		entry, present := result.manifest.Language(language)
		if !present {
			manifest = manifest.WithoutLanguage(language)
			continue
		}
		manifest = manifest.WithLanguage(entry)
	}
	return manifest, nil
}

func (s *Scanner) analyzePlan(
	ctx context.Context,
	manifest objectstore.Manifest,
	plan analysisPlan,
	temporary string,
) (objectstore.Manifest, error) {
	sourceRoot := filepath.Join(s.StateRoot, "source")
	if !plan.KnownPresent {
		present, err := hasLanguage(sourceRoot, plan.Language)
		if err != nil {
			return objectstore.Manifest{}, err
		}
		if !present {
			s.writeOutput("removing %s analysis\n", plan.Language)
			return manifest.WithoutLanguage(plan.Language), nil
		}
	}

	adapterOutput := filepath.Join(temporary, plan.Language+".jsonl")
	_ = os.Remove(adapterOutput)
	if plan.Full {
		s.writeOutput("analyzing %s\n", plan.Language)
	} else {
		s.writeOutput("analyzing %s files: %d\n", plan.Language, len(plan.ChangedFiles))
	}
	request, err := s.analysisRequest(plan, sourceRoot, temporary, adapterOutput)
	if err != nil {
		return objectstore.Manifest{}, err
	}
	if err := s.Analyzer.Run(ctx, request); err != nil {
		if plan.Full {
			return objectstore.Manifest{}, err
		}
		analysis, retryErr := s.retryFull(ctx, request, sourceRoot, err)
		if retryErr != nil {
			return objectstore.Manifest{}, retryErr
		}
		return s.applyFullAnalysis(manifest, analysis, sourceRoot)
	}

	analysis, err := objectstore.ReadAnalysis(adapterOutput, plan.Language)
	if err != nil {
		return objectstore.Manifest{}, err
	}
	if plan.Full {
		return s.applyFullAnalysis(manifest, analysis, sourceRoot)
	}
	if !analysis.IsIncremental() {
		analysis, err = s.retryFull(ctx, request, sourceRoot, fmt.Errorf("scoped adapter emitted a full stream"))
		if err != nil {
			return objectstore.Manifest{}, err
		}
		return s.applyFullAnalysis(manifest, analysis, sourceRoot)
	}
	fullRequired, err := s.Store.RequiresFullAnalysis(plan.Language, plan.ChangedFiles, analysis)
	if err != nil {
		return objectstore.Manifest{}, err
	}
	if fullRequired {
		analysis, err = s.retryFull(ctx, request, sourceRoot, nil)
		if err != nil {
			return objectstore.Manifest{}, err
		}
		return s.applyFullAnalysis(manifest, analysis, sourceRoot)
	}
	previous, ok := manifest.Language(plan.Language)
	if !ok {
		analysis, err = s.retryFull(ctx, request, sourceRoot, fmt.Errorf("snapshot has no %s analysis", plan.Language))
		if err != nil {
			return objectstore.Manifest{}, err
		}
		return s.applyFullAnalysis(manifest, analysis, sourceRoot)
	}
	fingerprint, err := s.adapterFingerprint(plan.Language)
	if err != nil {
		return objectstore.Manifest{}, err
	}
	entry, err := s.Store.BuildIncrementalLanguage(
		previous,
		analysis,
		sourceRoot,
		config.AnalysisID(),
		fingerprint,
		plan.ChangedFiles,
		plan.RemovedFiles,
		false,
	)
	if err != nil {
		return objectstore.Manifest{}, err
	}
	return manifest.WithLanguage(entry), nil
}

func (s *Scanner) applyFullAnalysis(
	manifest objectstore.Manifest,
	analysis *objectstore.Analysis,
	sourceRoot string,
) (objectstore.Manifest, error) {
	fingerprint, err := s.adapterFingerprint(analysis.Header.Language)
	if err != nil {
		return objectstore.Manifest{}, err
	}
	entry, err := s.Store.BuildFullLanguage(
		analysis,
		sourceRoot,
		analysis.Header.Language,
		config.AnalysisID(),
		fingerprint,
	)
	if err != nil {
		return objectstore.Manifest{}, err
	}
	return manifest.WithLanguage(entry), nil
}

func (s *Scanner) adapterFingerprint(language string) (string, error) {
	if s.AdapterRoot == "" {
		return "", nil
	}
	return adapters.Fingerprint(s.AdapterRoot, language)
}

func (s *Scanner) analysisRequest(plan analysisPlan, sourceRoot, temporary, output string) (adapters.Request, error) {
	repository := sourceRoot
	if !plan.Full {
		var err error
		repository, err = analysisscope.Build(sourceRoot, filepath.Join(temporary, "scopes"), plan.Language, plan.ContextFiles)
		if err != nil {
			return adapters.Request{}, err
		}
	}
	return adapters.Request{
		Language: plan.Language, Repository: repository, Output: output,
		ChangedFiles: plan.ChangedFiles, RemovedFiles: plan.RemovedFiles,
		Workers: plan.Execution.ActiveWorkers, Shards: plan.Execution.LogicalShards,
		MergeFanIn: plan.Execution.MergeFanIn,
	}, nil
}

func (s *Scanner) retryFull(
	ctx context.Context,
	request adapters.Request,
	sourceRoot string,
	scopedErr error,
) (*objectstore.Analysis, error) {
	s.writeOutput("expanding %s to full analysis\n", request.Language)
	_ = os.Remove(request.Output)
	request.Repository = sourceRoot
	request.ChangedFiles = nil
	request.RemovedFiles = nil
	if err := s.Analyzer.Run(ctx, request); err != nil {
		if scopedErr != nil {
			return nil, fmt.Errorf("scoped %s analysis failed: %v; full retry failed: %w", request.Language, scopedErr, err)
		}
		return nil, err
	}
	analysis, err := objectstore.ReadAnalysis(request.Output, request.Language)
	if err != nil {
		return nil, err
	}
	if analysis.IsIncremental() {
		return nil, fmt.Errorf("full %s retry emitted incremental output", request.Language)
	}
	return analysis, nil
}

func (s *Scanner) writeOutput(format string, arguments ...any) {
	if s.Output == nil {
		return
	}
	s.outputMu.Lock()
	defer s.outputMu.Unlock()
	_, _ = fmt.Fprintf(s.Output, format, arguments...)
}

func planLanguages(plans []analysisPlan) []string {
	languages := make([]string, 0, len(plans))
	for _, plan := range plans {
		languages = append(languages, plan.Language)
	}
	return languages
}
