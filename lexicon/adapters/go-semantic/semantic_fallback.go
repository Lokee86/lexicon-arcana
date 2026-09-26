package main

import (
	"go/ast"
	"path"
	"sort"
	"strconv"
)

type structuralFile struct {
	owner       string
	importPath  string
	packageName string
	file        *ast.File
}

func (scanner *structuralScanner) collectFallbackCalls(covered []semanticRecord) []semanticRecord {
	coveredKeys := make(map[string]bool)
	for _, record := range covered {
		switch record.(type) {
		case callObservation, unresolvedObservation:
			coveredKeys[recordCallsiteKey(record)] = true
		}
	}
	targets := scanner.fallbackTargets()
	var result []semanticRecord
	for _, file := range scanner.files {
		imports := scanner.fallbackImports(file.file)
		scope := file.importPath + "\x00" + file.packageName
		for _, declaration := range file.file.Decls {
			function, ok := declaration.(*ast.FuncDecl)
			if !ok || function.Body == nil {
				continue
			}
			_, source := declarationIdentity(file.importPath, file.owner, function)
			scanner.collectFallbackCallable(
				file, source, function.Body, imports, targets[scope], coveredKeys, &result,
			)
		}
	}
	sortSemanticCallRecords(result)
	return result
}

func (scanner *structuralScanner) collectFallbackCallable(
	file structuralFile,
	source string,
	body *ast.BlockStmt,
	imports map[string]string,
	targets map[string][]string,
	covered map[string]bool,
	result *[]semanticRecord,
) {
	ast.Inspect(body, func(node ast.Node) bool {
		if node != body {
			if literal, ok := node.(*ast.FuncLit); ok {
				position := scanner.set.PositionFor(literal.Pos(), false)
				closure := closureIdentity(file.importPath, file.owner, position)
				scanner.collectFallbackCallable(
					file, closure, literal.Body, imports, targets, covered, result,
				)
				return false
			}
		}
		call, ok := node.(*ast.CallExpr)
		if !ok {
			return true
		}
		location := sourceSpan(scanner.set, call.Pos(), call.End())
		key := callsiteRecordKey(source, file.owner, location)
		if covered[key] {
			return true
		}
		*result = append(*result, scanner.resolveFallbackCall(
			file, source, call, location, imports, targets,
		))
		covered[key] = true
		return true
	})
}

func (scanner *structuralScanner) resolveFallbackCall(
	file structuralFile,
	source string,
	call *ast.CallExpr,
	location span,
	imports map[string]string,
	targets map[string][]string,
) semanticRecord {
	if literal := fallbackFuncLiteral(call.Fun); literal != nil {
		position := scanner.set.PositionFor(literal.Pos(), false)
		return callRecord(
			source,
			closureIdentity(file.importPath, file.owner, position),
			"definite",
			"dynamic",
			file.owner,
			location,
		)
	}
	if selector, ok := call.Fun.(*ast.SelectorExpr); ok {
		receiver := expressionName(selector.X)
		if identifier, ok := selector.X.(*ast.Ident); ok {
			if namespace := imports[identifier.Name]; namespace != "" {
				class := "external"
				if internalNamespace(scanner.request.Modules, namespace) {
					class = "internal"
				}
				return callRecord(
					source,
					"function:"+canonicalNamespace(scanner.request.Modules, namespace)+":"+selector.Sel.Name,
					"definite",
					class,
					file.owner,
					location,
				)
			}
		}
		return callRecord(
			source,
			"dynamic-method:"+receiver+"."+selector.Sel.Name,
			"definite",
			"dynamic",
			file.owner,
			location,
		)
	}
	identifier, ok := call.Fun.(*ast.Ident)
	if !ok {
		reason, namespace, name := classifyCallExpression(call.Fun)
		return unresolvedForPackage(
			scanner.set, source, file.owner, call,
			reason, namespace, name, "dynamic", location,
		)
	}
	candidates := targets[identifier.Name]
	switch len(candidates) {
	case 0:
		if fallbackBuiltin(identifier.Name) {
			return callRecord(
				source,
				"function:go:builtins:"+identifier.Name,
				"definite",
				"builtin",
				file.owner,
				location,
			)
		}
		return unresolvedForPackage(
			scanner.set, source, file.owner, call,
			"missing-target", file.importPath, identifier.Name, "internal", location,
		)
	case 1:
		return callRecord(
			source, candidates[0], "definite", "internal", file.owner, location,
		)
	default:
		return unresolvedForPackage(
			scanner.set, source, file.owner, call,
			"ambiguous-target", file.importPath, identifier.Name, "internal", location,
		)
	}
}

func (scanner *structuralScanner) fallbackTargets() map[string]map[string][]string {
	result := make(map[string]map[string][]string)
	for _, file := range scanner.files {
		scope := file.importPath + "\x00" + file.packageName
		for _, declaration := range file.file.Decls {
			function, ok := declaration.(*ast.FuncDecl)
			if !ok || function.Recv != nil {
				continue
			}
			_, identity := declarationIdentity(file.importPath, file.owner, function)
			if result[scope] == nil {
				result[scope] = make(map[string][]string)
			}
			result[scope][function.Name.Name] = appendUniqueString(
				result[scope][function.Name.Name], identity,
			)
		}
	}
	return result
}

func (scanner *structuralScanner) fallbackImports(file *ast.File) map[string]string {
	result := make(map[string]string)
	for _, spec := range file.Imports {
		importPath, err := strconv.Unquote(spec.Path.Value)
		if err != nil {
			continue
		}
		alias := path.Base(importPath)
		if spec.Name != nil {
			alias = spec.Name.Name
		}
		if alias != "_" && alias != "." {
			result[alias] = importPath
		}
	}
	return result
}

func fallbackFuncLiteral(expression ast.Expr) *ast.FuncLit {
	for {
		switch typed := expression.(type) {
		case *ast.FuncLit:
			return typed
		case *ast.ParenExpr:
			expression = typed.X
		default:
			return nil
		}
	}
}

func fallbackBuiltin(name string) bool {
	switch name {
	case "append", "cap", "clear", "close", "complex", "copy", "delete", "imag", "len",
		"make", "max", "min", "new", "panic", "print", "println", "real", "recover":
		return true
	default:
		return false
	}
}

func appendUniqueString(values []string, value string) []string {
	for _, existing := range values {
		if existing == value {
			return values
		}
	}
	values = append(values, value)
	sort.Strings(values)
	return values
}
