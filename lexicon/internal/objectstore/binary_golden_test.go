package objectstore

import (
	"encoding/hex"
	"encoding/json"
	"testing"
)

const binaryObjectGoldenHex = "4c584f424a0001000101110002676f076d61696e2e676f477368613235363a6262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626205312e302e30477368613235363a63636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363477368613235363a313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310466696c65477368613235363a323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232320866756e6374696f6e046d61696e0964656d6f2e6d61696e08636f6e7461696e730166036628290e64796e616d69632d7461726765740563616c6c7301020304051802000306070202020200000008090a02020b010202010202070100020c0800060a01000d000e020f100800"
const binaryObjectV2GoldenHex = "4c584f424a00020001010900000005312e302e30000964656d6f2e6d61696e010d796e616d69632d746172676574000166010228290002676f00046d61696e04032e676f060801bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb0101cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc007a020001bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb0111111111111111111111111111111111111111111111111111111111111111110308010101000000000122222222222222222222222222222222222222222222222222222222222222220b0701010002010802010202090100010100020000010b0100040005010304000200"

func TestBinaryObjectGolden(t *testing.T) {
	data, err := encodeBinaryObject(binaryGoldenObject())
	if err != nil {
		t.Fatal(err)
	}
	if got := hex.EncodeToString(data); got != binaryObjectV2GoldenHex {
		t.Fatalf("binary v2 object changed:\n got %s\nwant %s", got, binaryObjectV2GoldenHex)
	}
	decoded, err := decodeBinaryObject(data)
	if err != nil {
		t.Fatal(err)
	}
	assertObjectEquivalent(t, decoded, binaryGoldenObject())
}

func TestBinaryObjectLegacyV1GoldenDecodes(t *testing.T) {
	data, err := hex.DecodeString(binaryObjectGoldenHex)
	if err != nil {
		t.Fatal(err)
	}
	got, err := decodeBinaryObject(data)
	if err != nil {
		t.Fatal(err)
	}
	assertObjectEquivalent(t, got, binaryGoldenObject())
}

func binaryGoldenObject() FactObject {
	return FactObject{
		Version: ObjectVersion, Language: "go", Owner: "main.go",
		SourceContentID: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
		AdapterVersion:  "1.0.0", SchemaVersion: 1,
		AnalysisConfigID: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
		Records: []json.RawMessage{
			json.RawMessage(`{"content_id":"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","id":"sha256:1111111111111111111111111111111111111111111111111111111111111111","kind":"file","name":"main.go","owner":"main.go","path":"main.go","qualified_name":"main.go","record":"node"}`),
			json.RawMessage(`{"id":"sha256:2222222222222222222222222222222222222222222222222222222222222222","kind":"function","name":"main","owner":"main.go","path":"main.go","qualified_name":"demo.main","record":"node","span":{"end_column":2,"end_line":2,"path":"main.go","start_column":1,"start_line":2}}`),
			json.RawMessage(`{"owner":"main.go","record":"edge","relation":"contains","source":"sha256:2222222222222222222222222222222222222222222222222222222222222222","target":"sha256:1111111111111111111111111111111111111111111111111111111111111111"}`),
			json.RawMessage(`{"candidate_name":"f","expression":"f()","owner":"main.go","reason":"dynamic-target","record":"unresolved","relation":"calls","source":"sha256:2222222222222222222222222222222222222222222222222222222222222222"}`),
		},
	}
}
