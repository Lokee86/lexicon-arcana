package objectstore

import "bytes"

func encodeBinaryObjectV1(object FactObject) ([]byte, error) {
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
	collectObjectStrings(table, object, records)

	var output bytes.Buffer
	_, _ = output.Write(binaryObjectMagic[:])
	writeUvarint(&output, uint64(object.Version))
	writeUvarint(&output, uint64(object.SchemaVersion))
	writeUvarint(&output, uint64(len(table.values)))
	for _, value := range table.values {
		writeBytes(&output, []byte(value))
	}
	writeStringRef(&output, table, object.Language)
	writeStringRef(&output, table, object.Owner)
	writeStringRef(&output, table, object.SourceContentID)
	writeStringRef(&output, table, object.AdapterVersion)
	writeStringRef(&output, table, object.AnalysisConfigID)

	writeSection(&output, encodeNodeSection(table, records.nodes))
	writeSection(&output, encodeEdgeSection(table, records.edges))
	writeSection(&output, encodeUnresolvedSection(table, records.unresolved))
	return output.Bytes(), nil
}

func writeSection(output *bytes.Buffer, section []byte) {
	writeBytes(output, section)
}

func encodeNodeSection(table *stringTable, records []nodeRecord) []byte {
	var section bytes.Buffer
	writeUvarint(&section, uint64(len(records)))
	for _, record := range records {
		writeBytes(&section, record.Attributes)
		writeStringRef(&section, table, record.ContentID)
		writeStringRef(&section, table, record.ID)
		writeStringRef(&section, table, record.Kind)
		writeStringRef(&section, table, record.Name)
		writeStringRef(&section, table, record.Owner)
		writeStringRef(&section, table, record.Path)
		writeStringRef(&section, table, record.QualifiedName)
		writeSpan(&section, table, record.Span)
	}
	return section.Bytes()
}

func encodeEdgeSection(table *stringTable, records []edgeRecord) []byte {
	var section bytes.Buffer
	writeUvarint(&section, uint64(len(records)))
	for _, record := range records {
		writeBytes(&section, record.Attributes)
		writeStringRef(&section, table, record.Owner)
		writeStringRef(&section, table, record.Relation)
		writeStringRef(&section, table, record.Source)
		writeSpan(&section, table, record.Span)
		writeStringRef(&section, table, record.Target)
	}
	return section.Bytes()
}

func encodeUnresolvedSection(table *stringTable, records []unresolvedRecord) []byte {
	var section bytes.Buffer
	writeUvarint(&section, uint64(len(records)))
	for _, record := range records {
		writeBytes(&section, record.Attributes)
		writeStringRef(&section, table, record.CandidateName)
		writeStringRef(&section, table, record.CandidateNamespace)
		writeStringRef(&section, table, record.Expression)
		writeStringRef(&section, table, record.Owner)
		writeStringRef(&section, table, record.Reason)
		writeStringRef(&section, table, record.Relation)
		writeStringRef(&section, table, record.Source)
		writeSpan(&section, table, record.Span)
	}
	return section.Bytes()
}
