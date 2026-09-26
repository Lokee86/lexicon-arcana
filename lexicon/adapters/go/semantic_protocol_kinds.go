package main

import "strings"

func goSemanticRecordPayloadCount(record goSemanticRecord) int {
	count := 0
	for _, present := range []bool{
		record.Declaration != nil, record.Relationship != nil, record.Call != nil,
		record.Dataflow != nil, record.Unresolved != nil, record.Diagnostic != nil,
	} {
		if present {
			count++
		}
	}
	return count
}

func knownGoSemanticDeclaration(kind goSemanticDeclarationKind) bool {
	switch kind {
	case goSemanticDeclarationPackage, goSemanticDeclarationImport, goSemanticDeclarationNamespace, goSemanticDeclarationType,
		goSemanticDeclarationFunction, goSemanticDeclarationMethod, goSemanticDeclarationTest,
		goSemanticDeclarationParameter, goSemanticDeclarationVariable, goSemanticDeclarationField,
		goSemanticDeclarationConstant:
		return true
	default:
		return false
	}
}

func identityMatchesDeclarationKind(identity string, kind goSemanticDeclarationKind) bool {
	prefix, _, _ := strings.Cut(identity, ":")
	switch kind {
	case goSemanticDeclarationPackage:
		return prefix == "package"
	case goSemanticDeclarationImport:
		return prefix == "import"
	case goSemanticDeclarationNamespace:
		return prefix == "namespace"
	case goSemanticDeclarationType:
		return prefix == "type" || prefix == "type-expression"
	case goSemanticDeclarationFunction:
		return prefix == "function" || prefix == "closure" || prefix == "ssa-function"
	case goSemanticDeclarationMethod:
		return prefix == "method" || prefix == "interface-method" || prefix == "dynamic-method"
	case goSemanticDeclarationTest:
		return prefix == "test"
	case goSemanticDeclarationParameter:
		return prefix == "parameter"
	case goSemanticDeclarationVariable:
		return prefix == "variable" || prefix == "capture"
	case goSemanticDeclarationField:
		return prefix == "field"
	case goSemanticDeclarationConstant:
		return prefix == "constant"
	default:
		return false
	}
}

func knownGoSemanticRelationship(kind goSemanticRelationshipKind) bool {
	switch kind {
	case goSemanticRelationshipImplements, goSemanticRelationshipExtends,
		goSemanticRelationshipOverrides, goSemanticRelationshipReferences:
		return true
	default:
		return false
	}
}

func knownGoSemanticCall(kind goSemanticCallKind) bool {
	switch kind {
	case goSemanticCallDefinite, goSemanticCallPossible, goSemanticCallConversion:
		return true
	default:
		return false
	}
}

func knownGoSemanticCallClass(class goSemanticCallClass) bool {
	switch class {
	case goSemanticCallClassInternal, goSemanticCallClassExternal,
		goSemanticCallClassBuiltin, goSemanticCallClassConversion,
		goSemanticCallClassDynamic, goSemanticCallClassInterface:
		return true
	default:
		return false
	}
}

func knownGoSemanticDataflow(kind goSemanticDataflowKind) bool {
	return kind == goSemanticDataflowRead || kind == goSemanticDataflowWrite
}

func knownGoSemanticUnresolved(reason goSemanticUnresolvedReason) bool {
	switch reason {
	case goSemanticUnresolvedMissing, goSemanticUnresolvedAmbiguous, goSemanticUnresolvedUnsupported,
		goSemanticUnresolvedDynamic, goSemanticUnresolvedExternal, goSemanticUnresolvedBuiltin,
		goSemanticUnresolvedTypeConversion, goSemanticUnresolvedSelf:
		return true
	default:
		return false
	}
}

func knownGoSemanticSeverity(severity goSemanticDiagnosticSeverity) bool {
	switch severity {
	case goSemanticDiagnosticInfo, goSemanticDiagnosticWarning, goSemanticDiagnosticError:
		return true
	default:
		return false
	}
}
