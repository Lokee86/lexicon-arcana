package objectstore

import (
	"bytes"
	"encoding/json"
	"fmt"
	"unicode/utf8"
)

func decodeBinaryObjectV1(data []byte) (FactObject, error) {
	reader := binaryObjectReader{data: data, position: len(legacyBinaryObjectMagic)}
	if len(data) < len(legacyBinaryObjectMagic) || !bytes.Equal(data[:len(legacyBinaryObjectMagic)], legacyBinaryObjectMagic[:]) {
		return FactObject{}, fmt.Errorf("invalid Lexicon binary object magic")
	}
	version, err := reader.uvarint("object version")
	if err != nil {
		return FactObject{}, err
	}
	schemaVersion, err := reader.uvarint("schema version")
	if err != nil {
		return FactObject{}, err
	}
	strings, err := reader.stringTable()
	if err != nil {
		return FactObject{}, err
	}
	language, err := reader.stringRef(strings, "language")
	if err != nil {
		return FactObject{}, err
	}
	owner, err := reader.stringRef(strings, "owner")
	if err != nil {
		return FactObject{}, err
	}
	sourceContentID, err := reader.stringRef(strings, "source content ID")
	if err != nil {
		return FactObject{}, err
	}
	adapterVersion, err := reader.stringRef(strings, "adapter version")
	if err != nil {
		return FactObject{}, err
	}
	analysisConfigID, err := reader.stringRef(strings, "analysis config ID")
	if err != nil {
		return FactObject{}, err
	}

	nodeSection, err := reader.bytes("node section", maxBinarySectionSize)
	if err != nil {
		return FactObject{}, err
	}
	edgeSection, err := reader.bytes("edge section", maxBinarySectionSize)
	if err != nil {
		return FactObject{}, err
	}
	unresolvedSection, err := reader.bytes("unresolved section", maxBinarySectionSize)
	if err != nil {
		return FactObject{}, err
	}
	if reader.position != len(data) {
		return FactObject{}, fmt.Errorf("Lexicon binary object has %d trailing bytes", len(data)-reader.position)
	}

	records := typedRecords{}
	records.nodes, err = decodeNodeSection(nodeSection, strings)
	if err != nil {
		return FactObject{}, err
	}
	records.edges, err = decodeEdgeSection(edgeSection, strings)
	if err != nil {
		return FactObject{}, err
	}
	records.unresolved, err = decodeUnresolvedSection(unresolvedSection, strings)
	if err != nil {
		return FactObject{}, err
	}
	raw, err := records.raw()
	if err != nil {
		return FactObject{}, fmt.Errorf("materialize Lexicon binary records: %w", err)
	}
	return FactObject{
		Version:          int(version),
		Language:         language,
		Owner:            owner,
		SourceContentID:  sourceContentID,
		AdapterVersion:   adapterVersion,
		SchemaVersion:    int(schemaVersion),
		AnalysisConfigID: analysisConfigID,
		Records:          raw,
	}, nil
}

func (reader *binaryObjectReader) stringTable() ([]string, error) {
	count, err := reader.count("string table", maxBinaryStrings)
	if err != nil {
		return nil, err
	}
	if count == 0 {
		return nil, fmt.Errorf("Lexicon binary object has no empty string sentinel")
	}
	strings := make([]string, count)
	for index := range strings {
		value, err := reader.bytes("string", maxBinaryStringSize)
		if err != nil {
			return nil, err
		}
		if !utf8.Valid(value) {
			return nil, fmt.Errorf("Lexicon binary object string %d is not valid UTF-8", index)
		}
		strings[index] = string(value)
	}
	if strings[0] != "" {
		return nil, fmt.Errorf("Lexicon binary object has invalid empty string sentinel")
	}
	return strings, nil
}

