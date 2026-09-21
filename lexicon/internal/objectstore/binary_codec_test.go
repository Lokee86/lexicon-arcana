package objectstore

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"reflect"
	"testing"
)

func TestBinaryObjectRoundTripIsDeterministic(t *testing.T) {
	object := binaryFixture(12)
	first, err := encodeBinaryObject(object)
	if err != nil {
		t.Fatal(err)
	}
	second, err := encodeBinaryObject(object)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(first, second) {
		t.Fatal("repeated binary encoding changed bytes")
	}
	if !isBinaryObject(first) {
		t.Fatal("encoded object does not have the Lexicon binary magic")
	}
	decoded, err := decodeBinaryObject(first)
	if err != nil {
		t.Fatal(err)
	}
	assertObjectEquivalent(t, decoded, object)
}

func TestBinaryObjectRejectsTruncationAndTrailingBytes(t *testing.T) {
	encoded, err := encodeBinaryObject(binaryFixture(2))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := decodeBinaryObject(encoded[:len(encoded)-1]); err == nil {
		t.Fatal("expected truncated object error")
	}
	if _, err := decodeBinaryObject(append(append([]byte(nil), encoded...), 0)); err == nil {
		t.Fatal("expected trailing byte error")
	}
}

func TestBinaryObjectRejectsInvalidUTF8(t *testing.T) {
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
	if err != nil || count < 2 {
		t.Fatalf("string table: count=%d err=%v", count, err)
	}
	for index := 0; index < count; index++ {
		if _, err := reader.uvarint("string prefix length"); err != nil {
			t.Fatal(err)
		}
		length, err := reader.uvarint("string suffix length")
		if err != nil {
			t.Fatal(err)
		}
		if length == 0 {
			continue
		}
		corrupt := append([]byte(nil), encoded...)
		corrupt[reader.position] = 0xff
		if _, err := decodeBinaryObject(corrupt); err == nil {
			t.Fatal("expected invalid UTF-8 error")
		}
		return
	}
	t.Fatal("first non-empty string suffix is unavailable")
}

func TestStoreReadsLegacyJSONObject(t *testing.T) {
	store := Store{Root: t.TempDir()}
	object := binaryFixture(1)
	data, err := json.Marshal(object)
	if err != nil {
		t.Fatal(err)
	}
	id := digest("lexicon:fact-object:v1\x00", data)
	if err := writeImmutable(store.ObjectPath(id), append(data, '\n')); err != nil {
		t.Fatal(err)
	}
	loaded, err := store.LoadObject(id)
	if err != nil {
		t.Fatal(err)
	}
	assertObjectEquivalent(t, loaded, object)
}

func TestStoreWritesBinaryObjects(t *testing.T) {
	store := Store{Root: t.TempDir()}
	object := binaryFixture(3)
	id, err := store.WriteObject(object)
	if err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(store.ObjectPath(id))
	if err != nil {
		t.Fatal(err)
	}
	if !isBinaryObject(data) {
		t.Fatal("object store wrote JSON instead of binary")
	}
	loaded, err := store.LoadObject(id)
	if err != nil {
		t.Fatal(err)
	}
	object.Version = ObjectVersion
	assertObjectEquivalent(t, loaded, object)
}

func TestBinaryObjectIsSmallerForRepeatedFacts(t *testing.T) {
	object := binaryFixture(200)
	v2Data, err := encodeBinaryObject(object)
	if err != nil {
		t.Fatal(err)
	}
	v1Data, err := encodeBinaryObjectV1(object)
	if err != nil {
		t.Fatal(err)
	}
	jsonData, err := json.Marshal(object)
	if err != nil {
		t.Fatal(err)
	}
	v1Reduction := 100 * (1 - float64(len(v2Data))/float64(len(v1Data)))
	jsonReduction := 100 * (1 - float64(len(v2Data))/float64(len(jsonData)))
	t.Logf("v2 bytes = %d, v1 bytes = %d, JSON bytes = %d, v1 reduction = %.1f%%, JSON reduction = %.1f%%",
		len(v2Data), len(v1Data), len(jsonData), v1Reduction, jsonReduction)
	if len(v2Data) >= len(v1Data) {
		t.Fatalf("v2 bytes = %d, v1 bytes = %d", len(v2Data), len(v1Data))
	}
	if len(v2Data) >= len(jsonData) {
		t.Fatalf("v2 bytes = %d, JSON bytes = %d", len(v2Data), len(jsonData))
	}
}

func FuzzDecodeBinaryObject(f *testing.F) {
	seed, err := encodeBinaryObject(binaryFixture(2))
	if err != nil {
		f.Fatal(err)
	}
	f.Add(seed)
	f.Fuzz(func(t *testing.T, data []byte) {
		_, _ = decodeBinaryObject(data)
	})
}

