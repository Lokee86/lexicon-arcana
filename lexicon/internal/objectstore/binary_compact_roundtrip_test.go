package objectstore

import (
	"encoding/json"
	"fmt"
	"testing"
)

func TestBinaryObjectUnknownKindAndRelationFallbackRoundTrip(t *testing.T) {
	object := FactObject{
		Version: ObjectVersion, Language: "custom", Owner: "src/custom.lang",
		SourceContentID:  "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		AdapterVersion:   "1.0.0",
		SchemaVersion:    1,
		AnalysisConfigID: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
		Records: []json.RawMessage{
			json.RawMessage(fmt.Sprintf(
				`{"id":%q,"kind":"language-specific-kind","name":"root","owner":"src/custom.lang","path":"src/custom.lang","qualified_name":"root","record":"node"}`,
				binaryFixtureNodeID(0),
			)),
			json.RawMessage(fmt.Sprintf(
				`{"id":%q,"kind":"function","name":"child","owner":"src/custom.lang","path":"src/custom.lang","qualified_name":"child","record":"node"}`,
				binaryFixtureNodeID(1),
			)),
			json.RawMessage(fmt.Sprintf(
				`{"owner":"src/custom.lang","record":"edge","relation":"custom-relation","source":%q,"target":%q}`,
				binaryFixtureNodeID(0), binaryFixtureNodeID(1),
			)),
			json.RawMessage(fmt.Sprintf(
				`{"expression":"dynamic","owner":"src/custom.lang","reason":"dynamic-target","record":"unresolved","relation":"custom-unresolved","source":%q}`,
				binaryFixtureNodeID(1),
			)),
		},
	}
	encoded, err := encodeBinaryObject(object)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := decodeBinaryObject(encoded)
	if err != nil {
		t.Fatal(err)
	}
	assertObjectEquivalent(t, decoded, object)
}

func TestBinaryObjectFactoredOwnerPathAndQualifiedNameRoundTrip(t *testing.T) {
	object := FactObject{
		Version: ObjectVersion, Language: "go", Owner: "src/main.go",
		SourceContentID:  "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		AdapterVersion:   "1.0.0",
		SchemaVersion:    1,
		AnalysisConfigID: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
		Records: []json.RawMessage{
			json.RawMessage(fmt.Sprintf(
				`{"id":%q,"kind":"function","name":"same-name","path":"","qualified_name":"same-name","record":"node"}`,
				binaryFixtureNodeID(0),
			)),
			json.RawMessage(fmt.Sprintf(
				`{"id":%q,"kind":"function","name":"same-path","owner":"src/main.go","path":"src/main.go","qualified_name":"src/main.go","record":"node"}`,
				binaryFixtureNodeID(1),
			)),
			json.RawMessage(fmt.Sprintf(
				`{"id":%q,"kind":"function","name":"owner-qname","owner":"other.go","path":"other.go","qualified_name":"src/main.go","record":"node"}`,
				binaryFixtureNodeID(2),
			)),
			json.RawMessage(fmt.Sprintf(
				`{"id":%q,"kind":"function","name":"fallback","owner":"other.go","path":"pkg/other.go","qualified_name":"pkg.fallback","record":"node"}`,
				binaryFixtureNodeID(3),
			)),
		},
	}
	encoded, err := encodeBinaryObject(object)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := decodeBinaryObject(encoded)
	if err != nil {
		t.Fatal(err)
	}
	assertObjectEquivalent(t, decoded, object)
}

func TestBinaryObjectFrontCodedStringTableRoundTripsUnicode(t *testing.T) {
	object := FactObject{
		Version: ObjectVersion, Language: "go", Owner: "src/ümlaut.go",
		SourceContentID:  "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		AdapterVersion:   "βeta",
		SchemaVersion:    1,
		AnalysisConfigID: "config/日本語",
		Records: []json.RawMessage{
			json.RawMessage(fmt.Sprintf(
				`{"id":%q,"kind":"function","name":"préfixe-α","owner":"src/ümlaut.go","path":"src/ümlaut.go","qualified_name":"préfixe-α","record":"node"}`,
				binaryFixtureNodeID(0),
			)),
			json.RawMessage(fmt.Sprintf(
				`{"id":%q,"kind":"function","name":"préfixe-β","owner":"src/ümlaut.go","path":"src/ümlaut.go","qualified_name":"préfixe-β","record":"node"}`,
				binaryFixtureNodeID(1),
			)),
		},
	}
	encoded, err := encodeBinaryObject(object)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := decodeBinaryObject(encoded)
	if err != nil {
		t.Fatal(err)
	}
	assertObjectEquivalent(t, decoded, object)
}

func TestBinaryObjectRejectsMalformedStringPrefix(t *testing.T) {
	encoded, err := encodeBinaryObject(binaryFixture(1))
	if err != nil {
		t.Fatal(err)
	}
	reader := binaryObjectReader{data: encoded, position: len(binaryObjectMagic)}
	if _, err := reader.uvarint("object version"); err != nil {
		t.Fatal(err)
	}
	if _, err := reader.uvarint("schema version"); err != nil {
		t.Fatal(err)
	}
	count, err := reader.count("string table", maxBinaryStrings)
	if err != nil {
		t.Fatal(err)
	}
	if count < 2 {
		t.Fatalf("string table count = %d, want at least 2", count)
	}
	if _, err := reader.uvarint("empty string prefix"); err != nil {
		t.Fatal(err)
	}
	if _, err := reader.bytes("empty string suffix", maxBinaryStringSize); err != nil {
		t.Fatal(err)
	}
	prefixOffset := reader.position
	corrupt := append([]byte(nil), encoded...)
	corrupt[prefixOffset] = 0x7f
	if _, err := decodeBinaryObject(corrupt); err == nil {
		t.Fatal("expected malformed string prefix error")
	}
}
