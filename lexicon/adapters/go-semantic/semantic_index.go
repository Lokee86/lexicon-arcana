package main

import (
	"go/types"
	"path/filepath"
	"sort"
	"strings"

	"golang.org/x/tools/go/packages"
)

type semanticIndex struct {
	request         request
	roots           []*packages.Package
	packages        []*packages.Package
	targetsByObject map[*types.Func]typedTarget
	targetsByID     map[string][]typedTarget
	typesByID       map[string]typedType
	allowedFiles    map[string]bool
}

type typedTarget struct {
	Identity   string
	SemanticID string
	Kind       string
	Owner      string
	Span       span
	Object     *types.Func
}

type typedType struct {
	Identity       string
	Owner          string
	Span           span
	Named          *types.Named
	Interface      *types.Interface
	ValueMethods   []string
	PointerMethods []string
}

func loadSemanticIndex(value request) (*semanticIndex, []diagnostic) {
	index := &semanticIndex{
		request:         value,
		targetsByObject: make(map[*types.Func]typedTarget),
		targetsByID:     make(map[string][]typedTarget),
		typesByID:       make(map[string]typedType),
		allowedFiles:    make(map[string]bool),
	}
	for _, file := range value.Files {
		if filepath.Ext(file) == ".go" {
			index.allowedFiles[file] = true
		}
	}

	var diagnostics []diagnostic
	for _, module := range value.Modules {
		root := value.RepositoryRoot
		if module.Root != "." {
			root = filepath.Join(root, filepath.FromSlash(module.Root))
		}
		roots, err := packages.Load(&packages.Config{
			Mode:  packages.LoadAllSyntax | packages.NeedModule,
			Dir:   root,
			Tests: true,
		}, "./...")
		if err != nil {
			diagnostics = append(diagnostics, packageLoadDiagnostic(module.Path, err))
			continue
		}
		index.roots = append(index.roots, roots...)
		loaded := flattenPackages(roots)
		index.packages = append(index.packages, loaded...)
		diagnostics = append(diagnostics, packageDiagnostics(loaded)...)
		for _, pkg := range loaded {
			index.collectTargets(pkg)
		}
		index.collectTypes(loaded)
	}
	sort.Slice(index.packages, func(i, j int) bool {
		return index.packages[i].ID < index.packages[j].ID
	})
	sort.Slice(diagnostics, func(i, j int) bool {
		if diagnostics[i].Code != diagnostics[j].Code {
			return diagnostics[i].Code < diagnostics[j].Code
		}
		return diagnostics[i].Message < diagnostics[j].Message
	})
	return index, diagnostics
}

func flattenPackages(roots []*packages.Package) []*packages.Package {
	seen := make(map[string]*packages.Package)
	var visit func(*packages.Package)
	visit = func(pkg *packages.Package) {
		if pkg == nil || seen[pkg.ID] != nil {
			return
		}
		seen[pkg.ID] = pkg
		paths := make([]string, 0, len(pkg.Imports))
		for path := range pkg.Imports {
			paths = append(paths, path)
		}
		sort.Strings(paths)
		for _, path := range paths {
			visit(pkg.Imports[path])
		}
	}
	for _, root := range roots {
		visit(root)
	}
	result := make([]*packages.Package, 0, len(seen))
	for _, pkg := range seen {
		result = append(result, pkg)
	}
	sort.Slice(result, func(i, j int) bool { return result[i].ID < result[j].ID })
	return result
}

func packageDiagnostics(values []*packages.Package) []diagnostic {
	var result []diagnostic
	for _, pkg := range values {
		for _, issue := range pkg.Errors {
			result = append(result, diagnostic{
				Record:   "diagnostic",
				Severity: "error",
				Code:     "go-package",
				Message:  issue.Error(),
			})
		}
	}
	return result
}

func packageLoadDiagnostic(modulePath string, err error) diagnostic {
	return diagnostic{
		Record:   "diagnostic",
		Severity: "error",
		Code:     "go-package-load",
		Message:  "load Go module " + modulePath + ": " + err.Error(),
	}
}

func (index *semanticIndex) ownerForPosition(filename string) (string, bool) {
	relative, err := filepath.Rel(index.request.RepositoryRoot, filename)
	if err != nil {
		return "", false
	}
	owner := filepath.ToSlash(relative)
	if strings.HasPrefix(owner, "../") || !index.allowedFiles[owner] {
		return "", false
	}
	return owner, true
}
