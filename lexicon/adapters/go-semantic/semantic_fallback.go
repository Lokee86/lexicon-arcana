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

func (scanner *structuralScanner) collectFallbackCalls(
	covered []callsiteObservation,
) []callsiteObservation {
	coveredKeys := make(map[string]bool)
	for _, value := range covered {
		coveredKeys[callsiteObservationKey(value)] = true
	}
	targets := scanner.fallbackTargets()
	var result []callsiteObservation
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
	sortCallsiteObservations(result)
	return result
}

func (scanner *structuralScanner) collectFallbackCallable(
	file structuralFile,
	source string,
	body *ast.BlockStmt,
	imports map[string]string,
	targets map[string][]string,
	covered map[string]bool,
	result *[]callsiteObservation,
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
) callsiteObservation {
	if literal := fallbackFuncLiteral(call.Fun); literal != nil {
		position := scanner.set.PositionFor(literal.Pos(), false)
		return resolvedCall(
			source,
			closureIdentity(file.importPath, file.owner, position),
			"dynamic",
			file.owner,
			location,
		)
	}
	if selector, ok := call.Fun.(*ast.SelectorExpr); ok {
		receiver := expressionName(selector.X)
		if identifier, ok := selector.X.(*ast.Ident); ok {
			if namespace := imports[identifier.Name]; namespace != "" {
				return resolvedCall(
					source,
					"function:"+canonicalNamespace(scanner.request.Modules, namespace)+":"+selector.Sel.Name,
					"direct",
					file.owner,
					location,
				)
			}
		}
		return resolvedCall(
			source,
			"dynamic-method:"+receiver+"."+selector.Sel.Name,
			"dynamic",
			file.owner,
			location,
		)
	}
	identifier, ok := call.Fun.(*ast.Ident)
	if !ok {
		resolution, namespace, name := classifyCallExpression(call.Fun)
		return unresolvedCall(
			scanner.set, source, file.owner, call,
			resolution, namespace, name, "dynamic", location,
		)
	}
	candidates := targets[identifier.Name]
	switch len(candidates) {
	case 0:
		if fallbackBuiltin(identifier.Name) {
			return resolvedCall(
				source,
				"function:go:builtins:"+identifier.Name,
				"builtin",
				file.owner,
				location,
			)
		}
		return unresolvedCall(
			scanner.set, source, file.owner, call,
			"missing", file.importPath, identifier.Name, "direct", location,
		)
	case 1:
		return resolvedCall(source, candidates[0], "direct", file.owner, location)
	default:
		return unresolvedCall(
			scanner.set, source, file.owner, call,
			"ambiguous", file.importPath, identifier.Name, "direct", location,
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
