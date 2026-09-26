package main

import (
	"go/types"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"golang.org/x/tools/go/packages"
)

type semanticIndex struct {
	request                  request
	roots                    []*packages.Package
	packages                 []*packages.Package
	targetsByObject          map[*types.Func]typedTarget
	targetsByID              map[string][]typedTarget
	typesByID                map[string]typedType
	interfaceImplementations map[string][]typedTarget
	allowedFiles             map[string]bool
	callsiteKeys             map[string]string
	structuralClosures       map[string]bool
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
	Identity  string
	Owner     string
	Span      span
	Named     *types.Named
	Interface *types.Interface
}

func loadSemanticModuleIndexProfiled(
	value request,
	module module,
	profile *performanceProfile,
) (*semanticIndex, []diagnostic) {
	index := newSemanticIndex(value)
	root := value.RepositoryRoot
	if module.Root != "." {
		root = filepath.Join(root, filepath.FromSlash(module.Root))
	}

	var loadStarted time.Time
	if profile != nil {
		loadStarted = time.Now()
	}
	roots, err := packages.Load(&packages.Config{
		Mode:  packages.LoadAllSyntax | packages.NeedModule,
		Dir:   root,
		Tests: true,
	}, "./...")
	if profile != nil {
		profile.PackageLoad += time.Since(loadStarted)
	}
	if err != nil {
		return index, []diagnostic{packageLoadDiagnostic(module.Path, err)}
	}

	index.roots = roots
	index.packages = flattenPackages(roots)
	if profile != nil {
		profile.LoadedPackages += len(index.packages)
		if len(index.packages) > profile.PeakLivePackages {
			profile.PeakLivePackages = len(index.packages)
		}
	}
	diagnostics := packageDiagnostics(index.packages)

	var indexStarted time.Time
	if profile != nil {
		indexStarted = time.Now()
	}
	for _, pkg := range index.packages {
		index.collectTargets(pkg)
	}
	index.collectTypes(index.packages)
	if profile != nil {
		profile.SemanticIndex += time.Since(indexStarted)
		profile.TypedTargets += len(index.targetsByObject)
		profile.TypedTypes += len(index.typesByID)
	}

	sort.Slice(diagnostics, func(i, j int) bool {
		if diagnostics[i].Code != diagnostics[j].Code {
			return diagnostics[i].Code < diagnostics[j].Code
		}
		return diagnostics[i].Message < diagnostics[j].Message
	})
	return index, diagnostics
}

func newSemanticIndex(value request) *semanticIndex {
	index := &semanticIndex{
		request:                  value,
		targetsByObject:          make(map[*types.Func]typedTarget),
		targetsByID:              make(map[string][]typedTarget),
		typesByID:                make(map[string]typedType),
		interfaceImplementations: make(map[string][]typedTarget),
		allowedFiles:             make(map[string]bool),
		callsiteKeys:             make(map[string]string),
		structuralClosures:       make(map[string]bool),
	}
	for _, file := range value.Files {
		if filepath.Ext(file) == ".go" {
			index.allowedFiles[file] = true
		}
	}
	return index
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
