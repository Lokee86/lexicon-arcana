package objectstore

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"sort"
)

type NodeFactSpan struct {
	Path        string
	StartLine   uint64
	StartColumn uint64
	EndLine     uint64
	EndColumn   uint64
}

type NodeFact struct {
	Attributes    json.RawMessage
	ContentID     string
	ID            string
	Kind          string
	Name          string
	Owner         string
	Path          string
	QualifiedName string
	Span          *NodeFactSpan
}

// LanguageNodes loads only node facts for one immutable language entry.
// Binary edge and unresolved sections are integrity-checked by the object digest
// but are not decoded or materialized.
func (s Store) LanguageNodes(entry LanguageEntry) ([]NodeFact, error) {
	nodes := make([]NodeFact, 0)
	if entry.SharedObjectID != "" {
		loaded, err := s.loadNodeFacts(entry.SharedObjectID, entry, "shared", "", "")
		if err != nil {
			return nil, err
		}
		nodes = append(nodes, loaded...)
	}

	files := append([]FileEntry(nil), entry.Files...)
	sort.Slice(files, func(left, right int) bool { return files[left].Path < files[right].Path })
	for _, file := range files {
		loaded, err := s.loadNodeFacts(file.ObjectID, entry, "file", file.Path, file.ContentID)
		if err != nil {
			return nil, err
		}
		nodes = append(nodes, loaded...)
	}
	return nodes, nil
}

func (s Store) loadNodeFacts(id string, entry LanguageEntry, kind, owner, contentID string) ([]NodeFact, error) {
	if !validID(id) {
		return nil, fmt.Errorf("invalid Lexicon object ID %q", id)
	}
	data, err := os.ReadFile(s.objectPath(id))
	if err != nil {
		return nil, fmt.Errorf("read Lexicon object %s: %w", id, err)
	}
	canonical := data
	if !isBinaryObject(data) {
		canonical = bytes.TrimSpace(data)
	}
	if actual := digest("lexicon:fact-object:v1\x00", canonical); actual != id {
		return nil, fmt.Errorf("Lexicon object %s failed content verification", id)
	}

	if !isBinaryObject(canonical) {
		object, err := s.LoadObject(id)
		if err != nil {
			return nil, err
		}
		if err := validateExportObject(object, entry, kind, owner, contentID); err != nil {
			return nil, err
		}
		records, err := parseTypedRecords(object.Records)
		if err != nil {
			return nil, fmt.Errorf("decode node facts from object %s: %w", id, err)
		}
		return exportNodeFacts(records.nodes), nil
	}

	object, records, err := decodeBinaryNodeFacts(canonical)
	if err != nil {
		return nil, fmt.Errorf("decode node facts from object %s: %w", id, err)
	}
	if object.Version != ObjectVersion {
		return nil, fmt.Errorf("unsupported Lexicon object version %d", object.Version)
	}
	if err := validateExportObject(object, entry, kind, owner, contentID); err != nil {
		return nil, err
	}
	return exportNodeFacts(records), nil
}

func decodeBinaryNodeFacts(data []byte) (FactObject, []nodeRecord, error) {
	if len(data) < len(binaryObjectMagic) {
		return FactObject{}, nil, fmt.Errorf("invalid Lexicon binary object magic")
	}
	switch {
	case bytes.Equal(data[:len(binaryObjectMagic)], binaryObjectMagic[:]):
		return decodeBinaryNodeFactsV2(data)
	case bytes.Equal(data[:len(legacyBinaryObjectMagic)], legacyBinaryObjectMagic[:]):
		return decodeBinaryNodeFactsV1(data)
	default:
		return FactObject{}, nil, fmt.Errorf("invalid Lexicon binary object magic")
	}
}

