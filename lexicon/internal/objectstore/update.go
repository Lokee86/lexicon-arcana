package objectstore

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"

	lexfiles "github.com/Lokee86/lexicon/internal/files"
)

func (s Store) BuildFullLanguage(
	analysis *Analysis,
	sourceRoot, language, analysisConfigID, adapterFingerprint string,
) (LanguageEntry, error) {
	if analysis == nil {
		return LanguageEntry{}, fmt.Errorf("missing %s analysis", language)
	}
	if analysis.IsIncremental() {
		return LanguageEntry{}, fmt.Errorf("application requires full adapter output, got mode %q", analysis.Header.Mode)
	}
	files, err := sourceFiles(sourceRoot, language)
	if err != nil {
		return LanguageEntry{}, err
	}
	allowedOwners := make(map[string]struct{}, len(files))
	for path := range files {
		allowedOwners[path] = struct{}{}
	}
	groups, shared := analysis.groups(allowedOwners)
	entry := languageMetadata(analysis.Header, analysisConfigID, adapterFingerprint)
	paths := sortedMapKeys(files)
	entry.Files, err = s.writeFileObjects(entry, paths, files, groups)
	if err != nil {
		return LanguageEntry{}, err
	}
	entry.SharedObjectID, err = s.writeSharedObject(entry, shared)
	if err != nil {
		return LanguageEntry{}, err
	}
	return entry, nil
}

// BuildSharedLanguage stores a synthetic full analysis as one shared object.
// Derived analyses such as interstack tracing reference nodes owned by other
// language libraries and therefore do not claim source-file ownership.
func (s Store) BuildSharedLanguage(
	analysis *Analysis,
	analysisConfigID, adapterFingerprint string,
) (LanguageEntry, error) {
	if analysis == nil {
		return LanguageEntry{}, fmt.Errorf("missing synthetic analysis")
	}
	if analysis.IsIncremental() {
		return LanguageEntry{}, fmt.Errorf("synthetic analysis must be full")
	}
	records := analysis.allTypedRecords()
	entry := languageMetadata(analysis.Header, analysisConfigID, adapterFingerprint)
	entry.Files = []FileEntry{}
	sharedObjectID, err := s.writeSharedObject(entry, records)
	if err != nil {
		return LanguageEntry{}, err
	}
	entry.SharedObjectID = sharedObjectID
	return entry, nil
}

func (s Store) BuildIncrementalLanguage(
	previous LanguageEntry,
	analysis *Analysis,
	sourceRoot, analysisConfigID, adapterFingerprint string,
	changedFiles, removedFiles []string,
	replaceShared bool,
) (LanguageEntry, error) {
	if analysis == nil || !analysis.IsIncremental() {
		return LanguageEntry{}, fmt.Errorf("application requires incremental adapter output")
	}
	if analysis.Header.Language != previous.Language {
		return LanguageEntry{}, fmt.Errorf("incremental language %q does not match previous %q", analysis.Header.Language, previous.Language)
	}
	if !samePaths(analysis.Header.ChangedFiles, changedFiles) || !samePaths(analysis.Header.RemovedFiles, removedFiles) {
		return LanguageEntry{}, fmt.Errorf("adapter incremental scope does not match requested files")
	}
	changed := pathSet(changedFiles)
	removed := pathSet(removedFiles)
	groups, shared := analysis.groups(nil)
	for owner := range groups {
		if !changed[owner] {
			return LanguageEntry{}, fmt.Errorf("incremental record is owned by undeclared file %q", owner)
		}
		if removed[owner] {
			return LanguageEntry{}, fmt.Errorf("incremental record is owned by removed file %q", owner)
		}
	}

	entry := languageMetadata(analysis.Header, analysisConfigID, adapterFingerprint)
	var invalidatedNodes map[string]struct{}
	if replaceShared {
		var err error
		invalidatedNodes, err = s.invalidatedNodeIDs(previous, changed, removed)
		if err != nil {
			return LanguageEntry{}, err
		}
	}
	files := make(map[string]FileEntry, len(previous.Files)+len(changed))
	for _, file := range previous.Files {
		if !changed[file.Path] && !removed[file.Path] {
			files[file.Path] = file
		}
	}
	for path := range changed {
		if removed[path] {
			continue
		}
		data, err := readLanguageSource(sourceRoot, previous.Language, path)
		if err != nil {
			return LanguageEntry{}, err
		}
		file, err := s.writeFileObject(entry, path, data, groups[path])
		if err != nil {
			return LanguageEntry{}, err
		}
		files[path] = file
	}
	entry.Files = make([]FileEntry, 0, len(files))
	for _, file := range files {
		entry.Files = append(entry.Files, file)
	}
	sort.Slice(entry.Files, func(left, right int) bool { return entry.Files[left].Path < entry.Files[right].Path })
	entry.SharedObjectID = previous.SharedObjectID
	if replaceShared {
		sharedObjectID, err := s.mergeSharedObject(entry, previous.SharedObjectID, shared, invalidatedNodes)
		if err != nil {
			return LanguageEntry{}, err
		}
		entry.SharedObjectID = sharedObjectID
	}
	return entry, nil
}

