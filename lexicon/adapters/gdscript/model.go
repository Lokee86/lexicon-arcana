package main

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"path/filepath"
)

const (
	adapterVersion = "0.3.0"
	language       = "gdscript"
)

type sourceSpan = map[string]any

type factSet struct {
	nodes                       []map[string]any
	edges                       []map[string]any
	unresolved                  []map[string]any
	nodeByID                    map[string]map[string]any
	edgeKeys                    map[string]struct{}
	dataflowKeys                map[string]struct{}
	edgeOrderKeys               []string
	unresolvedKeys              map[string]struct{}
	unresolvedOrderKeys         []string
	moduleByPath                map[string]string
	classByName                 map[string][]string
	classByFileAndName          map[string]map[string][]string
	methodByClassName           map[string]map[string][]string
	methodByClassID             map[string]map[string][]string
	methodByOwnerID             map[string]map[string][]string
	staticMethodByOwnerID       map[string]map[string][]string
	typeByOwnerID               map[string]map[string][]string
	ownerByFunctionID           map[string]string
	declarationByID             map[string]*declaration
	fileByDeclarationID         map[string]*parsedFile
	declaredMemberByOwner       map[string]map[string]bool
	declaredLocalByFunction     map[string]map[string]bool
	memberValueByOwner          map[string]map[string][]string
	localValueByFunction        map[string]map[string][]string
	parentByOwnerID             map[string][]string
	externalParentByOwnerID     map[string]bool
	scriptOwnerByPath           map[string]string
	scriptOwnerCandidatesByPath map[string][]string
	projectRootByFilePath       map[string]string
	autoloadOwnerByProjectName  map[string]map[string]string
	preloadAliasByFileAndName   map[string]map[string][]string
	staticMethodByModulePath    map[string]map[string][]string
	fileByPath                  map[string]string
}

func node(kind, name, path, qualified, id string, span sourceSpan, content string, attributes ...map[string]any) map[string]any {
	record := map[string]any{"id": id, "kind": kind, "name": name, "path": path, "qualified_name": qualified, "record": "node"}
	if content != "" {
		record["content_id"] = content
	}
	if span != nil {
		record["span"] = span
	}
	if len(attributes) > 0 && len(attributes[0]) > 0 {
		record["attributes"] = attributes[0]
	}
	return record
}

func edge(source, target, relation string, span sourceSpan) map[string]any {
	return edgeWithAttributes(source, target, relation, span, nil)
}

func edgeWithAttributes(source, target, relation string, span sourceSpan, attributes map[string]any) map[string]any {
	record := map[string]any{"record": "edge", "relation": relation, "source": source, "target": target}
	if span != nil {
		record["span"] = span
	}
	if len(attributes) > 0 {
		record["attributes"] = attributes
	}
	return record
}

func unresolved(source, relation, expression, reason string, span sourceSpan) map[string]any {
	record := map[string]any{"expression": expression, "reason": reason, "record": "unresolved", "relation": relation, "source": source}
	if span != nil {
		record["span"] = span
	}
	return record
}

func (f *factSet) addNode(record map[string]any) {
	id := record["id"].(string)
	if _, exists := f.nodeByID[id]; exists {
		return
	}
	f.nodeByID[id] = record
	f.nodes = append(f.nodes, record)
}

func (f *factSet) addEdge(record map[string]any) {
	if f.edgeKeys == nil {
		f.edgeKeys = make(map[string]struct{})
	}
	key := edgeSortKey(record)
	if _, exists := f.edgeKeys[key]; exists {
		return
	}
	f.edgeKeys[key] = struct{}{}
	f.edges = append(f.edges, record)
	f.edgeOrderKeys = append(f.edgeOrderKeys, key)
	f.indexClassMethod(record)
	f.indexStaticFunction(record)
}

func (f *factSet) addDataflowEdge(record map[string]any) {
	key := fmt.Sprintf("%s\x00%s\x00%s", record["source"], record["target"], record["relation"])
	if _, exists := f.dataflowKeys[key]; exists {
		return
	}
	f.dataflowKeys[key] = struct{}{}
	f.addEdge(record)
}

