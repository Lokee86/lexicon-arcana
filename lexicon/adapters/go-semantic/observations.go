package main

type observation interface {
	observation()
}

type declarationObservation struct {
	Observation string            `json:"observation"`
	SemanticKey string            `json:"semantic_key"`
	Kind        string            `json:"kind"`
	Name        string            `json:"name"`
	Owner       string            `json:"owner"`
	Span        span              `json:"span"`
	Metadata    map[string]string `json:"metadata,omitempty"`
}

func (declarationObservation) observation() {}

type relationshipObservation struct {
	Observation string `json:"observation"`
	SourceKey   string `json:"source_key"`
	TargetKey   string `json:"target_key"`
	Kind        string `json:"kind"`
	Owner       string `json:"owner"`
	Span        *span  `json:"span,omitempty"`
}

func (relationshipObservation) observation() {}

type symbolObservation struct {
	Observation  string `json:"observation"`
	SemanticKey  string `json:"semantic_key"`
	Name         string `json:"name,omitempty"`
	Namespace    string `json:"namespace,omitempty"`
	ContainerKey string `json:"container_key,omitempty"`
	Owner        string `json:"owner,omitempty"`
	Span         *span  `json:"span,omitempty"`
	Generated    bool   `json:"generated,omitempty"`
}

func (symbolObservation) observation() {}

type callTargetObservation struct {
	SemanticKey  string `json:"semantic_key"`
	Name         string `json:"name,omitempty"`
	Namespace    string `json:"namespace,omitempty"`
	ContainerKey string `json:"container_key,omitempty"`
	Owner        string `json:"owner,omitempty"`
	Span         *span  `json:"span,omitempty"`
	Generated    bool   `json:"generated,omitempty"`
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
	Span               span                    `json:"span"`
}

func (callsiteObservation) observation() {}

type dataflowObservation struct {
	Observation string `json:"observation"`
	SourceKey   string `json:"source_key"`
	TargetKey   string `json:"target_key"`
	Access      string `json:"access"`
	Owner       string `json:"owner"`
	Span        span   `json:"span"`
}

func (dataflowObservation) observation() {}

type captureObservation struct {
	Observation  string `json:"observation"`
	SourceKey    string `json:"source_key"`
	TargetKey    string `json:"target_key,omitempty"`
	TargetName   string `json:"target_name"`
	CaptureIndex int    `json:"capture_index"`
	Owner        string `json:"owner"`
	Span         *span  `json:"span,omitempty"`
}

func (captureObservation) observation() {}

type diagnosticObservation struct {
	Observation string `json:"observation"`
	Severity    string `json:"severity"`
	Code        string `json:"code"`
	Message     string `json:"message"`
	Owner       string `json:"owner,omitempty"`
	Span        *span  `json:"span,omitempty"`
}

func (diagnosticObservation) observation() {}

type span struct {
	StartLine   uint32 `json:"start_line"`
	StartColumn uint32 `json:"start_column"`
	EndLine     uint32 `json:"end_line"`
	EndColumn   uint32 `json:"end_column"`
}
