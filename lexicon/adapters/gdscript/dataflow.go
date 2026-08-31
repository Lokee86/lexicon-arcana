package main

// processDataflow is intentionally token-bounded. It links only declarations
// already indexed in the repository fact set; unknown names are ignored.
func processDataflow(facts *factSet, model *semanticModel, pf *parsedFile) {
	for i := range pf.declarations {
		decl := &pf.declarations[i]
		if decl.kind != "function" || decl.nodeID == "" {
			continue
		}
		for _, name := range decl.parameterNames {
			ensureGDScriptParameter(facts, pf, decl, name)
		}
	}
	for _, stmt := range pf.statements {
		decl := declarationForStatement(pf, stmt)
		context := contextForStatement(pf, stmt)
		source := ownerForStatement(pf, stmt)
		if decl != nil && decl.ownerFunction != "" {
			source = decl.ownerFunction
		}
		if source == "" {
			continue
		}
		if decl != nil && (decl.kind == "variable" || decl.kind == "constant") {
			if decl.nodeID != "" && len(decl.initializer) > 0 {
				facts.addDataflowEdge(edge(source, decl.nodeID, "writes", decl.span))
				emitGDScriptReads(facts, model, context, source, pf, decl.initializer, nil)
			}
			continue
		}
		emitGDScriptStatement(facts, model, context, source, pf, stmt)
	}
}

func ensureGDScriptParameter(facts *factSet, pf *parsedFile, function *declaration, name string) string {
	key := function.key + "::parameter::" + name
	id := nodeID("parameter", key)
	facts.addNode(node("parameter", name, pf.path, qualifiedDeclaration(pf.path, function.key, name), id, function.span, ""))
	facts.addEdge(edge(function.nodeID, id, "defines", function.span))
	return id
}

func emitGDScriptStatement(facts *factSet, model *semanticModel, context analysisContext, source string, pf *parsedFile, stmt statement) {
	assignment := dataflowAssignmentIndex(stmt.tokens)
	if assignment >= 0 {
		left := trimExpression(stmt.tokens[:assignment])
		compound := stmt.tokens[assignment].text != "=" && stmt.tokens[assignment].text != ":="
		if target, receiver, name := gdscriptTarget(left); name != "" {
			if receiver != nil {
				emitGDScriptMemberTarget(facts, model, context, source, pf, target, receiver, name, compound)
			} else if id := gdscriptLocalTarget(facts, context.functionID, name, left[0]); id != "" {
				if compound {
					facts.addDataflowEdge(edge(source, id, "reads", spanFromTokens(pf.path, left[0], left[len(left)-1])))
				}
				facts.addDataflowEdge(edge(source, id, "writes", spanFromTokens(pf.path, left[0], left[len(left)-1])))
			}
		}
		emitGDScriptReads(facts, model, context, source, pf, stmt.tokens[assignment+1:], left)
		return
	}
	if len(stmt.tokens) > 1 && (stmt.tokens[0].text == "++" || stmt.tokens[0].text == "--" || stmt.tokens[len(stmt.tokens)-1].text == "++" || stmt.tokens[len(stmt.tokens)-1].text == "--") {
		target := stmt.tokens[1:]
		if stmt.tokens[len(stmt.tokens)-1].text == "++" || stmt.tokens[len(stmt.tokens)-1].text == "--" {
			target = stmt.tokens[:len(stmt.tokens)-1]
		}
		if left, receiver, name := gdscriptTarget(target); name != "" {
			if receiver == nil {
				if id := gdscriptLocalTarget(facts, context.functionID, name, left[0]); id != "" {
					span := spanFromTokens(pf.path, left[0], left[len(left)-1])
					facts.addDataflowEdge(edge(source, id, "reads", span))
					facts.addDataflowEdge(edge(source, id, "writes", span))
				}
			} else {
				emitGDScriptMemberTarget(facts, model, context, source, pf, left, receiver, name, true)
			}
		}
		return
	}
	emitGDScriptReads(facts, model, context, source, pf, stmt.tokens, nil)
}

