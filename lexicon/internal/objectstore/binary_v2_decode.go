package objectstore

import (
	"bytes"
	"encoding/hex"
	"fmt"
)

func readIdentity(r *binaryObjectReader, ss []string, field string) (string, error) {
	tag, e := r.byte(field + " tag")
	if e != nil {
		return "", e
	}
	if tag == 0 {
		return r.stringRef(ss, field)
	}
	if tag != 1 {
		return "", fmt.Errorf("Lexicon binary object invalid %s tag %d", field, tag)
	}
	raw, e := r.take(32, field)
	if e != nil {
		return "", e
	}
	return "sha256:" + hex.EncodeToString(raw), nil
}
func (r *binaryObjectReader) take(n int, field string) ([]byte, error) {
	if n < 0 || r.position+n > len(r.data) {
		return nil, fmt.Errorf("Lexicon binary object is truncated in %s", field)
	}
	v := r.data[r.position : r.position+n]
	r.position += n
	return v, nil
}
func readNodeRef(r *binaryObjectReader, external []string, nodes []nodeRecord, field string) (string, error) {
	tag, e := r.byte(field + " tag")
	if e != nil {
		return "", e
	}
	if tag == 0 {
		n, e := r.uvarint(field + " ordinal")
		if e != nil {
			return "", e
		}
		if n == 0 || n > uint64(len(nodes)) {
			return "", fmt.Errorf("Lexicon binary object %s ordinal out of range", field)
		}
		return nodes[n-1].ID, nil
	}
	if tag != 1 {
		return "", fmt.Errorf("Lexicon binary object invalid %s tag %d", field, tag)
	}
	n, e := r.uvarint(field + " external index")
	if e != nil {
		return "", e
	}
	if n == 0 || n > uint64(len(external)) {
		return "", fmt.Errorf("Lexicon binary object %s external index out of range", field)
	}
	return external[n-1], nil
}

func decodeBinaryObject(data []byte) (FactObject, error) {
	if len(data) < 8 {
		return FactObject{}, fmt.Errorf("invalid Lexicon binary object magic")
	}
	if bytes.Equal(data[:8], legacyBinaryObjectMagic[:]) {
		return decodeBinaryObjectV1(data)
	}
	if !bytes.Equal(data[:8], binaryObjectMagic[:]) {
		return FactObject{}, fmt.Errorf("invalid Lexicon binary object magic")
	}
	r := binaryObjectReader{data: data, position: 8}
	version, e := r.uvarint("object version")
	if e != nil {
		return FactObject{}, e
	}
	schema, e := r.uvarint("schema version")
	if e != nil {
		return FactObject{}, e
	}
	ss, e := r.stringTableV2()
	if e != nil {
		return FactObject{}, e
	}
	language, e := r.stringRef(ss, "language")
	if e != nil {
		return FactObject{}, e
	}
	owner, e := r.stringRef(ss, "owner")
	if e != nil {
		return FactObject{}, e
	}
	content, e := readIdentity(&r, ss, "source content ID")
	if e != nil {
		return FactObject{}, e
	}
	adapter, e := r.stringRef(ss, "adapter version")
	if e != nil {
		return FactObject{}, e
	}
	config, e := readIdentity(&r, ss, "analysis config ID")
	if e != nil {
		return FactObject{}, e
	}
	externalCount, e := r.count("external references", maxBinaryExternalReferences)
	if e != nil {
		return FactObject{}, e
	}
	external := make([]string, externalCount)
	for i := range external {
		external[i], e = readIdentity(&r, ss, fmt.Sprintf("external reference %d", i))
		if e != nil {
			return FactObject{}, e
		}
	}
	ns, e := r.bytes("node section", maxBinarySectionSize)
	if e != nil {
		return FactObject{}, e
	}
	es, e := r.bytes("edge section", maxBinarySectionSize)
	if e != nil {
		return FactObject{}, e
	}
	us, e := r.bytes("unresolved section", maxBinarySectionSize)
	if e != nil {
		return FactObject{}, e
	}
	if r.position != len(data) {
		return FactObject{}, fmt.Errorf("Lexicon binary object has %d trailing bytes", len(data)-r.position)
	}
	nodes, e := decodeCompactNodes(ns, ss, owner)
	if e != nil {
		return FactObject{}, e
	}
	edges, e := decodeCompactEdges(es, ss, external, nodes, owner)
	if e != nil {
		return FactObject{}, e
	}
	unresolved, e := decodeCompactUnresolved(us, ss, external, nodes, owner)
	if e != nil {
		return FactObject{}, e
	}
	records := typedRecords{nodes: nodes, edges: edges, unresolved: unresolved}
	raw, e := records.raw()
	if e != nil {
		return FactObject{}, e
	}
	return FactObject{Version: int(version), Language: language, Owner: owner, SourceContentID: content, AdapterVersion: adapter, SchemaVersion: int(schema), AnalysisConfigID: config, Records: raw}, nil
}
func decodeCompactNodes(
	data []byte, strings []string, objectOwner string,
) ([]nodeRecord, error) {
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
		contentID, err := readIdentity(&reader, strings, "node content ID")
		if err != nil {
			return nil, err
		}
		id, err := readIdentity(&reader, strings, "node ID")
		if err != nil {
			return nil, err
		}
		kind, err := readCodeOrString(&reader, strings, commonNodeKinds, "node kind")
		if err != nil {
			return nil, err
		}
		name, err := reader.stringRef(strings, "node name")
		if err != nil {
			return nil, err
		}
		owner, err := readFactored(&reader, strings, objectOwner, "node owner")
		if err != nil {
			return nil, err
		}
		path, err := readFactored(&reader, strings, objectOwner, "node path")
		if err != nil {
			return nil, err
		}
		qualifiedName, err := readQName(
			&reader, strings, name, path, objectOwner, "node qualified name",
		)
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
		return nil, fmt.Errorf(
			"Lexicon node section has %d trailing bytes", len(data)-reader.position,
		)
	}
	return records, nil
}
