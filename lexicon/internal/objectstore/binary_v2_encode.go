package objectstore

import (
	"bytes"
	"encoding/hex"
	"fmt"
)

func encodeBinaryObject(object FactObject) ([]byte, error) {
	var records typedRecords
	if object.typed != nil {
		records = *object.typed
	} else {
		parsed, err := parseTypedRecords(object.Records)
		if err != nil {
			return nil, err
		}
		records = parsed
	}
	table := newStringTable()
	collectCompactStrings(table, object, records)
	table.finalizeV2()
	ids := make(map[string]uint64, len(records.nodes))
	for i, n := range records.nodes {
		if n.ID != "" {
			ids[n.ID] = uint64(i)
		}
	}
	external := collectExternalReferences(records, ids)
	if len(external.values) > maxBinaryExternalReferences {
		return nil, fmt.Errorf("Lexicon binary object external reference count exceeds limit")
	}
	var out bytes.Buffer
	out.Write(binaryObjectMagic[:])
	writeUvarint(&out, uint64(object.Version))
	writeUvarint(&out, uint64(object.SchemaVersion))
	writeUvarint(&out, uint64(len(table.values)))
	for i, s := range table.values {
		previous := ""
		if i > 0 {
			previous = table.values[i-1]
		}
		prefix := commonPrefix([]byte(previous), []byte(s))
		writeUvarint(&out, uint64(prefix))
		writeBytes(&out, []byte(s[prefix:]))
	}
	writeStringRef(&out, table, object.Language)
	writeStringRef(&out, table, object.Owner)
	writeIdentity(&out, table, object.SourceContentID)
	writeStringRef(&out, table, object.AdapterVersion)
	writeIdentity(&out, table, object.AnalysisConfigID)
	writeUvarint(&out, uint64(len(external.values)))
	for _, value := range external.values {
		writeIdentity(&out, table, value)
	}
	writeSection(&out, encodeCompactNodes(table, object.Owner, records.nodes))
	writeSection(&out, encodeCompactEdges(table, object.Owner, ids, external.index, records.edges))
	writeSection(&out, encodeCompactUnresolved(table, object.Owner, ids, external.index, records.unresolved))
	return out.Bytes(), nil
}

const maxBinaryExternalReferences = 4_000_000

type externalReferences struct {
	values []string
	index  map[string]uint64
}

func collectExternalReferences(r typedRecords, ids map[string]uint64) externalReferences {
	x := externalReferences{index: make(map[string]uint64)}
	add := func(s string) {
		if _, ok := ids[s]; ok {
			return
		}
		if _, ok := x.index[s]; ok {
			return
		}
		x.index[s] = uint64(len(x.values))
		x.values = append(x.values, s)
	}
	for _, e := range r.edges {
		add(e.Source)
		add(e.Target)
	}
	for _, u := range r.unresolved {
		add(u.Source)
	}
	return x
}

func collectCompactStrings(table *stringTable, object FactObject, records typedRecords) {
	for _, value := range []string{object.Language, object.Owner, object.AdapterVersion} {
		table.intern(value)
	}
	for _, value := range []string{object.SourceContentID, object.AnalysisConfigID} {
		if !isSHA256(value) {
			table.intern(value)
		}
	}
	for _, node := range records.nodes {
		if !isSHA256(node.ContentID) {
			table.intern(node.ContentID)
		}
		if !isSHA256(node.ID) {
			table.intern(node.ID)
		}
		if nodeKindCode(node.Kind) == 0 {
			table.intern(node.Kind)
		}
		table.intern(node.Name)
		if node.Owner != "" && node.Owner != object.Owner {
			table.intern(node.Owner)
		}
		if node.Path != "" && node.Path != object.Owner {
			table.intern(node.Path)
		}
		if node.QualifiedName != node.Name &&
			node.QualifiedName != node.Path &&
			node.QualifiedName != object.Owner {
			table.intern(node.QualifiedName)
		}
		collectSpanString(table, node.Span)
	}
	localIDs := make(map[string]struct{}, len(records.nodes))
	for _, node := range records.nodes {
		localIDs[node.ID] = struct{}{}
	}
	for _, edge := range records.edges {
		if edge.Owner != "" && edge.Owner != object.Owner {
			table.intern(edge.Owner)
		}
		if relationCode(edge.Relation) == 0 {
			table.intern(edge.Relation)
		}
		for _, identity := range []string{edge.Source, edge.Target} {
			if _, local := localIDs[identity]; !local && !isSHA256(identity) {
				table.intern(identity)
			}
		}
		collectSpanString(table, edge.Span)
	}
	for _, unresolved := range records.unresolved {
		for _, value := range []string{
			unresolved.CandidateName, unresolved.CandidateNamespace,
			unresolved.Expression, unresolved.Reason,
		} {
			table.intern(value)
		}
		if unresolved.Owner != "" && unresolved.Owner != object.Owner {
			table.intern(unresolved.Owner)
		}
		if relationCode(unresolved.Relation) == 0 {
			table.intern(unresolved.Relation)
		}
		if _, local := localIDs[unresolved.Source]; !local && !isSHA256(unresolved.Source) {
			table.intern(unresolved.Source)
		}
		collectSpanString(table, unresolved.Span)
	}
}

