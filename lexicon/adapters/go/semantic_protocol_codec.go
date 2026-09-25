package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
)

type goSemanticResponseWire struct {
	ProtocolVersion uint32            `json:"protocol_version"`
	Records         []json.RawMessage `json:"records"`
}

type goSemanticRecordHeader struct {
	Record goSemanticRecordKind `json:"record"`
}

func encodeGoSemanticRequest(request goSemanticRequest) ([]byte, error) {
	if err := validateGoSemanticRequest(request); err != nil {
		return nil, err
	}
	return json.Marshal(request)
}

func decodeGoSemanticRequest(data []byte) (goSemanticRequest, error) {
	var request goSemanticRequest
	if err := decodeStrictJSON(data, &request); err != nil {
		return goSemanticRequest{}, err
	}
	if err := validateGoSemanticRequest(request); err != nil {
		return goSemanticRequest{}, err
	}
	return request, nil
}

func encodeGoSemanticResponse(response goSemanticResponse) ([]byte, error) {
	if err := validateGoSemanticResponse(response); err != nil {
		return nil, err
	}
	wire := goSemanticResponseWire{ProtocolVersion: response.ProtocolVersion}
	for index, record := range response.Records {
		encoded, err := encodeGoSemanticRecord(record)
		if err != nil {
			return nil, fmt.Errorf("record %d: %w", index, err)
		}
		wire.Records = append(wire.Records, encoded)
	}
	return json.Marshal(wire)
}

func decodeGoSemanticResponse(data []byte) (goSemanticResponse, error) {
	var wire goSemanticResponseWire
	if err := decodeStrictJSON(data, &wire); err != nil {
		return goSemanticResponse{}, err
	}
	response := goSemanticResponse{ProtocolVersion: wire.ProtocolVersion}
	for index, raw := range wire.Records {
		record, err := decodeGoSemanticRecord(raw)
		if err != nil {
			return goSemanticResponse{}, fmt.Errorf("record %d: %w", index, err)
		}
		response.Records = append(response.Records, record)
	}
	if err := validateGoSemanticResponse(response); err != nil {
		return goSemanticResponse{}, err
	}
	return response, nil
}

func encodeGoSemanticRecord(record goSemanticRecord) ([]byte, error) {
	switch record.Kind {
	case goSemanticRecordDeclaration:
		return json.Marshal(struct {
			Record goSemanticRecordKind `json:"record"`
			*goSemanticDeclaration
		}{record.Kind, record.Declaration})
	case goSemanticRecordRelationship:
		return json.Marshal(struct {
			Record goSemanticRecordKind `json:"record"`
			*goSemanticRelationship
		}{record.Kind, record.Relationship})
	case goSemanticRecordCall:
		return json.Marshal(struct {
			Record goSemanticRecordKind `json:"record"`
			*goSemanticCallObservation
		}{record.Kind, record.Call})
	case goSemanticRecordDataflow:
		return json.Marshal(struct {
			Record goSemanticRecordKind `json:"record"`
			*goSemanticDataflowObservation
		}{record.Kind, record.Dataflow})
	case goSemanticRecordUnresolved:
		return json.Marshal(struct {
			Record goSemanticRecordKind `json:"record"`
			*goSemanticUnresolvedObservation
		}{record.Kind, record.Unresolved})
	case goSemanticRecordDiagnostic:
		return json.Marshal(struct {
			Record goSemanticRecordKind `json:"record"`
			*goSemanticDiagnostic
		}{record.Kind, record.Diagnostic})
	default:
		return nil, fmt.Errorf("unknown record kind %q", record.Kind)
	}
}

func decodeGoSemanticRecord(data []byte) (goSemanticRecord, error) {
	var header goSemanticRecordHeader
	if err := json.Unmarshal(data, &header); err != nil {
		return goSemanticRecord{}, err
	}
	switch header.Record {
	case goSemanticRecordDeclaration:
		var wire struct {
			Record goSemanticRecordKind `json:"record"`
			goSemanticDeclaration
		}
		if err := decodeStrictJSON(data, &wire); err != nil {
			return goSemanticRecord{}, err
		}
		return goSemanticRecord{Kind: header.Record, Declaration: &wire.goSemanticDeclaration}, nil
	case goSemanticRecordRelationship:
		var wire struct {
			Record goSemanticRecordKind `json:"record"`
			goSemanticRelationship
		}
		if err := decodeStrictJSON(data, &wire); err != nil {
			return goSemanticRecord{}, err
		}
		return goSemanticRecord{Kind: header.Record, Relationship: &wire.goSemanticRelationship}, nil
	case goSemanticRecordCall:
		var wire struct {
			Record goSemanticRecordKind `json:"record"`
			goSemanticCallObservation
		}
		if err := decodeStrictJSON(data, &wire); err != nil {
			return goSemanticRecord{}, err
		}
		return goSemanticRecord{Kind: header.Record, Call: &wire.goSemanticCallObservation}, nil
	case goSemanticRecordDataflow:
		var wire struct {
			Record goSemanticRecordKind `json:"record"`
			goSemanticDataflowObservation
		}
		if err := decodeStrictJSON(data, &wire); err != nil {
			return goSemanticRecord{}, err
		}
		return goSemanticRecord{Kind: header.Record, Dataflow: &wire.goSemanticDataflowObservation}, nil
	case goSemanticRecordUnresolved:
		var wire struct {
			Record goSemanticRecordKind `json:"record"`
			goSemanticUnresolvedObservation
		}
		if err := decodeStrictJSON(data, &wire); err != nil {
			return goSemanticRecord{}, err
		}
		return goSemanticRecord{Kind: header.Record, Unresolved: &wire.goSemanticUnresolvedObservation}, nil
	case goSemanticRecordDiagnostic:
		var wire struct {
			Record goSemanticRecordKind `json:"record"`
			goSemanticDiagnostic
		}
		if err := decodeStrictJSON(data, &wire); err != nil {
			return goSemanticRecord{}, err
		}
		return goSemanticRecord{Kind: header.Record, Diagnostic: &wire.goSemanticDiagnostic}, nil
	default:
		return goSemanticRecord{}, fmt.Errorf("unknown record kind %q", header.Record)
	}
}

func decodeStrictJSON(data []byte, target any) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return err
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		if err == nil {
			return fmt.Errorf("multiple JSON values")
		}
		return err
	}
	return nil
}
