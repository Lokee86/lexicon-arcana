package main

import (
	"go/ast"
	"go/types"

	"golang.org/x/tools/go/packages"
)

func (index *semanticIndex) collectTargets(pkg *packages.Package) {
	if pkg.TypesInfo == nil || pkg.Fset == nil {
		return
	}
	for _, file := range pkg.Syntax {
		filename := pkg.Fset.PositionFor(file.Pos(), false).Filename
		owner, ok := index.ownerForPosition(filename)
		if !ok {
			continue
		}
		importPath := moduleImportPath(index.request, owner)
		for _, declaration := range file.Decls {
			switch declaration := declaration.(type) {
			case *ast.FuncDecl:
				object, ok := pkg.TypesInfo.Defs[declaration.Name].(*types.Func)
				if !ok {
					continue
				}
				kind, identity := declarationIdentity(importPath, owner, declaration)
				index.addTarget(typedTarget{
					Identity:   identity,
					SemanticID: semanticFunctionIdentity(index.request.Modules, object),
					Kind:       kind,
					Owner:      owner,
					Span:       sourceSpan(pkg.Fset, declaration.Pos(), declaration.End()),
					Object:     object,
				})
			case *ast.GenDecl:
				index.collectInterfaceTargets(pkg, owner, importPath, declaration)
			}
		}
	}
}

func (index *semanticIndex) collectInterfaceTargets(
	pkg *packages.Package,
	owner, importPath string,
	declaration *ast.GenDecl,
) {
	for _, specification := range declaration.Specs {
		typeSpec, ok := specification.(*ast.TypeSpec)
		if !ok {
			continue
		}
		interfaceType, ok := typeSpec.Type.(*ast.InterfaceType)
		if !ok || interfaceType.Methods == nil {
			continue
		}
		for _, field := range interfaceType.Methods.List {
			for _, name := range field.Names {
				object, ok := pkg.TypesInfo.Defs[name].(*types.Func)
				if !ok {
					continue
				}
				identity := interfaceMethodIdentity(importPath, typeSpec.Name.Name, name.Name)
				index.addTarget(typedTarget{
					Identity: identity, SemanticID: identity, Kind: "method",
					Owner: owner, Span: sourceSpan(pkg.Fset, field.Pos(), field.End()),
					Object: object,
				})
			}
		}
	}
}

func (index *semanticIndex) addTarget(target typedTarget) {
	index.targetsByObject[target.Object] = target
	current := index.targetsByID[target.SemanticID]
	for _, existing := range current {
		if existing.Identity == target.Identity && existing.Owner == target.Owner {
			return
		}
	}
	index.targetsByID[target.SemanticID] = append(current, target)
}

func semanticFunctionIdentity(modules []module, function *types.Func) string {
	if origin := function.Origin(); origin != nil {
		function = origin
	}
	namespace := canonicalNamespace(modules, objectNamespace(function))
	signature, _ := function.Type().(*types.Signature)
	if signature != nil && signature.Recv() != nil {
		return "method:" + namespace + ":" +
			receiverTypeName(signature.Recv().Type()) + "." + function.Name()
	}
	return "function:" + namespace + ":" + function.Name()
}

func objectNamespace(object types.Object) string {
	if object == nil || object.Pkg() == nil {
		return ""
	}
	return object.Pkg().Path()
}

func receiverTypeName(value types.Type) string {
	value = types.Unalias(value)
	if pointer, ok := value.(*types.Pointer); ok {
		return "*" + receiverTypeName(pointer.Elem())
	}
	if named, ok := value.(*types.Named); ok {
		return named.Obj().Name()
	}
	return types.TypeString(value, func(*types.Package) string { return "" })
}

func canonicalNamespace(modules []module, namespace string) string {
	if namespace == "" {
		return namespace
	}
	if base, ok := trimTestNamespace(namespace); ok && internalNamespace(modules, base) {
		return base
	}
	return namespace
}

func trimTestNamespace(namespace string) (string, bool) {
	const suffix = "_test"
	if len(namespace) <= len(suffix) ||
		namespace[len(namespace)-len(suffix):] != suffix {
		return "", false
	}
	return namespace[:len(namespace)-len(suffix)], true
}
