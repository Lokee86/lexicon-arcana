package main

type semanticRecord interface {
	semanticRecord()
}

type declaration struct {
	Record   string
	Identity string
	Kind     string
	Name     string
	Owner    string
	Span     span
	Metadata map[string]string
}

func (declaration) semanticRecord() {}

type relationship struct {
	Record       string
	Source       string
	Target       string
	Kind         string
	TargetName   string
	CaptureIndex *int
	Owner        string
	Span         *span
}

func (relationship) semanticRecord() {}

type callObservation struct {
	Record          string
	Source          string
	Target          string
	Kind            string
	Class           string
	TargetName      string
	TargetNamespace string
	TargetContainer string
	TargetOwner     string
	TargetSpan      *span
	Owner           string
	Span            span
}

func (callObservation) semanticRecord() {}

type targetObservation struct {
	Record    string
	Identity  string
	Class     string
	Name      string
	Namespace string
	Container string
}

func (targetObservation) semanticRecord() {}

type dataflowObservation struct {
	Record string
	Source string
	Target string
	Kind   string
	Owner  string
	Span   span
}

func (dataflowObservation) semanticRecord() {}

type unresolvedObservation struct {
	Record             string
	Source             string
	Relation           string
	Expression         string
	CandidateNamespace string
	CandidateName      string
	Reason             string
	Class              string
	Owner              string
	Span               span
}

func (unresolvedObservation) semanticRecord() {}

type diagnostic struct {
	Record   string
	Severity string
	Code     string
	Message  string
	Owner    string
	Span     *span
}

func (diagnostic) semanticRecord() {}

type span struct {
	StartLine   uint32
	StartColumn uint32
	EndLine     uint32
	EndColumn   uint32
}
