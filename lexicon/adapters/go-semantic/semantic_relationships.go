package main

import (
	"go/types"
	"sort"
)

func (index *semanticIndex) collectRelationships() []relationship {
	var concrete []typedType
	var interfaces []typedType
	for _, entry := range index.typesByID {
		if entry.Interface != nil {
			interfaces = append(interfaces, entry)
		} else {
			concrete = append(concrete, entry)
		}
	}
	sort.Slice(concrete, func(i, j int) bool { return concrete[i].Identity < concrete[j].Identity })
	sort.Slice(interfaces, func(i, j int) bool { return interfaces[i].Identity < interfaces[j].Identity })

	var result []relationship
	for _, entry := range concrete {
		if structure, ok := entry.Named.Underlying().(*types.Struct); ok {
			result = append(result, index.embeddedRelationships(entry, structure)...)
		}
	}
	for _, contract := range interfaces {
		result = append(result, index.embeddedInterfaceRelationships(contract)...)
	}
	for _, candidate := range concrete {
		for _, contract := range interfaces {
			receiver := types.Type(candidate.Named)
			if !types.Implements(receiver, contract.Interface) &&
				!types.Implements(types.NewPointer(candidate.Named), contract.Interface) {
				continue
			}
			if candidate.Identity != contract.Identity {
				result = append(result, relation(candidate.Identity, contract.Identity,
					"implements", candidate.Owner, candidate.Span))
			}
			result = append(result, index.implementedMethodRelationships(candidate, contract)...)
		}
	}
	sortRelationships(result)
	return uniqueRelationships(result)
}

func (index *semanticIndex) embeddedRelationships(
	entry typedType,
	structure *types.Struct,
) []relationship {
	var result []relationship
	for fieldIndex := 0; fieldIndex < structure.NumFields(); fieldIndex++ {
		field := structure.Field(fieldIndex)
		if !field.Embedded() {
			continue
		}
		embedded := dereference(field.Type())
		embeddedNamed, ok := types.Unalias(embedded).(*types.Named)
		if !ok {
			continue
		}
		embeddedID := typeIdentity(index.request.Modules, embeddedNamed)
		if entry.Identity != embeddedID {
			result = append(result, relation(
				entry.Identity, embeddedID, "extends", entry.Owner, entry.Span))
		}
		result = append(result, index.overrideRelationships(entry, embeddedNamed)...)
	}
	return result
}

func (index *semanticIndex) embeddedInterfaceRelationships(entry typedType) []relationship {
	var result []relationship
	for position := 0; position < entry.Interface.NumEmbeddeds(); position++ {
		embedded := dereference(entry.Interface.EmbeddedType(position))
		named, ok := types.Unalias(embedded).(*types.Named)
		if !ok {
			continue
		}
		target := typeIdentity(index.request.Modules, named)
		if entry.Identity != target {
			result = append(result, relation(
				entry.Identity, target, "extends", entry.Owner, entry.Span))
		}
	}
	return result
}

func (index *semanticIndex) overrideRelationships(
	entry typedType,
	embedded *types.Named,
) []relationship {
	var result []relationship
	methods := types.NewMethodSet(types.NewPointer(embedded))
	for position := 0; position < entry.Named.NumMethods(); position++ {
		method := entry.Named.Method(position)
		selection := methods.Lookup(method.Pkg(), method.Name())
		if selection == nil {
			continue
		}
		base, ok := selection.Obj().(*types.Func)
		if !ok {
			continue
		}
		overrider := index.targetCandidates(method)
		baseTargets := index.targetCandidates(base)
		if len(overrider) == 1 && len(baseTargets) == 1 &&
			overrider[0].Identity != baseTargets[0].Identity {
			result = append(result, relation(
				overrider[0].Identity, baseTargets[0].Identity, "overrides",
				overrider[0].Owner, overrider[0].Span))
		}
	}
	return result
}
