package objectstore

import (
	"bytes"
	"encoding/binary"
	"fmt"
)

var binaryObjectMagic = [8]byte{'L', 'X', 'O', 'B', 'J', 0, 2, 0}
var legacyBinaryObjectMagic = [8]byte{'L', 'X', 'O', 'B', 'J', 0, 1, 0}

const (
	maxBinaryStrings     = 4_000_000
	maxBinaryRecords     = 20_000_000
	maxBinaryStringSize  = 32 * 1024 * 1024
	maxBinarySectionSize = 512 * 1024 * 1024
)

type stringTable struct {
	values []string
	index  map[string]uint64
}

func newStringTable() *stringTable {
	return &stringTable{values: []string{""}, index: map[string]uint64{"": 0}}
}

func (table *stringTable) intern(value string) uint64 {
	if index, ok := table.index[value]; ok {
		return index
	}
	index := uint64(len(table.values))
	table.values = append(table.values, value)
	table.index[value] = index
	return index
}

func collectObjectStrings(table *stringTable, object FactObject, records typedRecords) {
	for _, value := range []string{
		object.Language,
		object.Owner,
		object.SourceContentID,
		object.AdapterVersion,
		object.AnalysisConfigID,
	} {
		table.intern(value)
	}
	for _, record := range records.nodes {
		for _, value := range []string{
			record.ContentID,
			record.ID,
			record.Kind,
			record.Name,
			record.Owner,
			record.Path,
			record.QualifiedName,
		} {
			table.intern(value)
		}
		collectSpanString(table, record.Span)
	}
	for _, record := range records.edges {
		for _, value := range []string{
			record.Owner,
			record.Relation,
			record.Source,
			record.Target,
		} {
			table.intern(value)
		}
		collectSpanString(table, record.Span)
	}
	for _, record := range records.unresolved {
		for _, value := range []string{
			record.CandidateName,
			record.CandidateNamespace,
			record.Expression,
			record.Owner,
			record.Reason,
			record.Relation,
			record.Source,
		} {
			table.intern(value)
		}
		collectSpanString(table, record.Span)
	}
}

func collectSpanString(table *stringTable, span *sourceSpan) {
	if span != nil {
		table.intern(span.Path)
	}
}

func writeUvarint(output *bytes.Buffer, value uint64) {
	var encoded [binary.MaxVarintLen64]byte
	count := binary.PutUvarint(encoded[:], value)
	_, _ = output.Write(encoded[:count])
}

func writeBytes(output *bytes.Buffer, value []byte) {
	writeUvarint(output, uint64(len(value)))
	_, _ = output.Write(value)
}

func writeStringRef(output *bytes.Buffer, table *stringTable, value string) {
	index, ok := table.index[value]
	if !ok {
		panic("Lexicon binary encoder referenced an uninterned string")
	}
	writeUvarint(output, index)
}

func writeSpan(output *bytes.Buffer, table *stringTable, span *sourceSpan) {
	if span == nil {
		_ = output.WriteByte(0)
		return
	}
	_ = output.WriteByte(1)
	writeStringRef(output, table, span.Path)
	writeUvarint(output, span.StartLine)
	writeUvarint(output, span.StartColumn)
	writeUvarint(output, span.EndLine)
	writeUvarint(output, span.EndColumn)
}

type binaryObjectReader struct {
	data     []byte
	position int
}

func (reader *binaryObjectReader) magic() bool {
	if len(reader.data) < len(binaryObjectMagic) || (!bytes.Equal(reader.data[:len(binaryObjectMagic)], binaryObjectMagic[:]) && !bytes.Equal(reader.data[:len(binaryObjectMagic)], legacyBinaryObjectMagic[:])) {
		return false
	}
	reader.position = len(binaryObjectMagic)
	return true
}

func (reader *binaryObjectReader) uvarint(field string) (uint64, error) {
	if reader.position >= len(reader.data) {
		return 0, fmt.Errorf("Lexicon binary object is truncated before %s", field)
	}
	value, count := binary.Uvarint(reader.data[reader.position:])
	if count <= 0 {
		return 0, fmt.Errorf("Lexicon binary object has invalid %s varint", field)
	}
	reader.position += count
	return value, nil
}

func (reader *binaryObjectReader) count(field string, maximum uint64) (int, error) {
	value, err := reader.uvarint(field)
	if err != nil {
		return 0, err
	}
	if value > maximum {
		return 0, fmt.Errorf("Lexicon binary object %s count %d exceeds limit", field, value)
	}
	return int(value), nil
}

func (reader *binaryObjectReader) bytes(field string, maximum uint64) ([]byte, error) {
	length, err := reader.uvarint(field + " length")
	if err != nil {
		return nil, err
	}
	if length > maximum {
		return nil, fmt.Errorf("Lexicon binary object %s length %d exceeds limit", field, length)
	}
	end := reader.position + int(length)
	if end < reader.position || end > len(reader.data) {
		return nil, fmt.Errorf("Lexicon binary object is truncated in %s", field)
	}
	value := reader.data[reader.position:end]
	reader.position = end
	return value, nil
}

func (reader *binaryObjectReader) byte(field string) (byte, error) {
	if reader.position >= len(reader.data) {
		return 0, fmt.Errorf("Lexicon binary object is truncated before %s", field)
	}
	value := reader.data[reader.position]
	reader.position++
	return value, nil
}

func (reader *binaryObjectReader) stringRef(strings []string, field string) (string, error) {
	index, err := reader.uvarint(field)
	if err != nil {
		return "", err
	}
	if index >= uint64(len(strings)) {
		return "", fmt.Errorf("Lexicon binary object %s string index %d is out of range", field, index)
	}
	return strings[index], nil
}

func (reader *binaryObjectReader) span(strings []string) (*sourceSpan, error) {
	present, err := reader.byte("span flag")
	if err != nil {
		return nil, err
	}
	if present == 0 {
		return nil, nil
	}
	if present != 1 {
		return nil, fmt.Errorf("Lexicon binary object has invalid span flag %d", present)
	}
	path, err := reader.stringRef(strings, "span path")
	if err != nil {
		return nil, err
	}
	startLine, err := reader.uvarint("span start line")
	if err != nil {
		return nil, err
	}
	startColumn, err := reader.uvarint("span start column")
	if err != nil {
		return nil, err
	}
	endLine, err := reader.uvarint("span end line")
	if err != nil {
		return nil, err
	}
	endColumn, err := reader.uvarint("span end column")
	if err != nil {
		return nil, err
	}
	return &sourceSpan{
		EndColumn: endColumn, EndLine: endLine, Path: path,
		StartColumn: startColumn, StartLine: startLine,
	}, nil
}

func isBinaryObject(data []byte) bool {
	return len(data) >= len(binaryObjectMagic) && (bytes.Equal(data[:len(binaryObjectMagic)], binaryObjectMagic[:]) || bytes.Equal(data[:len(binaryObjectMagic)], legacyBinaryObjectMagic[:]))
}
