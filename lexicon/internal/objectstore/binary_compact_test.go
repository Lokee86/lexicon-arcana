package objectstore

import (
	"encoding/json"
	"fmt"
	"testing"
)

const externalFixtureID = "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"

func TestBinaryObjectExternalReferencesAreDeduplicated(t *testing.T) {
	object := binaryFixture(2)
	for index := 0; index < 2; index++ {
		object.Records = append(object.Records, json.RawMessage(fmt.Sprintf(
			`{"owner":"src/main.go","record":"edge","relation":"calls","source":%q,"target":%q}`,
			binaryFixtureNodeID(index), externalFixtureID,
		)))
	}
	encoded, err := encodeBinaryObject(object)
	if err != nil {
		t.Fatal(err)
	}
	reader, strings := binaryV2ReaderAtExternalTable(t, encoded)
	count, err := reader.count("external references", maxBinaryExternalReferences)
	if err != nil {
		t.Fatal(err)
	}
	if count != 1 {
		t.Fatalf("external reference count = %d, want 1", count)
	}
	value, err := readIdentity(&reader, strings, "external reference")
	if err != nil {
		t.Fatal(err)
	}
	if value != externalFixtureID {
		t.Fatalf("external reference = %q, want %q", value, externalFixtureID)
	}

	decoded, err := decodeBinaryObject(encoded)
	if err != nil {
		t.Fatal(err)
	}
	typed, err := parseTypedRecords(object.Records)
	if err != nil {
		t.Fatal(err)
	}
	object.Records, err = typed.raw()
	if err != nil {
		t.Fatal(err)
	}
	assertObjectEquivalent(t, decoded, object)
}

func TestBinaryObjectRejectsExternalReferenceIndexOutOfRange(t *testing.T) {
	object := binaryFixture(1)
	object.Records = append(object.Records, json.RawMessage(fmt.Sprintf(
		`{"owner":"src/main.go","record":"edge","relation":"calls","source":%q,"target":%q}`,
		binaryFixtureNodeID(0), externalFixtureID,
	)))
	encoded, err := encodeBinaryObject(object)
	if err != nil {
		t.Fatal(err)
	}

	reader, strings := binaryV2ReaderAtExternalTable(t, encoded)
	count, err := reader.count("external references", maxBinaryExternalReferences)
	if err != nil {
		t.Fatal(err)
	}
	for index := 0; index < count; index++ {
		if _, err := readIdentity(&reader, strings, "external reference"); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := reader.bytes("node section", maxBinarySectionSize); err != nil {
		t.Fatal(err)
	}
	edgeSection, err := reader.bytes("edge section", maxBinarySectionSize)
	if err != nil {
		t.Fatal(err)
	}
	edgeStart := reader.position - len(edgeSection)

	edgeReader := binaryObjectReader{data: edgeSection}
	if _, err := edgeReader.count("edge records", maxBinaryRecords); err != nil {
		t.Fatal(err)
	}
	if _, err := edgeReader.attributes(); err != nil {
		t.Fatal(err)
	}
	if _, err := readFactored(&edgeReader, strings, object.Owner, "edge owner"); err != nil {
		t.Fatal(err)
	}
	if _, err := readCodeOrString(&edgeReader, strings, commonRelations, "edge relation"); err != nil {
		t.Fatal(err)
	}
	sourceTag, err := edgeReader.byte("edge source tag")
	if err != nil {
		t.Fatal(err)
	}
	if sourceTag != 0 {
		t.Fatalf("edge source tag = %d, want local ordinal", sourceTag)
	}
	if _, err := edgeReader.uvarint("edge source ordinal"); err != nil {
		t.Fatal(err)
	}
	spanFlag, err := edgeReader.byte("span flag")
	if err != nil {
		t.Fatal(err)
	}
	if spanFlag != 0 {
		t.Fatalf("span flag = %d, want 0", spanFlag)
	}
	targetTag, err := edgeReader.byte("edge target tag")
	if err != nil {
		t.Fatal(err)
	}
	if targetTag != 1 {
		t.Fatalf("edge target tag = %d, want external reference", targetTag)
	}
	targetIndexOffset := edgeReader.position

	corrupt := append([]byte(nil), encoded...)
	corrupt[edgeStart+targetIndexOffset] = 2
	if _, err := decodeBinaryObject(corrupt); err == nil {
		t.Fatal("expected out-of-range external reference error")
	}
}

func TestBinaryObjectPreservesNonCanonicalSHA256LikeStrings(t *testing.T) {
	object := binaryFixture(1)
	object.AnalysisConfigID = "sha256:CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC"
	encoded, err := encodeBinaryObject(object)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := decodeBinaryObject(encoded)
	if err != nil {
		t.Fatal(err)
	}
	if decoded.AnalysisConfigID != object.AnalysisConfigID {
		t.Fatalf("analysis config ID = %q, want %q", decoded.AnalysisConfigID, object.AnalysisConfigID)
	}
}

func binaryV2ReaderAtExternalTable(t *testing.T, data []byte) (binaryObjectReader, []string) {
	t.Helper()
	reader := binaryObjectReader{data: data, position: len(binaryObjectMagic)}
	if _, err := reader.uvarint("object version"); err != nil {
		t.Fatal(err)
	}
	if _, err := reader.uvarint("schema version"); err != nil {
		t.Fatal(err)
	}
	strings, err := reader.stringTableV2()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := reader.stringRef(strings, "language"); err != nil {
		t.Fatal(err)
	}
	if _, err := reader.stringRef(strings, "owner"); err != nil {
		t.Fatal(err)
	}
	if _, err := readIdentity(&reader, strings, "source content ID"); err != nil {
		t.Fatal(err)
	}
	if _, err := reader.stringRef(strings, "adapter version"); err != nil {
		t.Fatal(err)
	}
	if _, err := readIdentity(&reader, strings, "analysis config ID"); err != nil {
		t.Fatal(err)
	}
	return reader, strings
}