func (s Store) invalidatedNodeIDs(
	previous LanguageEntry,
	changed, removed map[string]bool,
) (map[string]struct{}, error) {
	result := make(map[string]struct{})
	for _, file := range previous.Files {
		if !changed[file.Path] && !removed[file.Path] {
			continue
		}
		object, err := s.LoadObject(file.ObjectID)
		if err != nil {
			return nil, err
		}
		records, err := parseTypedRecords(object.Records)
		if err != nil {
			return nil, err
		}
		for _, node := range records.nodes {
			if node.ID != "" {
				result[node.ID] = struct{}{}
			}
		}
	}
	if previous.SharedObjectID == "" {
		return result, nil
	}
	sharedObject, err := s.LoadObject(previous.SharedObjectID)
	if err != nil {
		return nil, err
	}
	shared, err := parseTypedRecords(sharedObject.Records)
	if err != nil {
		return nil, err
	}
	for _, node := range shared.nodes {
		path := normalizeOwner(node.Path)
		if path == "" || (!changed[path] && !removed[path]) {
			continue
		}
		if node.ID != "" {
			result[node.ID] = struct{}{}
		}
	}
	return result, nil
}

func (s Store) mergeSharedObject(
	entry LanguageEntry,
	previousID string,
	updates typedRecords,
	invalidatedNodes map[string]struct{},
) (string, error) {
	if previousID == "" {
		return s.writeSharedObject(entry, updates)
	}
	previousObject, err := s.LoadObject(previousID)
	if err != nil {
		return "", err
	}
	previous, err := parseTypedRecords(previousObject.Records)
	if err != nil {
		return "", err
	}
	previous = withoutInvalidatedRelationships(previous, invalidatedNodes)
	merged, err := mergeTypedRecords(previous, updates)
	if err != nil {
		return "", err
	}
	return s.writeSharedObject(entry, merged)
}

func withoutInvalidatedRelationships(
	records typedRecords,
	invalidatedNodes map[string]struct{},
) typedRecords {
	if len(invalidatedNodes) == 0 {
		return records
	}
	filtered := typedRecords{}
	for _, node := range records.nodes {
		if _, invalid := invalidatedNodes[node.ID]; invalid {
			continue
		}
		filtered.nodes = append(filtered.nodes, node)
	}
	for _, edge := range records.edges {
		if _, invalid := invalidatedNodes[edge.Source]; invalid {
			continue
		}
		if _, invalid := invalidatedNodes[edge.Target]; invalid {
			continue
		}
		filtered.edges = append(filtered.edges, edge)
	}
	for _, unresolved := range records.unresolved {
		if _, invalid := invalidatedNodes[unresolved.Source]; invalid {
			continue
		}
		filtered.unresolved = append(filtered.unresolved, unresolved)
	}
	return filtered
}

func mergeTypedRecords(previous, updates typedRecords) (typedRecords, error) {
	previousRaw, err := previous.raw()
	if err != nil {
		return typedRecords{}, err
	}
	updateRaw, err := updates.raw()
	if err != nil {
		return typedRecords{}, err
	}
	records := make(map[string]json.RawMessage, len(previousRaw)+len(updateRaw))
	add := func(values []json.RawMessage) error {
		exported, err := exportRecords(values)
		if err != nil {
			return err
		}
		for _, record := range exported {
			records[record.key] = append(json.RawMessage(nil), record.raw...)
		}
		return nil
	}
	if err := add(previousRaw); err != nil {
		return typedRecords{}, err
	}
	if err := add(updateRaw); err != nil {
		return typedRecords{}, err
	}
	keys := make([]string, 0, len(records))
	for key := range records {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	result := typedRecords{}
	for _, key := range keys {
		record, err := parseTypedRecord(records[key])
		if err != nil {
			return typedRecords{}, err
		}
		result.append(record)
	}
	return result, nil
}

func languageMetadata(header Header, analysisConfigID, adapterFingerprint string) LanguageEntry {
	return LanguageEntry{
		Language: header.Language, AdapterVersion: header.AdapterVersion,
		AdapterFingerprint: adapterFingerprint, SchemaVersion: header.SchemaVersion,
		Repository: header.Repository, AnalysisConfigID: analysisConfigID,
	}
}

func (s Store) writeFileObject(entry LanguageEntry, path string, source []byte, records typedRecords) (FileEntry, error) {
	contentID := ContentID(source)
	objectID, err := s.WriteObject(FactObject{
		Language: entry.Language, Owner: path, SourceContentID: contentID,
		AdapterVersion: entry.AdapterVersion, SchemaVersion: entry.SchemaVersion,
		AnalysisConfigID: entry.AnalysisConfigID, typed: &records,
	})
	if err != nil {
		return FileEntry{}, err
	}
	return FileEntry{Path: path, Language: entry.Language, ContentID: contentID, ObjectID: objectID}, nil
}

func (s Store) writeSharedObject(entry LanguageEntry, records typedRecords) (string, error) {
	if records.len() == 0 {
		return "", nil
	}
	return s.WriteObject(FactObject{
		Language: entry.Language, AdapterVersion: entry.AdapterVersion,
		SchemaVersion: entry.SchemaVersion, AnalysisConfigID: entry.AnalysisConfigID,
		typed: &records,
	})
}

func readLanguageSource(root, language, path string) ([]byte, error) {
	path = normalizeOwner(path)
	if path == "" || !contains(lexfiles.Languages(path), language) {
		return nil, fmt.Errorf("%q is not a %s source path", path, language)
	}
	data, err := os.ReadFile(filepath.Join(root, filepath.FromSlash(path)))
	if err != nil {
		return nil, fmt.Errorf("read %s source %s: %w", language, path, err)
	}
	return data, nil
}

func pathSet(paths []string) map[string]bool {
	result := make(map[string]bool, len(paths))
	for _, path := range normalizedPaths(paths) {
		result[path] = true
	}
	return result
}
