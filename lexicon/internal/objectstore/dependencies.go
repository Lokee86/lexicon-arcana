package objectstore

import (
	"encoding/json"
	"fmt"
	"path/filepath"
	"sort"
	"strings"
)

type dependencyRecord struct {
	Record        string `json:"record"`
	ID            string `json:"id"`
	Source        string `json:"source"`
	Target        string `json:"target"`
	Relation      string `json:"relation"`
	Path          string `json:"path"`
	Reason        string `json:"reason"`
	CandidateName string `json:"candidate_name"`
}

// IncrementalScope computes the bounded dependency scope for existing roots.
func (s Store) IncrementalScope(language string, roots []string) (bool, []string, []string, error) {
	return s.IncrementalScopeWithAdditions(language, roots, nil)
}

// IncrementalScopeWithAdditions also validates whether newly-added Python
// modules could satisfy repository-sensitive unresolved imports in the prior
// snapshot. If so, the caller must fall back to full analysis.
func (s Store) IncrementalScopeWithAdditions(language string, roots, additions []string) (bool, []string, []string, error) {
	_, objects, nodeOwners, unresolvedCandidates, err := s.dependencyData(language)
	if err != nil {
		return true, nil, nil, err
	}
	rootSet := make(map[string]struct{}, len(roots))
	for _, path := range roots {
		rootSet[path] = struct{}{}
	}
	foundRoots := make(map[string]struct{}, len(rootSet))
	reverse := make(map[string]map[string]struct{})
	forward := make(map[string]map[string]struct{})
	for owner, object := range objects {
		if _, directRoot := rootSet[owner]; directRoot {
			foundRoots[owner] = struct{}{}
		}
		for _, raw := range object.Records {
			var record dependencyRecord
			if err := json.Unmarshal(raw, &record); err != nil {
				return true, nil, nil, err
			}
			if record.Record != "edge" || record.Target == "" {
				continue
			}
			targetOwner := nodeOwners[record.Target]
			if targetOwner == "" || targetOwner == owner {
				continue
			}
			addRelation(reverse, targetOwner, owner)
			addRelation(forward, owner, targetOwner)
		}
	}
	fullRequired := len(foundRoots) != len(rootSet)
	if len(additions) > 0 {
		if language != "python" {
			fullRequired = true
		} else {
			for _, path := range additions {
				candidate, ok := pythonModuleCandidate(path)
				if !ok {
					fullRequired = true
					break
				}
				if _, exists := unresolvedCandidates[candidate]; exists {
					fullRequired = true
					break
				}
			}
		}
	}
	emit := oneHopClosure(roots, reverse)
	context := oneHopClosure(emit, forward)
	return fullRequired, emit, context, nil
}

func (s Store) DependencyScope(language string, roots []string) ([]string, []string, error) {
	_, emit, context, err := s.IncrementalScope(language, roots)
	return emit, context, err
}

func (s Store) ImpactedFiles(language string, roots []string) ([]string, error) {
	emit, _, err := s.DependencyScope(language, roots)
	return emit, err
}

func (s Store) dependencyData(language string) (LanguageEntry, map[string]FactObject, map[string]string, map[string]struct{}, error) {
	_, manifest, err := s.Current()
	if err != nil {
		return LanguageEntry{}, nil, nil, nil, err
	}
	entry, ok := languageEntry(manifest, language)
	if !ok {
		return LanguageEntry{}, nil, nil, nil, fmt.Errorf("snapshot has no %s analysis", language)
	}
	objects := make(map[string]FactObject, len(entry.Files))
	nodeOwners := make(map[string]string)
	unresolvedCandidates := make(map[string]struct{})
	knownPaths := make(map[string]struct{}, len(entry.Files))
	for _, file := range entry.Files {
		knownPaths[file.Path] = struct{}{}
		object, err := s.LoadObject(file.ObjectID)
		if err != nil {
			return LanguageEntry{}, nil, nil, nil, err
		}
		objects[file.Path] = object
		for _, raw := range object.Records {
			var record dependencyRecord
			if err := json.Unmarshal(raw, &record); err != nil {
				return LanguageEntry{}, nil, nil, nil, fmt.Errorf("decode %s dependency record: %w", file.Path, err)
			}
			if record.Record == "node" && record.ID != "" {
				nodeOwners[record.ID] = file.Path
			}
			if record.Record == "unresolved" && repositorySensitiveUnresolved(record.Reason) {
				if candidate := strings.TrimSpace(record.CandidateName); candidate != "" {
					unresolvedCandidates[candidate] = struct{}{}
				}
			}
		}
	}
	if entry.SharedObjectID != "" {
		shared, err := s.LoadObject(entry.SharedObjectID)
		if err != nil {
			return LanguageEntry{}, nil, nil, nil, err
		}
		for _, raw := range shared.Records {
			var record dependencyRecord
			if err := json.Unmarshal(raw, &record); err != nil {
				return LanguageEntry{}, nil, nil, nil, fmt.Errorf("decode shared %s dependency record: %w", language, err)
			}
			if record.Record != "node" || record.ID == "" {
				continue
			}
			path := normalizeOwner(record.Path)
			if _, ok := knownPaths[path]; ok {
				nodeOwners[record.ID] = path
			}
		}
	}
	return entry, objects, nodeOwners, unresolvedCandidates, nil
}

func pythonModuleCandidate(path string) (string, bool) {
	path = filepath.ToSlash(filepath.Clean(filepath.FromSlash(path)))
	if !strings.EqualFold(filepath.Ext(path), ".py") {
		return "", false
	}
	path = strings.TrimSuffix(path, filepath.Ext(path))
	parts := strings.Split(path, "/")
	if len(parts) == 0 {
		return "", false
	}
	if parts[len(parts)-1] == "__init__" {
		parts = parts[:len(parts)-1]
	}
	if len(parts) == 0 {
		return "", false
	}
	return strings.Join(parts, "."), true
}

func addRelation(graph map[string]map[string]struct{}, source, target string) {
	if graph[source] == nil {
		graph[source] = make(map[string]struct{})
	}
	graph[source][target] = struct{}{}
}

func oneHopClosure(seeds []string, graph map[string]map[string]struct{}) []string {
	selected := make(map[string]struct{})
	for _, seed := range seeds {
		if seed == "" {
			continue
		}
		selected[seed] = struct{}{}
		for next := range graph[seed] {
			if next != "" {
				selected[next] = struct{}{}
			}
		}
	}
	result := make([]string, 0, len(selected))
	for path := range selected {
		result = append(result, path)
	}
	sort.Strings(result)
	return result
}

func languageEntry(manifest Manifest, language string) (LanguageEntry, bool) {
	for _, entry := range manifest.Languages {
		if entry.Language == language {
			return entry, true
		}
	}
	return LanguageEntry{}, false
}
