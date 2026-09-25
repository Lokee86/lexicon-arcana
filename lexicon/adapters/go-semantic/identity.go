package main

import (
	"go/ast"
	"go/token"
	"path"
	"strings"
)

func packageIdentity(importPath, packageName string) string {
	return "package:" + importPath + ":" + packageName
}

func declarationIdentity(importPath, owner string, declaration *ast.FuncDecl) (string, string) {
	name := declaration.Name.Name
	if declaration.Recv != nil {
		return "method", "method:" + importPath + ":" + receiverName(declaration.Recv) + "." + name
	}
	if strings.HasSuffix(owner, "_test.go") && strings.HasPrefix(name, "Test") {
		return "test", "test:" + importPath + ":" + name
	}
	return "function", "function:" + importPath + ":" + name
}

func interfaceMethodIdentity(importPath, interfaceName, methodName string) string {
	return "interface-method:" + importPath + ":" + interfaceName + "." + methodName
}

func closureIdentity(importPath, owner string, position token.Position) string {
	return "closure:" + importPath + ":" + owner + ":" +
		itoa(position.Line) + ":" + itoa(position.Column)
}

func receiverName(fields *ast.FieldList) string {
	if fields == nil || len(fields.List) == 0 {
		return ""
	}
	return expressionName(fields.List[0].Type)
}

func expressionName(expression ast.Expr) string {
	switch expression := expression.(type) {
	case *ast.Ident:
		return expression.Name
	case *ast.StarExpr:
		return "*" + expressionName(expression.X)
	case *ast.SelectorExpr:
		return expressionName(expression.X) + "." + expression.Sel.Name
	case *ast.IndexExpr:
		return expressionName(expression.X)
	case *ast.IndexListExpr:
		return expressionName(expression.X)
	case *ast.ParenExpr:
		return expressionName(expression.X)
	default:
		return "anonymous"
	}
}

func moduleImportPath(value request, owner string) string {
	selected := module{}
	selectedLength := -1
	for _, candidate := range value.Modules {
		root := candidate.Root
		if root == "." {
			root = ""
		}
		if root != "" && owner != root && !strings.HasPrefix(owner, root+"/") {
			continue
		}
		if len(root) > selectedLength {
			selected = candidate
			selectedLength = len(root)
		}
	}
	if selectedLength >= 0 {
		root := selected.Root
		if root == "." {
			root = ""
		}
		relative := strings.TrimPrefix(owner, root)
		relative = strings.TrimPrefix(relative, "/")
		directory := path.Dir(relative)
		if directory == "." || directory == "" {
			return selected.Path
		}
		return selected.Path + "/" + directory
	}
	repository := path.Base(strings.ReplaceAll(value.RepositoryRoot, "\\", "/"))
	directory := path.Dir(owner)
	if directory == "." || directory == "" {
		return repository
	}
	return repository + "/" + directory
}

func internalNamespace(modules []module, namespace string) bool {
	for _, module := range modules {
		if namespace == module.Path || strings.HasPrefix(namespace, module.Path+"/") {
			return true
		}
	}
	return false
}
