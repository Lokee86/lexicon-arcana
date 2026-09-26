package main

import (
	"fmt"
	"path"
	"path/filepath"
	"strings"
)

func validateGoSemanticRequest(request goSemanticRequest) error {
	if request.ProtocolVersion != goSemanticProtocolVersion {
		return fmt.Errorf("unsupported Go semantic protocol version %d", request.ProtocolVersion)
	}
	if !filepath.IsAbs(request.RepositoryRoot) || filepath.Clean(request.RepositoryRoot) != request.RepositoryRoot {
		return fmt.Errorf("repository_root must be an absolute cleaned path")
	}
	for index, file := range request.Files {
		if err := validateGoSemanticOwner(file); err != nil {
			return fmt.Errorf("files[%d]: %w", index, err)
		}
		if path.Base(file) != "go.mod" && path.Ext(file) != ".go" {
			return fmt.Errorf("files[%d]: unsupported semantic input %q", index, file)
		}
	}
	for index, module := range request.Modules {
		if module.Root != "." {
			if err := validateGoSemanticOwner(module.Root); err != nil {
				return fmt.Errorf("modules[%d].root: %w", index, err)
			}
		}
		if strings.TrimSpace(module.Path) == "" || containsControl(module.Path) {
			return fmt.Errorf("modules[%d].path is invalid", index)
		}
	}
	if request.Execution.Workers < 1 || request.Execution.Shards < 1 {
		return fmt.Errorf("semantic workers and shards must be positive")
	}
	if request.Execution.MergeFanIn < 2 {
		return fmt.Errorf("semantic merge fan-in must be at least 2")
	}
	return nil
}

func validateGoSemanticResponse(response goSemanticResponse) error {
	if response.ProtocolVersion != goSemanticProtocolVersion {
		return fmt.Errorf("unsupported Go semantic protocol version %d", response.ProtocolVersion)
	}
	for index, record := range response.Records {
		if err := validateGoSemanticRecord(record); err != nil {
			return fmt.Errorf("record %d: %w", index, err)
		}
	}
	return nil
}

func validateGoSemanticRecord(record goSemanticRecord) error {
	if goSemanticRecordPayloadCount(record) != 1 {
		return fmt.Errorf("record %q must contain exactly one payload", record.Kind)
	}
	switch record.Kind {
	case goSemanticRecordDeclaration:
		if record.Declaration == nil {
			return fmt.Errorf("declaration payload is missing")
		}
		return validateGoSemanticDeclaration(*record.Declaration)
	case goSemanticRecordRelationship:
		if record.Relationship == nil {
			return fmt.Errorf("relationship payload is missing")
		}
		value := *record.Relationship
		if !knownGoSemanticRelationship(value.Kind) {
			return fmt.Errorf("unknown relationship kind %q", value.Kind)
		}
		if err := validateGoSemanticIdentity(value.Source); err != nil {
			return fmt.Errorf("source: %w", err)
		}
		if err := validateGoSemanticOwner(value.Owner); err != nil {
			return err
		}
		if value.Kind == goSemanticRelationshipReferences {
			if value.CaptureIndex == nil || value.TargetName == "" {
				return fmt.Errorf("capture references require capture_index and target_name")
			}
			if value.Target != "" {
				if err := validateGoSemanticIdentity(value.Target); err != nil {
					return fmt.Errorf("target: %w", err)
				}
			}
			if value.Span != nil {
				return validateGoSemanticSpan(*value.Span)
			}
			return nil
		}
		if value.Target == "" || value.Span == nil {
			return fmt.Errorf("relationship target and span are required")
		}
		return validateGoSemanticReference(value.Source, value.Target, value.Owner, *value.Span)
	case goSemanticRecordCall:
		if record.Call == nil {
			return fmt.Errorf("call payload is missing")
		}
		value := *record.Call
		if !knownGoSemanticCall(value.Kind) {
			return fmt.Errorf("unknown call kind %q", value.Kind)
		}
		if !knownGoSemanticCallClass(value.Class) {
			return fmt.Errorf("unknown call class %q", value.Class)
		}
		if value.TargetContainer != "" {
			if err := validateGoSemanticIdentity(value.TargetContainer); err != nil {
				return fmt.Errorf("target_container: %w", err)
			}
		}
		if (value.TargetName == "") != (value.TargetNamespace == "") {
			return fmt.Errorf("target_name and target_namespace must be supplied together")
		}
		return validateGoSemanticReference(value.Source, value.Target, value.Owner, value.Span)
	case goSemanticRecordDataflow:
		if record.Dataflow == nil {
			return fmt.Errorf("dataflow payload is missing")
		}
		value := *record.Dataflow
		if !knownGoSemanticDataflow(value.Kind) {
			return fmt.Errorf("unknown dataflow kind %q", value.Kind)
		}
		return validateGoSemanticReference(value.Source, value.Target, value.Owner, value.Span)
	case goSemanticRecordUnresolved:
		if record.Unresolved == nil {
			return fmt.Errorf("unresolved payload is missing")
		}
		return validateGoSemanticUnresolved(*record.Unresolved)
	case goSemanticRecordDiagnostic:
		if record.Diagnostic == nil {
			return fmt.Errorf("diagnostic payload is missing")
		}
		return validateGoSemanticDiagnostic(*record.Diagnostic)
	default:
		return fmt.Errorf("unknown record kind %q", record.Kind)
	}
}

