package scan

import (
	"path/filepath"
	"sort"
	"strings"

	lexfiles "github.com/Lokee86/lexicon/internal/files"
	languageRegistry "github.com/Lokee86/lexicon/internal/languages"
	"github.com/Lokee86/lexicon/internal/state"
)

type analysisPlan struct {
	Language     string
	Full         bool
	KnownPresent bool
	ChangedFiles []string
	AddedFiles   []string
	RemovedFiles []string
	ContextFiles []string
	Execution    ExecutionPlan
}

func (s *Scanner) plansFor(changes []state.Change, drift []string) ([]analysisPlan, error) {
	plans := make(map[string]*analysisPlan)
	for _, language := range drift {
		plans[language] = &analysisPlan{Language: language, Full: true}
	}
	for _, change := range changes {
		status := strings.TrimSpace(change.Status)
		if status == "" {
			continue
		}
		switch status[0] {
		case 'M':
			s.addIncrementalPath(plans, change.New, false)
		case 'A':
			s.addIncrementalPath(plans, change.New, true)
		default:
			paths := []string{change.New}
			if change.Old != "" {
				paths = append(paths, change.Old)
			}
			for _, path := range paths {
				for _, language := range lexfiles.Languages(path) {
					if !s.languageEnabled(language) {
						continue
					}
					plan := ensurePlan(plans, language)
					plan.Full = true
				}
			}
		}
	}

	result := make([]analysisPlan, 0, len(plans))
	for _, plan := range plans {
		if !plan.Full {
			roots := uniqueSorted(plan.ChangedFiles)
			var impacted, context []string
			fullRequired := false
			if len(roots) > 0 {
				var err error
				fullRequired, impacted, context, err = s.Store.IncrementalScopeWithAdditions(plan.Language, roots, uniqueSorted(plan.AddedFiles))
				if err != nil {
					fullRequired = true
				}
			}
			if fullRequired {
				plan.Full = true
			} else {
				added := uniqueSorted(plan.AddedFiles)
				plan.ChangedFiles = uniqueSorted(append(impacted, added...))
				plan.RemovedFiles = []string{}
				plan.ContextFiles = uniqueSorted(append(context, added...))
			}
		}
		if plan.Full {
			plan.ChangedFiles = nil
			plan.AddedFiles = nil
			plan.RemovedFiles = nil
			plan.ContextFiles = nil
		}
		result = append(result, *plan)
	}
	sort.Slice(result, func(left, right int) bool {
		return result[left].Language < result[right].Language
	})
	return result, nil
}

func (s *Scanner) addIncrementalPath(plans map[string]*analysisPlan, path string, added bool) {
	for _, language := range lexfiles.Languages(path) {
		if !s.languageEnabled(language) {
			continue
		}
		plan := ensurePlan(plans, language)
		if added && language != "python" {
			plan.Full = true
			continue
		}
		if !languageOwnsSource(language, path) {
			plan.Full = true
			continue
		}
		if added {
			plan.AddedFiles = append(plan.AddedFiles, path)
		} else {
			plan.ChangedFiles = append(plan.ChangedFiles, path)
		}
	}
}

func ensurePlan(plans map[string]*analysisPlan, language string) *analysisPlan {
	plan := plans[language]
	if plan == nil {
		plan = &analysisPlan{Language: language}
		plans[language] = plan
	}
	return plan
}

func languageOwnsSource(language, path string) bool {
	return languageRegistry.OwnsSource(language, path)
}

func uniqueSorted(paths []string) []string {
	set := make(map[string]struct{}, len(paths))
	for _, path := range paths {
		if path != "" {
			set[filepath.ToSlash(path)] = struct{}{}
		}
	}
	result := make([]string, 0, len(set))
	for path := range set {
		result = append(result, path)
	}
	sort.Strings(result)
	return result
}