func isSHA256(s string) bool {
	if len(s) != 71 || s[:7] != "sha256:" {
		return false
	}
	for _, digit := range s[7:] {
		if (digit < '0' || digit > '9') && (digit < 'a' || digit > 'f') {
			return false
		}
	}
	return true
}
func writeIdentity(out *bytes.Buffer, t *stringTable, s string) {
	if isSHA256(s) {
		out.WriteByte(1)
		raw, _ := hex.DecodeString(s[7:])
		out.Write(raw)
	} else {
		out.WriteByte(0)
		writeStringRef(out, t, s)
	}
}
func writeNodeRef(out *bytes.Buffer, ids, external map[string]uint64, s string) {
	if n, ok := ids[s]; ok {
		out.WriteByte(0)
		writeUvarint(out, n+1)
	} else {
		out.WriteByte(1)
		writeUvarint(out, external[s]+1)
	}
}

func encodeCompactNodes(t *stringTable, objectOwner string, rs []nodeRecord) []byte {
	var b bytes.Buffer
	writeUvarint(&b, uint64(len(rs)))
	for _, n := range rs {
		writeBytes(&b, n.Attributes)
		writeIdentity(&b, t, n.ContentID)
		writeIdentity(&b, t, n.ID)
		writeCodeOrString(&b, t, n.Kind, nodeKindCode)
		writeStringRef(&b, t, n.Name)
		writeFactored(&b, t, n.Owner, objectOwner)
		writeFactored(&b, t, n.Path, objectOwner)
		writeQName(&b, t, n.QualifiedName, n.Name, n.Path, objectOwner)
		writeSpan(&b, t, n.Span)
	}
	return b.Bytes()
}
func encodeCompactEdges(t *stringTable, objectOwner string, ids, external map[string]uint64, rs []edgeRecord) []byte {
	var b bytes.Buffer
	writeUvarint(&b, uint64(len(rs)))
	for _, e := range rs {
		writeBytes(&b, e.Attributes)
		writeFactored(&b, t, e.Owner, objectOwner)
		writeCodeOrString(&b, t, e.Relation, relationCode)
		writeNodeRef(&b, ids, external, e.Source)
		writeSpan(&b, t, e.Span)
		writeNodeRef(&b, ids, external, e.Target)
	}
	return b.Bytes()
}
func encodeCompactUnresolved(t *stringTable, objectOwner string, ids, external map[string]uint64, rs []unresolvedRecord) []byte {
	var b bytes.Buffer
	writeUvarint(&b, uint64(len(rs)))
	for _, u := range rs {
		writeBytes(&b, u.Attributes)
		for _, s := range []string{u.CandidateName, u.CandidateNamespace, u.Expression} {
			writeStringRef(&b, t, s)
		}
		writeFactored(&b, t, u.Owner, objectOwner)
		writeStringRef(&b, t, u.Reason)
		writeCodeOrString(&b, t, u.Relation, relationCode)
		writeNodeRef(&b, ids, external, u.Source)
		writeSpan(&b, t, u.Span)
	}
	return b.Bytes()
}