func validateGoSemanticDeclaration(value goSemanticDeclaration) error {
	if !knownGoSemanticDeclaration(value.Kind) {
		return fmt.Errorf("unknown declaration kind %q", value.Kind)
	}
	if strings.TrimSpace(value.Name) == "" {
		return fmt.Errorf("declaration name is empty")
	}
	if err := validateGoSemanticIdentity(value.Identity); err != nil {
		return err
	}
	if !identityMatchesDeclarationKind(value.Identity, value.Kind) {
		return fmt.Errorf("identity %q does not match declaration kind %q", value.Identity, value.Kind)
	}
	return validateGoSemanticLocation(value.Owner, value.Span)
}

func validateGoSemanticReference(source, target, owner string, span goSemanticSpan) error {
	if err := validateGoSemanticIdentity(source); err != nil {
		return fmt.Errorf("source: %w", err)
	}
	if err := validateGoSemanticIdentity(target); err != nil {
		return fmt.Errorf("target: %w", err)
	}
	return validateGoSemanticLocation(owner, span)
}

func validateGoSemanticUnresolved(value goSemanticUnresolvedObservation) error {
	if err := validateGoSemanticIdentity(value.Source); err != nil {
		return fmt.Errorf("source: %w", err)
	}
	if strings.TrimSpace(value.Relation) == "" || strings.TrimSpace(value.Expression) == "" {
		return fmt.Errorf("unresolved relation and expression are required")
	}
	if !knownGoSemanticUnresolved(value.Reason) {
		return fmt.Errorf("unknown unresolved reason %q", value.Reason)
	}
	if !knownGoSemanticCallClass(value.Class) {
		return fmt.Errorf("unknown unresolved call class %q", value.Class)
	}
	return validateGoSemanticLocation(value.Owner, value.Span)
}

func validateGoSemanticDiagnostic(value goSemanticDiagnostic) error {
	if !knownGoSemanticSeverity(value.Severity) || strings.TrimSpace(value.Code) == "" || strings.TrimSpace(value.Message) == "" {
		return fmt.Errorf("diagnostic severity, code, and message are required")
	}
	if value.Owner != "" {
		if err := validateGoSemanticOwner(value.Owner); err != nil {
			return err
		}
	}
	if value.Span != nil {
		if value.Owner == "" {
			return fmt.Errorf("diagnostic span requires an owner")
		}
		return validateGoSemanticSpan(*value.Span)
	}
	return nil
}

func validateGoSemanticLocation(owner string, span goSemanticSpan) error {
	if err := validateGoSemanticOwner(owner); err != nil {
		return err
	}
	return validateGoSemanticSpan(span)
}
