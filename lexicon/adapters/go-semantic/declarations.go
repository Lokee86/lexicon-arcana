package main

import (
	"fmt"
	"go/ast"
)

func (scanner *structuralScanner) addDeclaration(
	owner, importPath, pkgIdentity string,
	value ast.Decl,
) {
	switch value := value.(type) {
	case *ast.GenDecl:
		for _, specification := range value.Specs {
			typeSpec, ok := specification.(*ast.TypeSpec)
			if !ok {
				continue
			}
			identity := "type:" + importPath + ":" + typeSpec.Name.Name
			scanner.add("type", identity, typeSpec.Name.Name, owner,
				sourceSpan(scanner.set, typeSpec.Pos(), typeSpec.End()),
				containerMetadata(pkgIdentity))
			if interfaceType, ok := typeSpec.Type.(*ast.InterfaceType); ok {
				scanner.addInterfaceMethods(owner, importPath, identity, typeSpec.Name.Name, interfaceType)
			}
		}
	case *ast.FuncDecl:
		kind, identity := declarationIdentity(importPath, owner, value)
		scanner.add(kind, identity, value.Name.Name, owner,
			sourceSpan(scanner.set, value.Pos(), value.End()),
			containerMetadata(pkgIdentity))
		if value.Body != nil {
			scanner.collectClosures(owner, importPath, identity, value.Body)
		}
	}
}

func (scanner *structuralScanner) addInterfaceMethods(
	owner, importPath, container, interfaceName string,
	interfaceType *ast.InterfaceType,
) {
	if interfaceType.Methods == nil {
		return
	}
	for _, field := range interfaceType.Methods.List {
		if len(field.Names) == 0 {
			continue
		}
		if _, ok := field.Type.(*ast.FuncType); !ok {
			continue
		}
		for _, name := range field.Names {
			scanner.add("method",
				interfaceMethodIdentity(importPath, interfaceName, name.Name),
				name.Name, owner,
				sourceSpan(scanner.set, field.Pos(), field.End()),
				containerMetadata(container))
		}
	}
}

func (scanner *structuralScanner) collectClosures(
	owner, importPath, parent string,
	body *ast.BlockStmt,
) {
	ast.Inspect(body, func(node ast.Node) bool {
		literal, ok := node.(*ast.FuncLit)
		if !ok {
			return true
		}
		position := scanner.set.PositionFor(literal.Pos(), false)
		identity := closureIdentity(importPath, owner, position)
		if scanner.closures == nil {
			scanner.closures = make(map[string]bool)
		}
		scanner.closures[identity] = true
		scanner.add(
			"function",
			identity,
			fmt.Sprintf("closure@%d:%d", position.Line, position.Column),
			owner,
			sourceSpan(scanner.set, literal.Pos(), literal.End()),
			containerMetadata(parent),
		)
		scanner.collectClosures(owner, importPath, identity, literal.Body)
		return false
	})
}

func containerMetadata(identity string) map[string]string {
	return map[string]string{"container": identity}
}
