package objectstore

import (
	"bytes"
	"encoding/json"
	"fmt"
)

const (
	recordNode byte = iota + 1
	recordEdge
	recordUnresolved
)

type sourceSpan struct {
	EndColumn   uint64 `json:"end_column"`
	EndLine     uint64 `json:"end_line"`
	Path        string `json:"path"`
	StartColumn uint64 `json:"start_column"`
	StartLine   uint64 `json:"start_line"`
}

type nodeRecord struct {
	Attributes    json.RawMessage `json:"attributes,omitempty"`
	ContentID     string          `json:"content_id,omitempty"`
	ID            string          `json:"id"`
	Kind          string          `json:"kind"`
	Name          string          `json:"name"`
	Owner         string          `json:"owner,omitempty"`
	Path          string          `json:"path"`
	QualifiedName string          `json:"qualified_name"`
	Record        string          `json:"record"`
	Span          *sourceSpan     `json:"span,omitempty"`
}

type edgeRecord struct {
	Attributes json.RawMessage `json:"attributes,omitempty"`
	Owner      string          `json:"owner,omitempty"`
	Record     string          `json:"record"`
	Relation   string          `json:"relation"`
	Source     string          `json:"source"`
	Span       *sourceSpan     `json:"span,omitempty"`
	Target     string          `json:"target"`
}

type unresolvedRecord struct {
	Attributes         json.RawMessage `json:"attributes,omitempty"`
	CandidateName      string          `json:"candidate_name,omitempty"`
	CandidateNamespace string          `json:"candidate_namespace,omitempty"`
	Expression         string          `json:"expression"`
	Owner              string          `json:"owner,omitempty"`
	Reason             string          `json:"reason"`
	Record             string          `json:"record"`
	Relation           string          `json:"relation"`
	Source             string          `json:"source"`
	Span               *sourceSpan     `json:"span,omitempty"`
}

type wireRecord struct {
	Attributes         json.RawMessage `json:"attributes,omitempty"`
	CandidateName      string          `json:"candidate_name,omitempty"`
	CandidateNamespace string          `json:"candidate_namespace,omitempty"`
	ContentID          string          `json:"content_id,omitempty"`
	Expression         string          `json:"expression,omitempty"`
	ID                 string          `json:"id,omitempty"`
	Kind               string          `json:"kind,omitempty"`
	Name               string          `json:"name,omitempty"`
	Owner              string          `json:"owner,omitempty"`
	Path               string          `json:"path,omitempty"`
	QualifiedName      string          `json:"qualified_name,omitempty"`
	Reason             string          `json:"reason,omitempty"`
	Record             string          `json:"record"`
	Relation           string          `json:"relation,omitempty"`
	Source             string          `json:"source,omitempty"`
	Span               *sourceSpan     `json:"span,omitempty"`
	Target             string          `json:"target,omitempty"`
}

type typedRecord struct {
	kind       byte
	node       nodeRecord
	edge       edgeRecord
	unresolved unresolvedRecord
}

type typedRecords struct {
	nodes      []nodeRecord
	edges      []edgeRecord
	unresolved []unresolvedRecord
}

func parseTypedRecords(records []json.RawMessage) (typedRecords, error) {
	result := typedRecords{}
	for index, raw := range records {
		record, err := parseTypedRecord(raw)
		if err != nil {
			return typedRecords{}, fmt.Errorf("decode Lexicon record %d: %w", index, err)
		}
		result.append(record)
	}
	return result, nil
}

