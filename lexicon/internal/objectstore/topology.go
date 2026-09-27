package objectstore

import (
	"encoding/json"
	"fmt"
)

func (s Store) DirectChangesRequireFull(language string, roots []string) (bool, error) {
	fullRequired, _, _, err := s.IncrementalScope(language, roots)
	return fullRequired, err
}

func additionSensitiveUnresolved(reason string) bool {
	switch reason {
	case "missing-target", "ambiguous-target", "generated-target":
		return true
	default:
		return false
	}
}

func topologySensitiveUnresolved(reason string) bool {
	switch reason {
	case "ambiguous-target", "generated-target":
		return true
	default:
		return false
	}
}

type topologyRecord struct {
	Record        string `json:"record"`
	Source        string `json:"source"`
	Target        string `json:"target"`
	Relation      string `json:"relation"`
	Expression    string `json:"expression"`
	Reason        string `json:"reason"`
	CandidateName string `json:"candidate_name"`
}

func (s Store) RequiresFullAnalysis(language string, changedFiles []string, analysis *Analysis) (bool, error) {
	if analysis == nil || !analysis.IsIncremental() {
		return true, nil
	}
	_, manifest, err := s.Current()
	if err != nil {
		return true, err
	}
	entry, ok := languageEntry(manifest, language)
	if !ok {
		return true, nil
	}
	files := make(map[string]FileEntry, len(entry.Files))
	for _, file := range entry.Files {
		files[file.Path] = file
	}
	selected := make(map[string]struct{}, len(changedFiles))
	added := make(map[string]struct{})
	previous := make(map[string]map[string]struct{}, len(changedFiles))
	for _, path := range changedFiles {
		path = normalizeOwner(path)
		if path == "" {
			return true, nil
		}
		selected[path] = struct{}{}
		file, ok := files[path]
		if !ok {
			added[path] = struct{}{}
			previous[path] = map[string]struct{}{}
			continue
		}
		object, err := s.LoadObject(file.ObjectID)
		if err != nil {
			return true, err
		}
		previous[path] = relationKeys(object.Records)
	}
	if analysis.partitions != nil {
		return partitionedAnalysisRequiresFull(analysis.partitions, selected, added, previous)
	}
	owners := nodeOwners(analysis.records)
	for _, record := range analysis.records {
		key, relationship, err := relationKey(record.raw)
		if err != nil {
			return true, err
		}
		if !relationship {
			continue
		}
		owner := recordOwner(record.value, owners)
		if owner == "" {
			continue
		}
		if _, ok := selected[owner]; !ok {
			return true, nil
		}
		_, isAdded := added[owner]
		if _, existed := previous[owner][key]; existed {
			continue
		}
		if record.value.Record == "unresolved" {
			var unresolved topologyRecord
			if err := json.Unmarshal(record.raw, &unresolved); err != nil {
				return true, err
			}
			if topologySensitiveUnresolved(unresolved.Reason) {
				return true, nil
			}
		}
		if isAdded {
			continue
		}
	}
	return false, nil
}

func partitionedAnalysisRequiresFull(
	partitions *analysisPartitions,
	selected, added map[string]struct{},
	previous map[string]map[string]struct{},
) (bool, error) {
	for owner, records := range partitions.groups {
		if len(records.edges) == 0 && len(records.unresolved) == 0 {
			continue
		}
		if _, ok := selected[owner]; !ok {
			return true, nil
		}
		for _, record := range records.edges {
			key, err := relationKeyValues([]string{"edge", record.Source, record.Target, record.Relation})
			if err != nil {
				return true, err
			}
			if _, existed := previous[owner][key]; existed {
				continue
			}
		}
		for _, record := range records.unresolved {
			key, err := relationKeyValues([]string{
				"unresolved", record.Source, record.Relation, record.Expression, record.Reason, record.CandidateName,
			})
			if err != nil {
				return true, err
			}
			if _, existed := previous[owner][key]; existed {
				continue
			}
			if topologySensitiveUnresolved(record.Reason) {
				return true, nil
			}
			if _, isAdded := added[owner]; isAdded {
				continue
			}
		}
	}
	return false, nil
}

func relationKeys(records []json.RawMessage) map[string]struct{} {
	result := make(map[string]struct{})
	for _, raw := range records {
		key, relationship, err := relationKey(raw)
		if err == nil && relationship {
			result[key] = struct{}{}
		}
	}
	return result
}

func relationKey(raw json.RawMessage) (string, bool, error) {
	var record topologyRecord
	if err := json.Unmarshal(raw, &record); err != nil {
		return "", false, fmt.Errorf("decode relationship topology: %w", err)
	}
	var values []string
	switch record.Record {
	case "edge":
		values = []string{"edge", record.Source, record.Target, record.Relation}
	case "unresolved":
		values = []string{"unresolved", record.Source, record.Relation, record.Expression, record.Reason, record.CandidateName}
	default:
		return "", false, nil
	}
	key, err := relationKeyValues(values)
	if err != nil {
		return "", false, err
	}
	return key, true, nil
}

func relationKeyValues(values []string) (string, error) {
	encoded, err := json.Marshal(values)
	if err != nil {
		return "", err
	}
	return string(encoded), nil
}
