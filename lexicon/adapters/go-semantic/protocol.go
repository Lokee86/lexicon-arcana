package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"path"
	"path/filepath"
	"sort"
	"strings"
)

const protocolVersion uint32 = 2

type request struct {
	ProtocolVersion uint32    `json:"protocol_version"`
	RepositoryRoot  string    `json:"repository_root"`
	Files           []string  `json:"files"`
	Modules         []module  `json:"modules,omitempty"`
	Execution       execution `json:"execution"`
}

type module struct {
	Root string `json:"root"`
	Path string `json:"path"`
}

type execution struct {
	Workers    int `json:"workers"`
	Shards     int `json:"shards"`
	MergeFanIn int `json:"merge_fan_in"`
}

type wireObservation interface {
	wireObservation()
}

type response struct {
	ProtocolVersion uint32            `json:"protocol_version"`
	Observations    []wireObservation `json:"observations"`
	records         []semanticRecord
}

type declarationObservation struct {
	Observation string            `json:"observation"`
	SemanticKey string            `json:"semantic_key"`
	Kind        string            `json:"kind"`
	Name        string            `json:"name"`
	Owner       string            `json:"owner"`
	Span        wireSpan          `json:"span"`
	Metadata    map[string]string `json:"metadata,omitempty"`
}

func (declarationObservation) wireObservation() {}

type relationshipObservation struct {
	Observation string    `json:"observation"`
	SourceKey   string    `json:"source_key"`
	TargetKey   string    `json:"target_key"`
	Kind        string    `json:"kind"`
	Owner       string    `json:"owner"`
	Span        *wireSpan `json:"span,omitempty"`
}

func (relationshipObservation) wireObservation() {}

type symbolObservation struct {
	Observation  string    `json:"observation"`
	SemanticKey  string    `json:"semantic_key"`
	Name         string    `json:"name,omitempty"`
	Namespace    string    `json:"namespace,omitempty"`
	ContainerKey string    `json:"container_key,omitempty"`
	Owner        string    `json:"owner,omitempty"`
	Span         *wireSpan `json:"span,omitempty"`
	Generated    bool      `json:"generated,omitempty"`
}

func (symbolObservation) wireObservation() {}

type callTargetObservation struct {
	SemanticKey  string    `json:"semantic_key"`
	Name         string    `json:"name,omitempty"`
	Namespace    string    `json:"namespace,omitempty"`
	ContainerKey string    `json:"container_key,omitempty"`
	Owner        string    `json:"owner,omitempty"`
	Span         *wireSpan `json:"span,omitempty"`
	Generated    bool      `json:"generated,omitempty"`
}

type callsiteObservation struct {
	Observation        string                  `json:"observation"`
	SourceKey          string                  `json:"source_key"`
	Form               string                  `json:"form"`
	Resolution         string                  `json:"resolution"`
	Expression         string                  `json:"expression,omitempty"`
	CandidateNamespace string                  `json:"candidate_namespace,omitempty"`
	CandidateName      string                  `json:"candidate_name,omitempty"`
	Targets            []callTargetObservation `json:"targets,omitempty"`
	Owner              string                  `json:"owner"`
	Span               wireSpan                `json:"span"`
}

func (callsiteObservation) wireObservation() {}

type dataflowWireObservation struct {
	Observation string   `json:"observation"`
	SourceKey   string   `json:"source_key"`
	TargetKey   string   `json:"target_key"`
	Access      string   `json:"access"`
	Owner       string   `json:"owner"`
	Span        wireSpan `json:"span"`
}

func (dataflowWireObservation) wireObservation() {}

type captureObservation struct {
	Observation  string    `json:"observation"`
	SourceKey    string    `json:"source_key"`
	TargetKey    string    `json:"target_key,omitempty"`
	TargetName   string    `json:"target_name"`
	CaptureIndex int       `json:"capture_index"`
	Owner        string    `json:"owner"`
	Span         *wireSpan `json:"span,omitempty"`
}

func (captureObservation) wireObservation() {}

type diagnosticObservation struct {
	Observation string    `json:"observation"`
	Severity    string    `json:"severity"`
	Code        string    `json:"code"`
	Message     string    `json:"message"`
	Owner       string    `json:"owner,omitempty"`
	Span        *wireSpan `json:"span,omitempty"`
}