func (f *factSet) indexClassMethod(record map[string]any) {
	if record["relation"] != "defines" {
		return
	}
	owner, ownerOK := f.nodeByID[record["source"].(string)]
	method, methodOK := f.nodeByID[record["target"].(string)]
	if !ownerOK || !methodOK || owner["kind"] != "type" || method["kind"] != "function" {
		return
	}
	className, _ := owner["name"].(string)
	classID := owner["id"].(string)
	if f.methodByClassID == nil {
		f.methodByClassID = make(map[string]map[string][]string)
	}
	if f.methodByClassID[classID] == nil {
		f.methodByClassID[classID] = make(map[string][]string)
	}
	methodName, _ := method["name"].(string)
	f.methodByClassID[classID][methodName] = append(f.methodByClassID[classID][methodName], method["id"].(string))
	if len(f.classByName[className]) == 1 {
		if f.methodByClassName[className] == nil {
			f.methodByClassName[className] = make(map[string][]string)
		}
		f.methodByClassName[className][methodName] = append(f.methodByClassName[className][methodName], method["id"].(string))
	}
}

func (f *factSet) indexDeclaration(pf *parsedFile, decl *declaration) {
	if f.declarationByID == nil {
		f.declarationByID = make(map[string]*declaration)
		f.fileByDeclarationID = make(map[string]*parsedFile)
		f.methodByOwnerID = make(map[string]map[string][]string)
		f.staticMethodByOwnerID = make(map[string]map[string][]string)
		f.typeByOwnerID = make(map[string]map[string][]string)
		f.ownerByFunctionID = make(map[string]string)
		f.declaredMemberByOwner = make(map[string]map[string]bool)
		f.declaredLocalByFunction = make(map[string]map[string]bool)
		f.memberValueByOwner = make(map[string]map[string][]string)
		f.localValueByFunction = make(map[string]map[string][]string)
	}
	f.declarationByID[decl.nodeID] = decl
	f.fileByDeclarationID[decl.nodeID] = pf
	if decl.kind == "variable" || decl.kind == "constant" {
		if decl.ownerFunction != "" {
			if f.declaredLocalByFunction[decl.ownerFunction] == nil {
				f.declaredLocalByFunction[decl.ownerFunction] = make(map[string]bool)
			}
			if f.localValueByFunction[decl.ownerFunction] == nil {
				f.localValueByFunction[decl.ownerFunction] = make(map[string][]string)
			}
			f.declaredLocalByFunction[decl.ownerFunction][decl.name] = true
			f.localValueByFunction[decl.ownerFunction][decl.name] = append(f.localValueByFunction[decl.ownerFunction][decl.name], decl.nodeID)
		} else {
			if f.declaredMemberByOwner[decl.ownerID] == nil {
				f.declaredMemberByOwner[decl.ownerID] = make(map[string]bool)
			}
			if f.memberValueByOwner[decl.ownerID] == nil {
				f.memberValueByOwner[decl.ownerID] = make(map[string][]string)
			}
			f.declaredMemberByOwner[decl.ownerID][decl.name] = true
			f.memberValueByOwner[decl.ownerID][decl.name] = append(f.memberValueByOwner[decl.ownerID][decl.name], decl.nodeID)
		}
	}
	if decl.kind == "type" && decl.keyword != "class_name" {
		if f.typeByOwnerID[decl.ownerID] == nil {
			f.typeByOwnerID[decl.ownerID] = make(map[string][]string)
		}
		f.typeByOwnerID[decl.ownerID][decl.name] = append(f.typeByOwnerID[decl.ownerID][decl.name], decl.nodeID)
		return
	}
	if decl.kind != "function" {
		return
	}
	owner := decl.ownerID
	if f.methodByOwnerID[owner] == nil {
		f.methodByOwnerID[owner] = make(map[string][]string)
	}
	f.methodByOwnerID[owner][decl.name] = append(f.methodByOwnerID[owner][decl.name], decl.nodeID)
	f.ownerByFunctionID[decl.nodeID] = owner
	if decl.static {
		if f.staticMethodByOwnerID[owner] == nil {
			f.staticMethodByOwnerID[owner] = make(map[string][]string)
		}
		f.staticMethodByOwnerID[owner][decl.name] = append(f.staticMethodByOwnerID[owner][decl.name], decl.nodeID)
	}
}

