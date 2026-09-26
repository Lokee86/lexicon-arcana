package main

import (
	"fmt"
	"go/ast"
	"strconv"
)

func (scanner *structuralScanner) addImport(
	owner, pkgIdentity string,
	spec *ast.ImportSpec,
) error {
	importPath, err := strconv.Unquote(spec.Path.Value)
	if err != nil {
		return fmt.Errorf("parse import in %s: %w", owner, err)
	}
	alias := ""
	if spec.Name != nil {
		alias = spec.Name.Name
	}
	class := "external"
	if internalNamespace(scanner.request.Modules, importPath) {
		class = "internal"
	}
	identity := "import:" + class + ":" + importPath
	scanner.add(
		"import",
		identity,
		importPath,
		owner,
		sourceSpan(scanner.set, spec.Pos(), spec.End()),
		map[string]string{
			"container":    pkgIdentity,
			"import_alias": alias,
			"import_class": class,
			"import_path":  importPath,
		},
	)
	return nil
}
