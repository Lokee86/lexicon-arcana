package interstack

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/Lokee86/lexicon/internal/objectstore"
)

func Build(
	sourceRoot string,
	store objectstore.Store,
	manifest objectstore.Manifest,
	outputPath string,
) (*objectstore.Analysis, Summary, error) {
	libraries := make([]Library, 0, len(manifest.Languages))
	for _, entry := range manifest.Languages {
		if entry.Language == Language {
			continue
		}
		facts, err := store.LanguageNodes(entry)
		if err != nil {
			return nil, Summary{}, fmt.Errorf("load %s nodes for interstack analysis: %w", entry.Language, err)
		}
		library, err := libraryFromNodeFacts(entry, facts)
		if err != nil {
			return nil, Summary{}, err
		}
		libraries = append(libraries, library)
	}
	result, err := Resolve(sourceRoot, libraries)
	if err != nil {
		return nil, Summary{}, err
	}
	data, err := Encode(result)
	if err != nil {
		return nil, Summary{}, err
	}
	if err := os.MkdirAll(filepath.Dir(outputPath), 0o755); err != nil {
		return nil, Summary{}, err
	}
	if err := os.WriteFile(outputPath, data, 0o644); err != nil {
		return nil, Summary{}, err
	}
	analysis, err := objectstore.ReadAnalysis(outputPath, Language)
	if err != nil {
		return nil, Summary{}, err
	}
	return analysis, result.Summary, nil
}
func libraryFromNodeFacts(entry objectstore.LanguageEntry, facts []objectstore.NodeFact) (Library, error) {
	library := Library{
		Language: entry.Language, Repository: entry.Repository,
		Nodes: make([]Node, 0, len(facts)),
	}
	for _, fact := range facts {
		var attributes map[string]any
		if len(fact.Attributes) > 0 {
			if err := json.Unmarshal(fact.Attributes, &attributes); err != nil {
				return Library{}, fmt.Errorf("decode %s node %s attributes: %w", entry.Language, fact.ID, err)
			}
		}
		var span *Span
		if fact.Span != nil {
			span = &Span{
				Path:      fact.Span.Path,
				StartLine: uint32(fact.Span.StartLine), StartColumn: uint32(fact.Span.StartColumn),
				EndLine: uint32(fact.Span.EndLine), EndColumn: uint32(fact.Span.EndColumn),
			}
		}
		library.Nodes = append(library.Nodes, Node{
			ID: fact.ID, Kind: fact.Kind, Name: fact.Name, Path: fact.Path,
			QualifiedName: fact.QualifiedName, Span: span, Attributes: attributes,
		})
	}
	return library, nil
}
