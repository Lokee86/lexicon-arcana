package main

import (
	"fmt"
	"go/parser"
	"go/token"
	"path/filepath"
	"sort"
	"time"
)

type structuralScanner struct {
	request      request
	set          *token.FileSet
	observations []observation
	files        []structuralFile
	closures     map[string]bool
}

func scanStructural(value request) (response, error) {
	return scanStructuralWithProfile(value, nil)
}

func scanStructuralProfiled(value request) (response, performanceProfile, error) {
	profile := performanceProfile{}
	result, err := scanStructuralWithProfile(value, &profile)
	return result, profile, err
}

func scanStructuralWithProfile(value request, profile *performanceProfile) (response, error) {
	scanner := &structuralScanner{request: value, set: token.NewFileSet()}
	files := append([]string(nil), value.Files...)
	sort.Strings(files)
	var parsingStarted time.Time
	if profile != nil {
		parsingStarted = time.Now()
	}
	for _, owner := range files {
		if filepath.Ext(owner) != ".go" {
			continue
		}
		if err := scanner.parseFile(owner); err != nil {
			return response{}, err
		}
		if profile != nil {
			profile.ParsedFiles++
		}
	}
	if profile != nil {
		profile.StructuralParsing = time.Since(parsingStarted)
	}

	state := newSemanticRepositoryState()
	var diagnostics []diagnosticObservation
	for _, module := range value.Modules {
		if profile != nil {
			profile.ProcessedModules++
		}
		index, moduleDiagnostics := loadSemanticModuleIndexProfiled(value, module, profile)
		index.structuralClosures = scanner.closures
		diagnostics = append(diagnostics, moduleDiagnostics...)

		var relationshipsStarted time.Time
		if profile != nil {
			relationshipsStarted = time.Now()
		}
		state.addRelationships(index.collectRelationships())
		if profile != nil {
			profile.Relationships += time.Since(relationshipsStarted)
		}

		var callsStarted time.Time
		if profile != nil {
			callsStarted = time.Now()
		}
		collection, err := index.collectParallelSemantics(value.Execution)
		if err != nil {
			return response{}, err
		}
		state.addCollection(collection)
		if profile != nil {
			profile.CallsDataflow += time.Since(callsStarted)
		}

		var ssaStarted time.Time
		if profile != nil {
			ssaStarted = time.Now()
		}
		state.addSSAObservations(index.mergeSSASemantics(
			collection.calls.observations(),
			state.resolvedCallsites(),
		))
		if profile != nil {
			profile.SSAVTA += time.Since(ssaStarted)
		}
	}

	if profile != nil {
		profile.RawCalls = state.directBeforeSSA.raw
		profile.CompactedCalls = state.directBeforeSSA.compactedCount()
		profile.RawDataflow = state.dataflow.raw
		profile.CompactedDataflow = len(state.dataflow.order)
	}

	scanner.observations = append(scanner.observations, state.relationshipObservations()...)
	scanner.observations = append(scanner.observations, state.dataflow.observations()...)
	scanner.observations = append(scanner.observations, state.symbolObservations()...)
	resolvedCalls := state.finalCalls.observations()
	scanner.observations = append(scanner.observations, callsitesAsObservations(resolvedCalls)...)
	scanner.observations = append(scanner.observations, state.captureObservations()...)
	fallback := scanner.collectFallbackCalls(resolvedCalls)
	scanner.observations = append(scanner.observations, callsitesAsObservations(fallback)...)

	sort.Slice(diagnostics, func(i, j int) bool {
		if diagnostics[i].Code != diagnostics[j].Code {
			return diagnostics[i].Code < diagnostics[j].Code
		}
		return diagnostics[i].Message < diagnostics[j].Message
	})
	for _, value := range diagnostics {
		scanner.observations = append(scanner.observations, value)
	}
	return response{ProtocolVersion: protocolVersion, Observations: scanner.observations}, nil
}

func callsitesAsObservations(values []callsiteObservation) []observation {
	result := make([]observation, 0, len(values))
	for _, value := range values {
		result = append(result, value)
	}
	return result
}

func (scanner *structuralScanner) parseFile(owner string) error {
	absolute := filepath.Join(
		scanner.request.RepositoryRoot,
		filepath.FromSlash(owner),
	)
	file, err := parser.ParseFile(scanner.set, absolute, nil, parser.ParseComments)
	if err != nil {
		return fmt.Errorf("parse %s: %w", owner, err)
	}
	importPath := moduleImportPath(scanner.request, owner)
	pkgIdentity := packageIdentity(importPath, file.Name.Name)
	scanner.add("package", pkgIdentity, file.Name.Name, owner,
		sourceSpan(scanner.set, file.Name.Pos(), file.Name.End()), nil)

	for _, spec := range file.Imports {
		if err := scanner.addImport(owner, pkgIdentity, spec); err != nil {
			return err
		}
	}
	for _, declaration := range file.Decls {
		scanner.addDeclaration(owner, importPath, pkgIdentity, declaration)
	}
	scanner.files = append(scanner.files, structuralFile{
		owner: owner, importPath: importPath, packageName: file.Name.Name, file: file,
	})
	return nil
}

func (scanner *structuralScanner) add(
	kind, semanticKey, name, owner string,
	location span,
	metadata map[string]string,
) {
	scanner.observations = append(scanner.observations, declarationObservation{
		Observation: "declaration",
		SemanticKey: semanticKey,
		Kind:        kind,
		Name:        name,
		Owner:       owner,
		Span:        location,
		Metadata:    metadata,
	})
}