func parseTypedRecord(raw json.RawMessage) (typedRecord, error) {
	var wire wireRecord
	if err := json.Unmarshal(raw, &wire); err != nil {
		return typedRecord{}, err
	}
	attributes, err := compactOptionalJSON(wire.Attributes)
	if err != nil {
		return typedRecord{}, fmt.Errorf("decode %s attributes: %w", wire.Record, err)
	}
	switch wire.Record {
	case "node":
		return typedRecord{kind: recordNode, node: nodeRecord{
			Attributes: attributes, ContentID: wire.ContentID, ID: wire.ID,
			Kind: wire.Kind, Name: wire.Name, Owner: wire.Owner, Path: wire.Path,
			QualifiedName: wire.QualifiedName, Record: wire.Record, Span: wire.Span,
		}}, nil
	case "edge":
		return typedRecord{kind: recordEdge, edge: edgeRecord{
			Attributes: attributes, Owner: wire.Owner, Record: wire.Record,
			Relation: wire.Relation, Source: wire.Source, Span: wire.Span, Target: wire.Target,
		}}, nil
	case "unresolved":
		return typedRecord{kind: recordUnresolved, unresolved: unresolvedRecord{
			Attributes: attributes, CandidateName: wire.CandidateName,
			CandidateNamespace: wire.CandidateNamespace, Expression: wire.Expression,
			Owner: wire.Owner, Reason: wire.Reason, Record: wire.Record,
			Relation: wire.Relation, Source: wire.Source, Span: wire.Span,
		}}, nil
	default:
		return typedRecord{}, fmt.Errorf("unsupported Lexicon record %q", wire.Record)
	}
}

func (record typedRecord) ownership() rawRecord {
	switch record.kind {
	case recordNode:
		return rawRecord{
			Record: record.node.Record, Owner: record.node.Owner, Path: record.node.Path,
			Kind: record.node.Kind, Span: ownershipSpan(record.node.Span),
		}
	case recordEdge:
		return rawRecord{
			Record: record.edge.Record, Owner: record.edge.Owner, Source: record.edge.Source,
			Span: ownershipSpan(record.edge.Span),
		}
	case recordUnresolved:
		return rawRecord{
			Record: record.unresolved.Record, Owner: record.unresolved.Owner,
			Source: record.unresolved.Source, Span: ownershipSpan(record.unresolved.Span),
		}
	default:
		panic("ownership for unsupported typed Lexicon record")
	}
}

func ownershipSpan(span *sourceSpan) *struct {
	Path string `json:"path"`
} {
	if span == nil {
		return nil
	}
	return &struct {
		Path string `json:"path"`
	}{Path: span.Path}
}

func (record typedRecord) nodeID() string {
	if record.kind == recordNode {
		return record.node.ID
	}
	return ""
}

func (records *typedRecords) append(record typedRecord) {
	switch record.kind {
	case recordNode:
		records.nodes = append(records.nodes, record.node)
	case recordEdge:
		records.edges = append(records.edges, record.edge)
	case recordUnresolved:
		records.unresolved = append(records.unresolved, record.unresolved)
	default:
		panic("append unsupported typed Lexicon record")
	}
}

func (records *typedRecords) appendAll(other typedRecords) {
	records.nodes = append(records.nodes, other.nodes...)
	records.edges = append(records.edges, other.edges...)
	records.unresolved = append(records.unresolved, other.unresolved...)
}

func (records typedRecords) clone() typedRecords {
	return typedRecords{
		nodes:      append([]nodeRecord(nil), records.nodes...),
		edges:      append([]edgeRecord(nil), records.edges...),
		unresolved: append([]unresolvedRecord(nil), records.unresolved...),
	}
}

func (records typedRecords) len() int {
	return len(records.nodes) + len(records.edges) + len(records.unresolved)
}

func compactOptionalJSON(raw json.RawMessage) (json.RawMessage, error) {
	trimmed := bytes.TrimSpace(raw)
	if len(trimmed) == 0 || bytes.Equal(trimmed, []byte("null")) {
		return nil, nil
	}
	var compact bytes.Buffer
	if err := json.Compact(&compact, trimmed); err != nil {
		return nil, err
	}
	return append(json.RawMessage(nil), compact.Bytes()...), nil
}

func (records typedRecords) raw() ([]json.RawMessage, error) {
	result := make([]json.RawMessage, 0, records.len())
	appendRecord := func(value any) error {
		data, err := json.Marshal(value)
		if err != nil {
			return err
		}
		result = append(result, data)
		return nil
	}
	for index := range records.nodes {
		if err := appendRecord(records.nodes[index]); err != nil {
			return nil, err
		}
	}
	for index := range records.edges {
		if err := appendRecord(records.edges[index]); err != nil {
			return nil, err
		}
	}
	for index := range records.unresolved {
		if err := appendRecord(records.unresolved[index]); err != nil {
			return nil, err
		}
	}
	return result, nil
}