func BenchmarkFactObjectCodecs(b *testing.B) {
	object := binaryFixture(500)
	jsonData, err := json.Marshal(object)
	if err != nil {
		b.Fatal(err)
	}
	typed, err := parseTypedRecords(object.Records)
	if err != nil {
		b.Fatal(err)
	}
	typedObject := object
	typedObject.typed = &typed
	binaryData, err := encodeBinaryObject(typedObject)
	if err != nil {
		b.Fatal(err)
	}
	b.ReportMetric(float64(len(jsonData)), "json-bytes")
	b.ReportMetric(float64(len(binaryData)), "binary-bytes")
	b.Run("json-encode", func(b *testing.B) {
		for b.Loop() {
			if _, err := json.Marshal(object); err != nil {
				b.Fatal(err)
			}
		}
	})
	b.Run("binary-encode-raw", func(b *testing.B) {
		for b.Loop() {
			if _, err := encodeBinaryObject(object); err != nil {
				b.Fatal(err)
			}
		}
	})
	b.Run("binary-encode-typed", func(b *testing.B) {
		for b.Loop() {
			if _, err := encodeBinaryObject(typedObject); err != nil {
				b.Fatal(err)
			}
		}
	})
	b.Run("json-decode", func(b *testing.B) {
		for b.Loop() {
			var decoded FactObject
			if err := json.Unmarshal(jsonData, &decoded); err != nil {
				b.Fatal(err)
			}
		}
	})
	b.Run("binary-decode", func(b *testing.B) {
		for b.Loop() {
			if _, err := decodeBinaryObject(binaryData); err != nil {
				b.Fatal(err)
			}
		}
	})
	b.Run("binary-node-only", func(b *testing.B) {
		for b.Loop() {
			if _, _, err := decodeBinaryNodeFacts(binaryData); err != nil {
				b.Fatal(err)
			}
		}
	})
}

func binaryFixture(count int) FactObject {
	records := make([]json.RawMessage, 0, count*2+1)
	for index := range count {
		id := binaryFixtureNodeID(index)
		records = append(records, json.RawMessage(fmt.Sprintf(
			`{"attributes":{"visibility":"public"},"id":%q,"kind":"function","name":%q,"owner":"src/main.go","path":"src/main.go","qualified_name":%q,"record":"node","span":{"end_column":2,"end_line":%d,"path":"src/main.go","start_column":1,"start_line":%d}}`,
			id, fmt.Sprintf("function%d", index), fmt.Sprintf("demo.function%d", index), index+1, index+1,
		)))
	}
	for index := 1; index < count; index++ {
		records = append(records, json.RawMessage(fmt.Sprintf(
			`{"owner":"src/main.go","record":"edge","relation":"calls","source":%q,"target":%q}`,
			binaryFixtureNodeID(index-1), binaryFixtureNodeID(index),
		)))
	}
	if count > 0 {
		records = append(records, json.RawMessage(fmt.Sprintf(
			`{"candidate_name":"dynamic","expression":"dynamic()","owner":"src/main.go","reason":"dynamic-target","record":"unresolved","relation":"calls","source":%q}`,
			binaryFixtureNodeID(0),
		)))
	}
	return FactObject{
		Version: ObjectVersion, Language: "go", Owner: "src/main.go",
		SourceContentID:  "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		AdapterVersion:   "1.2.3",
		SchemaVersion:    1,
		AnalysisConfigID: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
		Records:          records,
	}
}

func binaryFixtureNodeID(index int) string {
	return fmt.Sprintf("sha256:%064x", index+1)
}

func assertObjectEquivalent(t *testing.T, got, want FactObject) {
	t.Helper()
	if got.Version != want.Version || got.Language != want.Language || got.Owner != want.Owner ||
		got.SourceContentID != want.SourceContentID || got.AdapterVersion != want.AdapterVersion ||
		got.SchemaVersion != want.SchemaVersion || got.AnalysisConfigID != want.AnalysisConfigID {
		t.Fatalf("metadata mismatch: got %#v want %#v", got, want)
	}
	if len(got.Records) != len(want.Records) {
		t.Fatalf("records = %d, want %d", len(got.Records), len(want.Records))
	}
	for index := range got.Records {
		var gotValue, wantValue any
		if err := json.Unmarshal(got.Records[index], &gotValue); err != nil {
			t.Fatal(err)
		}
		if err := json.Unmarshal(want.Records[index], &wantValue); err != nil {
			t.Fatal(err)
		}
		if !reflect.DeepEqual(gotValue, wantValue) {
			t.Fatalf("record %d mismatch:\n got %s\nwant %s", index, got.Records[index], want.Records[index])
		}
	}
}