func (diagnosticObservation) wireObservation() {}

type wireSpan struct {
	StartLine   uint32 `json:"start_line"`
	StartColumn uint32 `json:"start_column"`
	EndLine     uint32 `json:"end_line"`
	EndColumn   uint32 `json:"end_column"`
}

func responseFromRecords(records []semanticRecord) response {
	declarations := make([]wireObservation, 0)
	relationships := make([]wireObservation, 0)
	dataflow := make([]wireObservation, 0)
	symbols := make([]wireObservation, 0)
	captures := make([]wireObservation, 0)
	diagnostics := make([]wireObservation, 0)
	calls := make(map[string]*callsiteObservation)

	for _, record := range records {
		switch value := record.(type) {
		case declaration:
			declarations = append(declarations, declarationObservation{
				Observation: "declaration", SemanticKey: value.Identity, Kind: value.Kind,
				Name: value.Name, Owner: value.Owner, Span: toWireSpan(value.Span), Metadata: value.Metadata,
			})
		case relationship:
			if value.Kind == "references" {
				if value.CaptureIndex == nil {
					continue
				}
				captures = append(captures, captureObservation{
					Observation: "capture", SourceKey: value.Source, TargetKey: value.Target,
					TargetName: value.TargetName, CaptureIndex: *value.CaptureIndex,
					Owner: value.Owner, Span: toOptionalWireSpan(value.Span),
				})
				continue
			}
			relationships = append(relationships, relationshipObservation{
				Observation: "relationship", SourceKey: value.Source, TargetKey: value.Target,
				Kind: value.Kind, Owner: value.Owner, Span: toOptionalWireSpan(value.Span),
			})
		case dataflowObservation:
			dataflow = append(dataflow, dataflowWireObservation{
				Observation: "dataflow", SourceKey: value.Source, TargetKey: value.Target,
				Access: value.Kind, Owner: value.Owner, Span: toWireSpan(value.Span),
			})
		case targetObservation:
			symbols = append(symbols, symbolObservation{
				Observation: "symbol", SemanticKey: value.Identity, Name: value.Name,
				Namespace: value.Namespace, ContainerKey: value.Container,
				Generated: value.Class == "internal" || strings.HasPrefix(value.Identity, "ssa-function:"),
			})
		case callObservation:
			key := recordCallsiteKey(value)
			call := calls[key]
			if call == nil {
				call = &callsiteObservation{
					Observation: "callsite", SourceKey: value.Source, Form: callForm(value.Class),
					Resolution: "resolved", Owner: value.Owner, Span: toWireSpan(value.Span),
				}
				calls[key] = call
			}
			call.Form = mergeCallForm(call.Form, callForm(value.Class))
			call.Resolution = "resolved"
			target := callTargetObservation{
				SemanticKey: value.Target, Name: value.TargetName, Namespace: value.TargetNamespace,
				ContainerKey: value.TargetContainer, Owner: value.TargetOwner,
				Span: toOptionalWireSpan(value.TargetSpan),
			}
			if !hasWireCallTarget(call.Targets, target.SemanticKey) {
				call.Targets = append(call.Targets, target)
			}
		case unresolvedObservation:
			key := recordCallsiteKey(value)
			call := calls[key]
			if call == nil {
				call = &callsiteObservation{
					Observation: "callsite", SourceKey: value.Source, Form: callForm(value.Class),
					Resolution: resolutionEvidence(value.Reason), Owner: value.Owner, Span: toWireSpan(value.Span),
				}
				calls[key] = call
			}
			if len(call.Targets) == 0 {
				call.Form = mergeCallForm(call.Form, callForm(value.Class))
				call.Resolution = resolutionEvidence(value.Reason)
				call.Expression = value.Expression
				call.CandidateNamespace = value.CandidateNamespace
				call.CandidateName = value.CandidateName
			}
		case diagnostic:
			diagnostics = append(diagnostics, diagnosticObservation{
				Observation: "diagnostic", Severity: value.Severity, Code: value.Code,
				Message: value.Message, Owner: value.Owner, Span: toOptionalWireSpan(value.Span),
			})
		}
	}

	callKeys := make([]string, 0, len(calls))
	for key := range calls {
		callKeys = append(callKeys, key)
	}
	sort.Strings(callKeys)
	callObservations := make([]wireObservation, 0, len(callKeys))
	for _, key := range callKeys {
		call := calls[key]
		sort.SliceStable(call.Targets, func(i, j int) bool {
			return call.Targets[i].SemanticKey < call.Targets[j].SemanticKey
		})
		callObservations = append(callObservations, *call)
	}

	observations := make([]wireObservation, 0,
		len(declarations)+len(relationships)+len(dataflow)+len(symbols)+len(callObservations)+len(captures)+len(diagnostics))
	observations = append(observations, declarations...)
	observations = append(observations, relationships...)
	observations = append(observations, dataflow...)
	observations = append(observations, symbols...)
	observations = append(observations, callObservations...)
	observations = append(observations, captures...)
	observations = append(observations, diagnostics...)

	return response{ProtocolVersion: protocolVersion, Observations: observations, records: records}
}

