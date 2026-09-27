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
		case 'R':
			s.addRename(plans, change.Old, change.New)
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
			removed := uniqueSorted(plan.RemovedFiles)
			roots := uniqueSorted(append(append([]string(nil), plan.ChangedFiles...), removed...))
			var impacted, context []string
			fullRequired := false
			if len(roots) > 0 || len(plan.AddedFiles) > 0 {
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
				plan.ChangedFiles = withoutPaths(uniqueSorted(append(impacted, added...)), removed)
				plan.RemovedFiles = removed
				plan.ContextFiles = withoutPaths(uniqueSorted(append(context, added...)), removed)
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

func (s *Scanner) addRename(plans map[string]*analysisPlan, oldPath, newPath string) {
	oldPython := languageOwnsSource("python", oldPath)
	newPython := languageOwnsSource("python", newPath)
	if s.languageEnabled("python") && (oldPython || newPython) {
		plan := ensurePlan(plans, "python")
		if oldPython && newPython {
			plan.RemovedFiles = append(plan.RemovedFiles, oldPath)
			plan.AddedFiles = append(plan.AddedFiles, newPath)
		} else {
			plan.Full = true
		}
	}
	for _, path := range []string{oldPath, newPath} {
		for _, language := range lexfiles.Languages(path) {
			if language == "python" || !s.languageEnabled(language) {
				continue
			}
			ensurePlan(plans, language).Full = true
		}
	}
}

func withoutPaths(paths, removed []string) []string {
	if len(removed) == 0 {
		return paths
	}
	blocked := make(map[string]struct{}, len(removed))
	for _, path := range removed {
		blocked[filepath.ToSlash(path)] = struct{}{}
	}
	result := make([]string, 0, len(paths))
	for _, path := range paths {
		if _, drop := blocked[filepath.ToSlash(path)]; !drop {
			result = append(result, path)
		}
	}
	return result
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
