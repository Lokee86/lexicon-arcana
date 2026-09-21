package objectstore

import "fmt"

func decodeCompactEdges(
	data []byte,
	strings, external []string,
	nodes []nodeRecord,
	objectOwner string,
) ([]edgeRecord, error) {
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
		owner, err := readFactored(&reader, strings, objectOwner, "edge owner")
		if err != nil {
			return nil, err
		}
		relation, err := readCodeOrString(
			&reader, strings, commonRelations, "edge relation",
		)
		if err != nil {
			return nil, err
		}
		source, err := readNodeRef(&reader, external, nodes, "edge source")
		if err != nil {
			return nil, err
		}
		span, err := reader.span(strings)
		if err != nil {
			return nil, err
		}
		target, err := readNodeRef(&reader, external, nodes, "edge target")
		if err != nil {
			return nil, err
		}
		records = append(records, edgeRecord{
			Attributes: attributes, Owner: owner, Record: "edge", Relation: relation,
			Source: source, Span: span, Target: target,
		})
	}
	if reader.position != len(data) {
		return nil, fmt.Errorf(
			"Lexicon edge section has %d trailing bytes", len(data)-reader.position,
		)
	}
	return records, nil
}

func decodeCompactUnresolved(
	data []byte,
	strings, external []string,
	nodes []nodeRecord,
	objectOwner string,
) ([]unresolvedRecord, error) {
	reader := binaryObjectReader{data: data}
	count, err := reader.count("unresolved records", maxBinaryRecords)
	if err != nil {
		return nil, err
	}
	records := make([]unresolvedRecord, 0, count)
	for range count {
		attributes, err := reader.attributes()
		if err != nil {
			return nil, err
		}
		candidateName, err := reader.stringRef(strings, "unresolved candidate name")
		if err != nil {
			return nil, err
		}
		candidateNamespace, err := reader.stringRef(
			strings, "unresolved candidate namespace",
		)
		if err != nil {
			return nil, err
		}
		expression, err := reader.stringRef(strings, "unresolved expression")
		if err != nil {
			return nil, err
		}
		owner, err := readFactored(&reader, strings, objectOwner, "unresolved owner")
		if err != nil {
			return nil, err
		}
		reason, err := reader.stringRef(strings, "unresolved reason")
		if err != nil {
			return nil, err
		}
		relation, err := readCodeOrString(
			&reader, strings, commonRelations, "unresolved relation",
		)
		if err != nil {
			return nil, err
		}
		source, err := readNodeRef(&reader, external, nodes, "unresolved source")
		if err != nil {
			return nil, err
		}
		span, err := reader.span(strings)
		if err != nil {
			return nil, err
		}
		records = append(records, unresolvedRecord{
			Attributes: attributes, CandidateName: candidateName,
			CandidateNamespace: candidateNamespace, Expression: expression,
			Owner: owner, Reason: reason, Record: "unresolved",
			Relation: relation, Source: source, Span: span,
		})
	}
	if reader.position != len(data) {
		return nil, fmt.Errorf(
			"Lexicon unresolved section has %d trailing bytes", len(data)-reader.position,
		)
	}
	return records, nil
}