func callForm(class string) string {
	switch class {
	case "interface":
		return "interface"
	case "dynamic":
		return "dynamic"
	case "builtin":
		return "builtin"
	case "conversion":
		return "conversion"
	default:
		return "direct"
	}
}

func mergeCallForm(left, right string) string {
	priority := map[string]int{"direct": 0, "builtin": 1, "conversion": 2, "dynamic": 3, "interface": 4}
	if priority[right] > priority[left] {
		return right
	}
	return left
}

func resolutionEvidence(reason string) string {
	switch reason {
	case "ambiguous-target":
		return "ambiguous"
	case "unsupported-form":
		return "unsupported"
	default:
		return "missing"
	}
}

func hasWireCallTarget(targets []callTargetObservation, semanticKey string) bool {
	for _, target := range targets {
		if target.SemanticKey == semanticKey {
			return true
		}
	}
	return false
}

func toWireSpan(value span) wireSpan {
	return wireSpan{
		StartLine: value.StartLine, StartColumn: value.StartColumn,
		EndLine: value.EndLine, EndColumn: value.EndColumn,
	}
}

func toOptionalWireSpan(value *span) *wireSpan {
	if value == nil {
		return nil
	}
	converted := toWireSpan(*value)
	return &converted
}

func decodeRequest(data []byte) (request, error) {
	var value request
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&value); err != nil {
		return request{}, err
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		if err == nil {
			return request{}, fmt.Errorf("multiple JSON values")
		}
		return request{}, err
	}
	if err := validateRequest(value); err != nil {
		return request{}, err
	}
	return value, nil
}

func validateRequest(value request) error {
	if value.ProtocolVersion != protocolVersion {
		return fmt.Errorf("unsupported Go semantic protocol version %d", value.ProtocolVersion)
	}
	if !filepath.IsAbs(value.RepositoryRoot) || filepath.Clean(value.RepositoryRoot) != value.RepositoryRoot {
		return fmt.Errorf("repository_root must be an absolute cleaned path")
	}
	for index, file := range value.Files {
		if err := validateOwner(file); err != nil {
			return fmt.Errorf("files[%d]: %w", index, err)
		}
		if path.Base(file) != "go.mod" && path.Ext(file) != ".go" {
			return fmt.Errorf("files[%d]: unsupported semantic input %q", index, file)
		}
	}
	for index, module := range value.Modules {
		if module.Root != "." {
			if err := validateOwner(module.Root); err != nil {
				return fmt.Errorf("modules[%d].root: %w", index, err)
			}
		}
		if strings.TrimSpace(module.Path) == "" {
			return fmt.Errorf("modules[%d].path is empty", index)
		}
	}
	if value.Execution.Workers < 1 || value.Execution.Shards < 1 || value.Execution.MergeFanIn < 2 {
		return fmt.Errorf("invalid semantic execution parameters")
	}
	return nil
}

func validateOwner(owner string) error {
	if owner == "" || strings.Contains(owner, "\\") || strings.HasPrefix(owner, "/") {
		return fmt.Errorf("owner path %q is not repository-relative canonical form", owner)
	}
	cleaned := path.Clean(owner)
	if cleaned != owner || cleaned == "." || cleaned == ".." || strings.HasPrefix(cleaned, "../") {
		return fmt.Errorf("owner path %q is not repository-relative canonical form", owner)
	}
	return nil
}
