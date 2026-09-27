package objectstore

import (
	"encoding/json"
	"fmt"
	"sort"
)

type dependencyRecord struct {
	Record   string `json:"record"`
	ID       string `json:"id"`
	Source   string `json:"source"`
	Target   string `json:"target"`
	Relation string `json:"relation"`
	Path     string `json:"path"`
}

// IncrementalScope loads the current language objects once and computes the
// incremental emission and context closures from the previous dependency view.
// Semantic topology safety is checked after scoped analysis by
// RequiresFullAnalysis; the previous graph alone cannot tell whether an
// existing cross-file relationship or unresolved reference actually changed.
func (s Store) IncrementalScope(language string, roots []string) (bool, []string, []string, error) {
	_, objects, nodeOwners, err := s.dependencyData(language)
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

func (s Store) dependencyData(language string) (LanguageEntry, map[string]FactObject, map[string]string, error) {
	_, manifest, err := s.Current()
	if err != nil {
		return LanguageEntry{}, nil, nil, err
	}
	entry, ok := languageEntry(manifest, language)
	if !ok {
		return LanguageEntry{}, nil, nil, fmt.Errorf("snapshot has no %s analysis", language)
	}
	objects := make(map[string]FactObject, len(entry.Files))
	nodeOwners := make(map[string]string)
	knownPaths := make(map[string]struct{}, len(entry.Files))
	for _, file := range entry.Files {
		knownPaths[file.Path] = struct{}{}
		object, err := s.LoadObject(file.ObjectID)
		if err != nil {
			return LanguageEntry{}, nil, nil, err
		}
		objects[file.Path] = object
		for _, raw := range object.Records {
			var record dependencyRecord
			if err := json.Unmarshal(raw, &record); err != nil {
				return LanguageEntry{}, nil, nil, fmt.Errorf("decode %s dependency record: %w", file.Path, err)
			}
			if record.Record == "node" && record.ID != "" {
				nodeOwners[record.ID] = file.Path
			}
		}
	}
	if entry.SharedObjectID != "" {
		shared, err := s.LoadObject(entry.SharedObjectID)
		if err != nil {
			return LanguageEntry{}, nil, nil, err
		}
		for _, raw := range shared.Records {
			var record dependencyRecord
			if err := json.Unmarshal(raw, &record); err != nil {
				return LanguageEntry{}, nil, nil, fmt.Errorf("decode shared %s dependency record: %w", language, err)
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
	return entry, objects, nodeOwners, nil
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