func emitGDScriptReads(facts *factSet, model *semanticModel, context analysisContext, source string, pf *parsedFile, tokens []token, excluded []token) {
	for index, current := range tokens {
		if current.kind != tokenIdentifier || isCallKeyword(current.text) || isBuiltin(current.text) || current.text == "self" {
			continue
		}
		if tokenExcluded(current, excluded) || (index > 0 && tokens[index-1].text == "func") {
			continue
		}
		if index > 0 && tokens[index-1].text == "." {
			base := tokens[:index-1]
			if id := gdscriptMemberTargetID(model, context, base, current.text); id != "" {
				facts.addDataflowEdge(edge(source, id, "reads", spanFromTokens(pf.path, current, current)))
			}
			continue
		}
		if id := gdscriptLocalTarget(facts, context.functionID, current.text, current); id != "" {
			facts.addDataflowEdge(edge(source, id, "reads", spanFromTokens(pf.path, current, current)))
		}
	}
}

func emitGDScriptMemberTarget(facts *factSet, model *semanticModel, context analysisContext, source string, pf *parsedFile, target, receiver []token, name string, compound bool) {
	id := gdscriptMemberTargetID(model, context, receiver, name)
	if id == "" {
		return
	}
	span := spanFromTokens(pf.path, target[0], target[len(target)-1])
	if compound {
		facts.addDataflowEdge(edge(source, id, "reads", span))
	}
	facts.addDataflowEdge(edge(source, id, "writes", span))
}

func gdscriptLocalTarget(facts *factSet, functionID, name string, reference token) string {
	if functionID == "" {
		return ""
	}
	bestID := ""
	bestLine, bestColumn := -1, -1
	ambiguous := false
	for _, id := range facts.localValueIDs(functionID, name) {
		decl := facts.declarationByID[id]
		if decl == nil {
			continue
		}
		line := spanInt(decl.span, "start_line")
		column := spanInt(decl.span, "start_column")
		if declarationContainsToken(decl, reference) || line > reference.line || line == reference.line && column >= reference.column {
			continue
		}
		if line > bestLine || line == bestLine && column > bestColumn {
			bestID, bestLine, bestColumn, ambiguous = id, line, column, false
			continue
		}
		if line == bestLine && column == bestColumn && id != bestID {
			ambiguous = true
		}
	}
	if bestID != "" && !ambiguous {
		return bestID
	}
	function := facts.declarationByID[functionID]
	if function == nil {
		return ""
	}
	for _, parameter := range function.parameterNames {
		if parameter == name {
			return nodeID("parameter", function.key+"::parameter::"+name)
		}
	}
	return ""
}

func declarationContainsToken(decl *declaration, reference token) bool {
	startLine := spanInt(decl.span, "start_line")
	startColumn := spanInt(decl.span, "start_column")
	endLine := spanInt(decl.span, "end_line")
	endColumn := spanInt(decl.span, "end_column")
	if reference.line < startLine || reference.line > endLine {
		return false
	}
	if reference.line == startLine && reference.column < startColumn {
		return false
	}
	return reference.line != endLine || reference.column < endColumn
}

func gdscriptMemberTargetID(model *semanticModel, context analysisContext, receiver []token, name string) string {
	owners := ownerSet{}
	if simpleIdentifier(receiver) == "self" {
		if context.ownerID != "" {
			owners[context.ownerID] = struct{}{}
		}
	} else if len(receiver) > 0 {
		owners = model.inferExpressionOwners(context, receiver)
	}
	candidates := make(map[string]struct{})
	for owner := range owners {
		for _, id := range model.facts.memberValueIDs(owner, name) {
			candidates[id] = struct{}{}
		}
	}
	if len(candidates) != 1 {
		return ""
	}
	for id := range candidates {
		return id
	}
	return ""
}

func gdscriptTarget(tokens []token) (target, receiver []token, name string) {
	if len(tokens) == 1 && tokens[0].kind == tokenIdentifier {
		return tokens, nil, tokens[0].text
	}
	if len(tokens) >= 3 && tokens[len(tokens)-2].text == "." && tokens[len(tokens)-1].kind == tokenIdentifier {
		return tokens, tokens[:len(tokens)-2], tokens[len(tokens)-1].text
	}
	return nil, nil, ""
}

func dataflowAssignmentIndex(tokens []token) int {
	depth := 0
	for index, current := range tokens {
		switch current.text {
		case "(", "[", "{":
			depth++
		case ")", "]", "}":
			if depth > 0 {
				depth--
			}
		case "=", ":=", "+=", "-=", "*=", "/=":
			if depth == 0 {
				return index
			}
		}
	}
	return -1
}

func tokenExcluded(current token, excluded []token) bool {
	for _, candidate := range excluded {
		if candidate.line == current.line && candidate.column == current.column {
			return true
		}
	}
	return false
}