func decodeNodeSection(data []byte, strings []string) ([]nodeRecord, error) {
	reader := binaryObjectReader{data: data}
	count, err := reader.count("node records", maxBinaryRecords)
	if err != nil {
		return nil, err
	}
	records := make([]nodeRecord, 0, count)
	for range count {
		attributes, err := reader.attributes()
		if err != nil {
			return nil, err
		}
		contentID, err := reader.stringRef(strings, "node content ID")
		if err != nil {
			return nil, err
		}
		id, err := reader.stringRef(strings, "node ID")
		if err != nil {
			return nil, err
		}
		kind, err := reader.stringRef(strings, "node kind")
		if err != nil {
			return nil, err
		}
		name, err := reader.stringRef(strings, "node name")
		if err != nil {
			return nil, err
		}
		owner, err := reader.stringRef(strings, "node owner")
		if err != nil {
			return nil, err
		}
		path, err := reader.stringRef(strings, "node path")
		if err != nil {
			return nil, err
		}
		qualifiedName, err := reader.stringRef(strings, "node qualified name")
		if err != nil {
			return nil, err
		}
		span, err := reader.span(strings)
		if err != nil {
			return nil, err
		}
		records = append(records, nodeRecord{
			Attributes: attributes, ContentID: contentID, ID: id, Kind: kind,
			Name: name, Owner: owner, Path: path, QualifiedName: qualifiedName,
			Record: "node", Span: span,
		})
	}
	if reader.position != len(data) {
		return nil, fmt.Errorf("Lexicon node section has %d trailing bytes", len(data)-reader.position)
	}
	return records, nil
}

func decodeEdgeSection(data []byte, strings []string) ([]edgeRecord, error) {
	reader := binaryObjectReader{data: data}
	count, err := reader.count("edge records", maxBinaryRecords)
	if err != nil {
		return nil, err
	}
	records := make([]edgeRecord, 0, count)
	for range count {
		attributes, err := reader.attributes()
		if err != nil {
			return nil, err
		}
		owner, err := reader.stringRef(strings, "edge owner")
		if err != nil {
			return nil, err
		}
		relation, err := reader.stringRef(strings, "edge relation")
		if err != nil {
			return nil, err
		}
		source, err := reader.stringRef(strings, "edge source")
		if err != nil {
			return nil, err
		}
		span, err := reader.span(strings)
		if err != nil {
			return nil, err
		}
		target, err := reader.stringRef(strings, "edge target")
		if err != nil {
			return nil, err
		}
		records = append(records, edgeRecord{
			Attributes: attributes, Owner: owner, Record: "edge", Relation: relation,
			Source: source, Span: span, Target: target,
		})
	}
	if reader.position != len(data) {
		return nil, fmt.Errorf("Lexicon edge section has %d trailing bytes", len(data)-reader.position)
	}
	return records, nil
}

func decodeUnresolvedSection(data []byte, strings []string) ([]unresolvedRecord, error) {
	reader := binaryObjectReader{data: data}
	count, err := reader.count("unresolved records", maxBinaryRecords)
	if err != nil {
		return nil, err
	}
	records := make([]unresolvedRecord, 0, count)
	for range count {
		record, err := reader.unresolvedRecord(strings)
		if err != nil {
			return nil, err
		}
		records = append(records, record)
	}
	if reader.position != len(data) {
		return nil, fmt.Errorf("Lexicon unresolved section has %d trailing bytes", len(data)-reader.position)
	}
	return records, nil
}

func (reader *binaryObjectReader) unresolvedRecord(strings []string) (unresolvedRecord, error) {
	attributes, err := reader.attributes()
	if err != nil {
		return unresolvedRecord{}, err
	}
	fields := make([]string, 7)
	names := []string{
		"candidate name", "candidate namespace", "expression", "owner",
		"reason", "relation", "source",
	}
	for index, name := range names {
		fields[index], err = reader.stringRef(strings, "unresolved "+name)
		if err != nil {
			return unresolvedRecord{}, err
		}
	}
	span, err := reader.span(strings)
	if err != nil {
		return unresolvedRecord{}, err
	}
	return unresolvedRecord{
		Attributes: attributes, CandidateName: fields[0], CandidateNamespace: fields[1],
		Expression: fields[2], Owner: fields[3], Reason: fields[4], Record: "unresolved",
		Relation: fields[5], Source: fields[6], Span: span,
	}, nil
}

func (reader *binaryObjectReader) attributes() (json.RawMessage, error) {
	value, err := reader.bytes("record attributes", maxBinaryStringSize)
	if err != nil {
		return nil, err
	}
	attributes, err := compactOptionalJSON(value)
	if err != nil {
		return nil, fmt.Errorf("Lexicon binary object has invalid record attributes: %w", err)
	}
	return attributes, nil
}
