package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"path"
	"path/filepath"
	"strings"
)

const protocolVersion uint32 = 1

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

type semanticRecord interface {
	semanticRecord()
}

type response struct {
	ProtocolVersion uint32           `json:"protocol_version"`
	Records         []semanticRecord `json:"records"`
}

type declaration struct {
	Record   string            `json:"record"`
	Identity string            `json:"identity"`
	Kind     string            `json:"kind"`
	Name     string            `json:"name"`
	Owner    string            `json:"owner"`
	Span     span              `json:"span"`
	Metadata map[string]string `json:"metadata,omitempty"`
}

func (declaration) semanticRecord() {}

type diagnostic struct {
	Record   string `json:"record"`
	Severity string `json:"severity"`
	Code     string `json:"code"`
	Message  string `json:"message"`
	Owner    string `json:"owner,omitempty"`
	Span     *span  `json:"span,omitempty"`
}

func (diagnostic) semanticRecord() {}

type span struct {
	StartLine   uint32 `json:"start_line"`
	StartColumn uint32 `json:"start_column"`
	EndLine     uint32 `json:"end_line"`
	EndColumn   uint32 `json:"end_column"`
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
