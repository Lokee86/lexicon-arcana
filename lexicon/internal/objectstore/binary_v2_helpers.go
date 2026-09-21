package objectstore

import (
	"bytes"
	"fmt"
	"sort"
)

var commonNodeKinds = []string{
	"repository", "directory", "file", "module", "namespace", "symbol",
	"type", "interface", "protocol", "trait", "function", "method",
	"constructor", "field", "variable", "constant", "parameter", "import",
	"test", "http-endpoint", "message-channel", "config-key",
}

var commonRelations = []string{
	"contains", "defines", "imports", "calls", "possible-calls", "passes-to",
	"converts-to", "references", "extends", "implements", "uses-trait",
	"overrides", "reads", "writes", "annotates", "includes", "depends-on",
	"tests", "documents", "generates", "calls-endpoint", "handled-by",
	"publishes", "consumes", "reads-config",
}

func codeMap(values []string) map[string]uint64 {
	result := make(map[string]uint64, len(values))
	for index, value := range values {
		result[value] = uint64(index + 1)
	}
	return result
}

var nodeKindCodes = codeMap(commonNodeKinds)
var relationCodes = codeMap(commonRelations)

func nodeKindCode(value string) uint64 { return nodeKindCodes[value] }
func relationCode(value string) uint64 { return relationCodes[value] }

func commonPrefix(left, right []byte) int {
	limit := len(left)
	if len(right) < limit {
		limit = len(right)
	}
	index := 0
	for index < limit && left[index] == right[index] {
		index++
	}
	return index
}

func (table *stringTable) finalizeV2() {
	sort.Strings(table.values[1:])
	table.index = make(map[string]uint64, len(table.values))
	for index, value := range table.values {
		table.index[value] = uint64(index)
	}
}

func writeCodeOrString(
	output *bytes.Buffer, table *stringTable, value string, code func(string) uint64,
) {
	if encoded := code(value); encoded != 0 {
		writeUvarint(output, encoded)
		return
	}
	writeUvarint(output, 0)
	writeStringRef(output, table, value)
}

func readCodeOrString(
	reader *binaryObjectReader, strings, values []string, field string,
) (string, error) {
	code, err := reader.uvarint(field + " code")
	if err != nil {
		return "", err
	}
	if code == 0 {
		return reader.stringRef(strings, field)
	}
	if code > uint64(len(values)) {
		return "", fmt.Errorf("Lexicon binary object %s code %d is out of range", field, code)
	}
	return values[code-1], nil
}

// Factored owner/path: 0 empty, 1 object owner, 2 string fallback.
func writeFactored(
	output *bytes.Buffer, table *stringTable, value, objectOwner string,
) {
	switch {
	case value == "":
		writeUvarint(output, 0)
	case value == objectOwner:
		writeUvarint(output, 1)
	default:
		writeUvarint(output, 2)
		writeStringRef(output, table, value)
	}
}

func readFactored(
	reader *binaryObjectReader, strings []string, objectOwner, field string,
) (string, error) {
	tag, err := reader.uvarint(field + " factor")
	if err != nil {
		return "", err
	}
	switch tag {
	case 0:
		return "", nil
	case 1:
		return objectOwner, nil
	case 2:
		return reader.stringRef(strings, field)
	default:
		return "", fmt.Errorf("Lexicon binary object has invalid %s factor %d", field, tag)
	}
}

// Qualified name: 0 string fallback, 1 node name, 2 node path, 3 object owner.
func writeQName(
	output *bytes.Buffer, table *stringTable, value, name, path, objectOwner string,
) {
	switch {
	case value == name:
		writeUvarint(output, 1)
	case value == path:
		writeUvarint(output, 2)
	case value == objectOwner:
		writeUvarint(output, 3)
	default:
		writeUvarint(output, 0)
		writeStringRef(output, table, value)
	}
}

func readQName(
	reader *binaryObjectReader,
	strings []string,
	name, path, objectOwner, field string,
) (string, error) {
	tag, err := reader.uvarint(field + " factor")
	if err != nil {
		return "", err
	}
	switch tag {
	case 0:
		return reader.stringRef(strings, field)
	case 1:
		return name, nil
	case 2:
		return path, nil
	case 3:
		return objectOwner, nil
	default:
		return "", fmt.Errorf("Lexicon binary object has invalid %s factor %d", field, tag)
	}
}
