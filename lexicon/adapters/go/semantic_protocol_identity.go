package main

import (
	"fmt"
	"path"
	"strings"
	"unicode"
)

func validateGoSemanticOwner(owner string) error {
	if owner == "" || strings.Contains(owner, "\\") || strings.HasPrefix(owner, "/") {
		return fmt.Errorf("owner path %q is not repository-relative canonical form", owner)
	}
	if cleaned := path.Clean(owner); cleaned != owner || cleaned == "." || cleaned == ".." || strings.HasPrefix(cleaned, "../") {
		return fmt.Errorf("owner path %q is not repository-relative canonical form", owner)
	}
	return nil
}

func validateGoSemanticSpan(span goSemanticSpan) error {
	if span.StartLine == 0 || span.StartColumn == 0 || span.EndLine == 0 || span.EndColumn == 0 {
		return fmt.Errorf("source span is incomplete")
	}
	if span.EndLine < span.StartLine || (span.EndLine == span.StartLine && span.EndColumn < span.StartColumn) {
		return fmt.Errorf("source span ends before it starts")
	}
	return nil
}

func validateGoSemanticIdentity(identity string) error {
	prefix, rest, ok := strings.Cut(identity, ":")
	if !ok || rest == "" || strings.TrimSpace(identity) != identity ||
		containsControl(identity) || strings.HasPrefix(identity, "sha256:") {
		return fmt.Errorf("invalid semantic identity %q", identity)
	}
	switch prefix {
	case "package", "import", "namespace", "type", "type-expression", "function", "closure", "ssa-function",
		"method", "interface-method", "dynamic-method", "test", "parameter", "variable", "capture", "field", "constant":
		return nil
	default:
		return fmt.Errorf("unknown semantic identity prefix %q", prefix)
	}
}

func containsControl(value string) bool {
	return strings.IndexFunc(value, unicode.IsControl) >= 0
}
