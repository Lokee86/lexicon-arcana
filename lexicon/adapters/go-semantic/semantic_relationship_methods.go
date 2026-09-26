package main

import (
	"go/types"
	"sort"
)

func (index *semanticIndex) implementedMethodRelationships(
	candidate, contract typedType,
) []relationship {
	receiver := types.Type(candidate.Named)
	if !types.Implements(receiver, contract.Interface) {
		receiver = types.NewPointer(candidate.Named)
	}
	methodSet := types.NewMethodSet(receiver)
	var result []relationship
	for position := 0; position < contract.Interface.NumMethods(); position++ {
		interfaceMethod := contract.Interface.Method(position)
		selection := methodSet.Lookup(interfaceMethod.Pkg(), interfaceMethod.Name())
		if selection == nil {
			continue
		}
		concrete, ok := selection.Obj().(*types.Func)
		if !ok {
			continue
		}
		concreteTargets := index.targetCandidates(concrete)
		interfaceTargets := index.targetCandidates(interfaceMethod)
		if len(concreteTargets) == 1 && len(interfaceTargets) == 1 &&
			concreteTargets[0].Identity != interfaceTargets[0].Identity {
			result = append(result, relation(
				concreteTargets[0].Identity,
				interfaceTargets[0].Identity,
				"implements",
				concreteTargets[0].Owner,
				concreteTargets[0].Span,
			))
		}
	}
	return result
}

func (index *semanticIndex) targetCandidates(function *types.Func) []typedTarget {
	if target, exists := index.targetsByObject[function]; exists {
		return []typedTarget{target}
	}
	semanticID := semanticFunctionIdentity(index.request.Modules, function)
	return append([]typedTarget(nil), index.targetsByID[semanticID]...)
}

func typeIdentity(modules []module, named *types.Named) string {
	object := named.Obj()
	namespace := canonicalNamespace(modules, objectNamespace(object))
	return "type:" + namespace + ":" + object.Name()
}

func dereference(value types.Type) types.Type {
	for {
		pointer, ok := value.(*types.Pointer)
		if !ok {
			return value
		}
		value = pointer.Elem()
	}
}

func relation(source, target, kind, owner string, evidence span) relationship {
	return relationship{
		Record: "relationship", Source: source, Target: target,
		Kind: kind, Owner: owner, Span: &evidence,
	}
}

func sortRelationships(values []relationship) {
	sort.Slice(values, func(i, j int) bool {
		left, right := values[i], values[j]
		if left.Source != right.Source {
			return left.Source < right.Source
		}
		if left.Target != right.Target {
			return left.Target < right.Target
		}
		return left.Kind < right.Kind
	})
}

func uniqueRelationships(values []relationship) []relationship {
	if len(values) < 2 {
		return values
	}
	result := values[:1]
	for _, value := range values[1:] {
		last := result[len(result)-1]
		if value.Source != last.Source || value.Target != last.Target || value.Kind != last.Kind {
			result = append(result, value)
		}
	}
	return result
}
