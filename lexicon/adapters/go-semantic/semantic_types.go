package main

import (
	"go/types"
	"sort"

	"golang.org/x/tools/go/packages"
)

func (index *semanticIndex) collectTypes(values []*packages.Package) {
	for _, pkg := range values {
		if pkg.Types == nil || pkg.Fset == nil {
			continue
		}
		namespace := canonicalNamespace(index.request.Modules, pkg.Types.Path())
		if !internalNamespace(index.request.Modules, namespace) {
			continue
		}
		names := pkg.Types.Scope().Names()
		sort.Strings(names)
		for _, name := range names {
			typeName, ok := pkg.Types.Scope().Lookup(name).(*types.TypeName)
			if !ok {
				continue
			}
			named, ok := types.Unalias(typeName.Type()).(*types.Named)
			if !ok {
				continue
			}
			position := pkg.Fset.PositionFor(typeName.Pos(), false)
			owner, ok := index.ownerForPosition(position.Filename)
			if !ok {
				continue
			}
			object := named.Obj()
			typeNamespace := canonicalNamespace(index.request.Modules, objectNamespace(object))
			identity := "type:" + typeNamespace + ":" + object.Name()
			if _, exists := index.typesByID[identity]; exists {
				continue
			}
			entry := typedType{
				Identity: identity,
				Owner:    owner,
				Span:     pointSpan(position),
				Named:    named,
			}
			if iface, ok := named.Underlying().(*types.Interface); ok {
				iface.Complete()
				entry.Interface = iface
				index.ensureInterfaceTargets(pkg, entry)
			}
			index.typesByID[identity] = entry
		}
	}
}

func (index *semanticIndex) ensureInterfaceTargets(
	pkg *packages.Package,
	entry typedType,
) {
	if entry.Interface == nil {
		return
	}
	namespace := canonicalNamespace(index.request.Modules, objectNamespace(entry.Named.Obj()))
	for position := 0; position < entry.Interface.NumExplicitMethods(); position++ {
		method := entry.Interface.ExplicitMethod(position)
		if _, exists := index.targetsByObject[method]; exists {
			continue
		}
		if !index.objectIsAllowed(pkg, method) {
			continue
		}
		owner, _ := index.ownerForPosition(pkg.Fset.PositionFor(method.Pos(), false).Filename)
		identity := interfaceMethodIdentity(namespace, entry.Named.Obj().Name(), method.Name())
		position := pkg.Fset.PositionFor(method.Pos(), false)
		index.addTarget(typedTarget{
			Identity: identity, SemanticID: identity, Kind: "method",
			Owner: owner, Span: pointSpan(position), Object: method,
		})
	}
}

func (index *semanticIndex) objectIsAllowed(
	pkg *packages.Package,
	object types.Object,
) bool {
	if object == nil || pkg.Fset == nil || !object.Pos().IsValid() {
		return false
	}
	position := pkg.Fset.PositionFor(object.Pos(), false)
	_, ok := index.ownerForPosition(position.Filename)
	return ok
}