func (f *factSet) indexParent(source, target string) {
	if f.parentByOwnerID == nil {
		f.parentByOwnerID = make(map[string][]string)
	}
	for _, existing := range f.parentByOwnerID[source] {
		if existing == target {
			return
		}
	}
	f.parentByOwnerID[source] = append(f.parentByOwnerID[source], target)
}

func (f *factSet) indexClassDeclaration(path, name, id string) {
	if f.classByFileAndName == nil {
		f.classByFileAndName = make(map[string]map[string][]string)
	}
	path = normalizeSourcePath(path)
	if f.classByFileAndName[path] == nil {
		f.classByFileAndName[path] = make(map[string][]string)
	}
	f.classByFileAndName[path][name] = append(f.classByFileAndName[path][name], id)
}

func (f *factSet) indexPreloadAlias(path, name, targetPath string) {
	if f.preloadAliasByFileAndName == nil {
		f.preloadAliasByFileAndName = make(map[string]map[string][]string)
	}
	path = normalizeSourcePath(path)
	if f.preloadAliasByFileAndName[path] == nil {
		f.preloadAliasByFileAndName[path] = make(map[string][]string)
	}
	f.preloadAliasByFileAndName[path][name] = append(f.preloadAliasByFileAndName[path][name], normalizeSourcePath(targetPath))
}

func (f *factSet) indexStaticFunction(record map[string]any) {
	if record["relation"] != "defines" {
		return
	}
	method, ok := f.nodeByID[record["target"].(string)]
	if !ok || method["kind"] != "function" {
		return
	}
	attrs, _ := method["attributes"].(map[string]any)
	if attrs == nil || attrs["static"] != true {
		return
	}
	span, _ := method["span"].(map[string]any)
	if spanInt(span, "start_column") != 1 {
		return
	}
	path, _ := method["path"].(string)
	if f.staticMethodByModulePath == nil {
		f.staticMethodByModulePath = make(map[string]map[string][]string)
	}
	path = normalizeSourcePath(path)
	if f.staticMethodByModulePath[path] == nil {
		f.staticMethodByModulePath[path] = make(map[string][]string)
	}
	name, _ := method["name"].(string)
	f.staticMethodByModulePath[path][name] = append(f.staticMethodByModulePath[path][name], method["id"].(string))
}

func (f *factSet) addUnresolved(record map[string]any) {
	if f.unresolvedKeys == nil {
		f.unresolvedKeys = make(map[string]struct{})
	}
	key := unresolvedSortKey(record)
	if _, exists := f.unresolvedKeys[key]; exists {
		return
	}
	f.unresolvedKeys[key] = struct{}{}
	f.unresolved = append(f.unresolved, record)
	f.unresolvedOrderKeys = append(f.unresolvedOrderKeys, key)
}

func nodeID(kind, canonical string) string {
	return digest("lexicon:v1\x00" + language + "\x00" + kind + "\x00" + canonical)
}

func digest(value string) string {
	hash := sha256.Sum256([]byte(value))
	return "sha256:" + hex.EncodeToString(hash[:])
}

func contentID(content []byte) string {
	hash := sha256.Sum256(content)
	return "sha256:" + hex.EncodeToString(hash[:])
}

func cloneMap(input map[string]any) map[string]any {
	output := make(map[string]any, len(input))
	for key, value := range input {
		output[key] = value
	}
	return output
}

func spanString(span map[string]any, key string) string {
	value, _ := span[key].(string)
	return value
}

func spanInt(span map[string]any, key string) int {
	value, _ := span[key].(int)
	return value
}

func normalizeSourcePath(path string) string {
	return filepath.ToSlash(filepath.Clean(filepath.FromSlash(path)))
}
