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
	request  request
	set      *token.FileSet
	records  []semanticRecord
	files    []structuralFile
	closures map[string]bool
	index    *semanticIndex
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

	index, diagnostics := loadSemanticIndexProfiled(value, profile)
	index.structuralClosures = scanner.closures
	scanner.index = index

	var relationshipsStarted time.Time
	if profile != nil {
		relationshipsStarted = time.Now()
	}
	for _, record := range index.collectRelationships() {
		scanner.records = append(scanner.records, record)
	}
	if profile != nil {
		profile.Relationships = time.Since(relationshipsStarted)
	}

	var callsStarted time.Time
	if profile != nil {
		callsStarted = time.Now()
	}
	dataflow, directCalls, err := index.collectParallelSemantics(value.Execution)
	if err != nil {
		return response{}, err
	}
	if profile != nil {
		profile.RawCalls = len(directCalls)
		profile.RawDataflow = len(dataflow)
	}
	directCalls = mergeDirectCallRecords(directCalls)
	if profile != nil {
		profile.CompactedCalls = len(directCalls)
		profile.CompactedDataflow = len(dataflow)
		profile.CallsDataflow = time.Since(callsStarted)
	}

	for _, record := range dataflow {
		scanner.records = append(scanner.records, record)
	}
	var ssaStarted time.Time
	if profile != nil {
		ssaStarted = time.Now()
	}
	resolvedCalls := index.mergeSSASemantics(directCalls)
	if profile != nil {
		profile.SSAVTA = time.Since(ssaStarted)
	}
	for _, record := range resolvedCalls {
		scanner.records = append(scanner.records, record)
	}
	for _, record := range scanner.collectFallbackCalls(resolvedCalls) {
		scanner.records = append(scanner.records, record)
	}
	for _, record := range diagnostics {
		scanner.records = append(scanner.records, record)
	}
	return response{ProtocolVersion: protocolVersion, Records: scanner.records}, nil
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
	kind, identity, name, owner string,
	location span,
	metadata map[string]string,
) {
	scanner.records = append(scanner.records, declaration{
		Record: "declaration", Identity: identity, Kind: kind, Name: name,
		Owner: owner, Span: location, Metadata: metadata,
	})
}
