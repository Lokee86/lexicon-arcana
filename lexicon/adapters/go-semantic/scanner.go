package main

import (
	"fmt"
	"go/parser"
	"go/token"
	"path/filepath"
	"sort"
)

type structuralScanner struct {
	request request
	set     *token.FileSet
	records []semanticRecord
	files   []structuralFile
	index   *semanticIndex
}

func scanStructural(value request) (response, error) {
	scanner := &structuralScanner{request: value, set: token.NewFileSet()}
	files := append([]string(nil), value.Files...)
	sort.Strings(files)
	for _, owner := range files {
		if filepath.Ext(owner) != ".go" {
			continue
		}
		if err := scanner.parseFile(owner); err != nil {
			return response{}, err
		}
	}
	index, diagnostics := loadSemanticIndex(value)
	scanner.index = index
	for _, record := range index.collectRelationships() {
		scanner.records = append(scanner.records, record)
	}
	dataflow, directCalls, err := index.collectParallelSemantics(value.Execution)
	if err != nil {
		return response{}, err
	}
	for _, record := range dataflow {
		scanner.records = append(scanner.records, record)
	}
	resolvedCalls := index.mergeSSASemantics(directCalls)
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
