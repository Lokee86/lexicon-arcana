package main

const goSemanticProtocolVersion uint32 = 1

type goSemanticRequest struct {
	ProtocolVersion uint32              `json:"protocol_version"`
	RepositoryRoot  string              `json:"repository_root"`
	Files           []string            `json:"files"`
	Modules         []goSemanticModule  `json:"modules,omitempty"`
	Execution       goSemanticExecution `json:"execution"`
}

type goSemanticModule struct {
	Root string `json:"root"`
	Path string `json:"path"`
}

type goSemanticExecution struct {
	Workers    int `json:"workers"`
	Shards     int `json:"shards"`
	MergeFanIn int `json:"merge_fan_in"`
}

type goSemanticResponse struct {
	ProtocolVersion uint32
	Records         []goSemanticRecord
}

type goSemanticRecordKind string

const (
	goSemanticRecordDeclaration  goSemanticRecordKind = "declaration"
	goSemanticRecordRelationship goSemanticRecordKind = "relationship"
	goSemanticRecordCall         goSemanticRecordKind = "call"
	goSemanticRecordDataflow     goSemanticRecordKind = "dataflow"
	goSemanticRecordUnresolved   goSemanticRecordKind = "unresolved"
	goSemanticRecordDiagnostic   goSemanticRecordKind = "diagnostic"
)

type goSemanticRecord struct {
	Kind         goSemanticRecordKind
	Declaration  *goSemanticDeclaration
	Relationship *goSemanticRelationship
	Call         *goSemanticCallObservation
	Dataflow     *goSemanticDataflowObservation
	Unresolved   *goSemanticUnresolvedObservation
	Diagnostic   *goSemanticDiagnostic
}

type goSemanticSpan struct {
	StartLine   uint32 `json:"start_line"`
	StartColumn uint32 `json:"start_column"`
	EndLine     uint32 `json:"end_line"`
	EndColumn   uint32 `json:"end_column"`
}

type goSemanticDeclarationKind string

const (
	goSemanticDeclarationPackage   goSemanticDeclarationKind = "package"
	goSemanticDeclarationImport    goSemanticDeclarationKind = "import"
	goSemanticDeclarationNamespace goSemanticDeclarationKind = "namespace"
	goSemanticDeclarationType      goSemanticDeclarationKind = "type"
	goSemanticDeclarationFunction  goSemanticDeclarationKind = "function"
	goSemanticDeclarationMethod    goSemanticDeclarationKind = "method"
	goSemanticDeclarationTest      goSemanticDeclarationKind = "test"
	goSemanticDeclarationParameter goSemanticDeclarationKind = "parameter"
	goSemanticDeclarationVariable  goSemanticDeclarationKind = "variable"
	goSemanticDeclarationField     goSemanticDeclarationKind = "field"
	goSemanticDeclarationConstant  goSemanticDeclarationKind = "constant"
)

type goSemanticDeclaration struct {
	Identity string                    `json:"identity"`
	Kind     goSemanticDeclarationKind `json:"kind"`
	Name     string                    `json:"name"`
	Owner    string                    `json:"owner"`
	Span     goSemanticSpan            `json:"span"`
	Metadata map[string]string         `json:"metadata,omitempty"`
}

type goSemanticRelationshipKind string

const (
	goSemanticRelationshipImplements goSemanticRelationshipKind = "implements"
	goSemanticRelationshipExtends    goSemanticRelationshipKind = "extends"
	goSemanticRelationshipOverrides  goSemanticRelationshipKind = "overrides"
	goSemanticRelationshipReferences goSemanticRelationshipKind = "references"
)

type goSemanticRelationship struct {
	Source string                     `json:"source"`
	Target string                     `json:"target"`
	Kind   goSemanticRelationshipKind `json:"kind"`
	Owner  string                     `json:"owner"`
	Span   goSemanticSpan             `json:"span"`
}

type goSemanticCallKind string

const (
	goSemanticCallDefinite   goSemanticCallKind = "definite"
	goSemanticCallPossible   goSemanticCallKind = "possible"
	goSemanticCallConversion goSemanticCallKind = "conversion"
)

type goSemanticCallClass string

const (
	goSemanticCallClassInternal   goSemanticCallClass = "internal"
	goSemanticCallClassExternal   goSemanticCallClass = "external"
	goSemanticCallClassBuiltin    goSemanticCallClass = "builtin"
	goSemanticCallClassConversion goSemanticCallClass = "conversion"
	goSemanticCallClassDynamic    goSemanticCallClass = "dynamic"
	goSemanticCallClassInterface  goSemanticCallClass = "interface"
)

type goSemanticCallObservation struct {
	Source string              `json:"source"`
	Target string              `json:"target"`
	Kind   goSemanticCallKind  `json:"kind"`
	Class  goSemanticCallClass `json:"class"`
	Owner  string              `json:"owner"`
	Span   goSemanticSpan      `json:"span"`
}

type goSemanticDataflowKind string

const (
	goSemanticDataflowRead  goSemanticDataflowKind = "read"
	goSemanticDataflowWrite goSemanticDataflowKind = "write"
)

type goSemanticDataflowObservation struct {
	Source string                 `json:"source"`
	Target string                 `json:"target"`
	Kind   goSemanticDataflowKind `json:"kind"`
	Owner  string                 `json:"owner"`
	Span   goSemanticSpan         `json:"span"`
}

type goSemanticUnresolvedReason string

const (
	goSemanticUnresolvedMissing        goSemanticUnresolvedReason = "missing-target"
	goSemanticUnresolvedAmbiguous      goSemanticUnresolvedReason = "ambiguous-target"
	goSemanticUnresolvedUnsupported    goSemanticUnresolvedReason = "unsupported-form"
	goSemanticUnresolvedDynamic        goSemanticUnresolvedReason = "dynamic-target"
	goSemanticUnresolvedExternal       goSemanticUnresolvedReason = "external-target"
	goSemanticUnresolvedBuiltin        goSemanticUnresolvedReason = "builtin-target"
	goSemanticUnresolvedTypeConversion goSemanticUnresolvedReason = "type-conversion"
	goSemanticUnresolvedSelf           goSemanticUnresolvedReason = "self-target"
)

type goSemanticUnresolvedObservation struct {
	Source             string                     `json:"source"`
	Relation           string                     `json:"relation"`
	Expression         string                     `json:"expression"`
	CandidateNamespace string                     `json:"candidate_namespace,omitempty"`
	CandidateName      string                     `json:"candidate_name,omitempty"`
	Reason             goSemanticUnresolvedReason `json:"reason"`
	Class              goSemanticCallClass        `json:"class"`
	Owner              string                     `json:"owner"`
	Span               goSemanticSpan             `json:"span"`
}

type goSemanticDiagnosticSeverity string

const (
	goSemanticDiagnosticInfo    goSemanticDiagnosticSeverity = "info"
	goSemanticDiagnosticWarning goSemanticDiagnosticSeverity = "warning"
	goSemanticDiagnosticError   goSemanticDiagnosticSeverity = "error"
)

type goSemanticDiagnostic struct {
	Severity goSemanticDiagnosticSeverity `json:"severity"`
	Code     string                       `json:"code"`
	Message  string                       `json:"message"`
	Owner    string                       `json:"owner,omitempty"`
	Span     *goSemanticSpan              `json:"span,omitempty"`
}