func decodeBinaryNodeFactsV2(data []byte) (FactObject, []nodeRecord, error) {
	reader := binaryObjectReader{data: data, position: len(binaryObjectMagic)}
	version, err := reader.uvarint("object version")
	if err != nil {
		return FactObject{}, nil, err
	}
	schemaVersion, err := reader.uvarint("schema version")
	if err != nil {
		return FactObject{}, nil, err
	}
	strings, err := reader.stringTableV2()
	if err != nil {
		return FactObject{}, nil, err
	}
	language, err := reader.stringRef(strings, "language")
	if err != nil {
		return FactObject{}, nil, err
	}
	owner, err := reader.stringRef(strings, "owner")
	if err != nil {
		return FactObject{}, nil, err
	}
	sourceContentID, err := readIdentity(&reader, strings, "source content ID")
	if err != nil {
		return FactObject{}, nil, err
	}
	adapterVersion, err := reader.stringRef(strings, "adapter version")
	if err != nil {
		return FactObject{}, nil, err
	}
	analysisConfigID, err := readIdentity(&reader, strings, "analysis config ID")
	if err != nil {
		return FactObject{}, nil, err
	}
	externalCount, err := reader.count("external references", maxBinaryExternalReferences)
	if err != nil {
		return FactObject{}, nil, err
	}
	for index := 0; index < externalCount; index++ {
		if _, err := readIdentity(&reader, strings, fmt.Sprintf("external reference %d", index)); err != nil {
			return FactObject{}, nil, err
		}
	}

	nodeSection, err := reader.bytes("node section", maxBinarySectionSize)
	if err != nil {
		return FactObject{}, nil, err
	}
	if _, err := reader.bytes("edge section", maxBinarySectionSize); err != nil {
		return FactObject{}, nil, err
	}
	if _, err := reader.bytes("unresolved section", maxBinarySectionSize); err != nil {
		return FactObject{}, nil, err
	}
	if reader.position != len(data) {
		return FactObject{}, nil, fmt.Errorf("Lexicon binary object has %d trailing bytes", len(data)-reader.position)
	}
	nodes, err := decodeCompactNodes(nodeSection, strings, owner)
	if err != nil {
		return FactObject{}, nil, err
	}
	return FactObject{
		Version:          int(version),
		Language:         language,
		Owner:            owner,
		SourceContentID:  sourceContentID,
		AdapterVersion:   adapterVersion,
		SchemaVersion:    int(schemaVersion),
		AnalysisConfigID: analysisConfigID,
	}, nodes, nil
}

func decodeBinaryNodeFactsV1(data []byte) (FactObject, []nodeRecord, error) {
	reader := binaryObjectReader{data: data, position: len(legacyBinaryObjectMagic)}
	version, err := reader.uvarint("object version")
	if err != nil {
		return FactObject{}, nil, err
	}
	schemaVersion, err := reader.uvarint("schema version")
	if err != nil {
		return FactObject{}, nil, err
	}
	strings, err := reader.stringTable()
	if err != nil {
		return FactObject{}, nil, err
	}
	language, err := reader.stringRef(strings, "language")
	if err != nil {
		return FactObject{}, nil, err
	}
	owner, err := reader.stringRef(strings, "owner")
	if err != nil {
		return FactObject{}, nil, err
	}
	sourceContentID, err := reader.stringRef(strings, "source content ID")
	if err != nil {
		return FactObject{}, nil, err
	}
	adapterVersion, err := reader.stringRef(strings, "adapter version")
	if err != nil {
		return FactObject{}, nil, err
	}
	analysisConfigID, err := reader.stringRef(strings, "analysis config ID")
	if err != nil {
		return FactObject{}, nil, err
	}

	nodeSection, err := reader.bytes("node section", maxBinarySectionSize)
	if err != nil {
		return FactObject{}, nil, err
	}
	if _, err := reader.bytes("edge section", maxBinarySectionSize); err != nil {
		return FactObject{}, nil, err
	}
	if _, err := reader.bytes("unresolved section", maxBinarySectionSize); err != nil {
		return FactObject{}, nil, err
	}
	if reader.position != len(data) {
		return FactObject{}, nil, fmt.Errorf("Lexicon binary object has %d trailing bytes", len(data)-reader.position)
	}
	nodes, err := decodeNodeSection(nodeSection, strings)
	if err != nil {
		return FactObject{}, nil, err
	}
	return FactObject{
		Version:          int(version),
		Language:         language,
		Owner:            owner,
		SourceContentID:  sourceContentID,
		AdapterVersion:   adapterVersion,
		SchemaVersion:    int(schemaVersion),
		AnalysisConfigID: analysisConfigID,
	}, nodes, nil
}

func exportNodeFacts(records []nodeRecord) []NodeFact {
	result := make([]NodeFact, len(records))
	for index, record := range records {
		result[index] = NodeFact{
			Attributes:    append(json.RawMessage(nil), record.Attributes...),
			ContentID:     record.ContentID,
			ID:            record.ID,
			Kind:          record.Kind,
			Name:          record.Name,
			Owner:         record.Owner,
			Path:          record.Path,
			QualifiedName: record.QualifiedName,
			Span:          exportNodeFactSpan(record.Span),
		}
	}
	return result
}

func exportNodeFactSpan(span *sourceSpan) *NodeFactSpan {
	if span == nil {
		return nil
	}
	return &NodeFactSpan{
		Path: span.Path, StartLine: span.StartLine, StartColumn: span.StartColumn,
		EndLine: span.EndLine, EndColumn: span.EndColumn,
	}
}
