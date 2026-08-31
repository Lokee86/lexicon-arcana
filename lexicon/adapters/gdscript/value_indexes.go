package main

func (f *factSet) localValueIDs(functionID, name string) []string {
	if f.localValueByFunction != nil {
		return f.localValueByFunction[functionID][name]
	}
	return f.scanValueIDs(functionID, name, true)
}

func (f *factSet) memberValueIDs(ownerID, name string) []string {
	if f.memberValueByOwner != nil {
		return f.memberValueByOwner[ownerID][name]
	}
	return f.scanValueIDs(ownerID, name, false)
}

func (f *factSet) scanValueIDs(ownerID, name string, local bool) []string {
	result := make([]string, 0, 1)
	for id, decl := range f.declarationByID {
		if decl == nil || decl.name != name || (decl.kind != "variable" && decl.kind != "constant") {
			continue
		}
		if local && decl.ownerFunction == ownerID || !local && decl.ownerID == ownerID && decl.ownerFunction == "" {
			result = append(result, id)
		}
	}
	return result
}
