import * as fs from "node:fs";
import * as path from "node:path";
import { createFileContext, extractDeclarations } from "./ast";
import { createTypeScriptProgram, readPathMappings, scanRepository } from "./discovery";
import { emitFacts, writeJsonl } from "./emission";
import { resolveCalls } from "./call-resolution";
import { resolveImports, resolveRelationships } from "./resolution";
import { emitDataflow } from "./dataflow";
import { FactStore, type Fact } from "./model";
import { addDependencyFacts } from "./dependencies";
import { emitSemanticFacts } from "./semantic-facts";

export function buildFacts(repositoryPath: string, changedFiles?: string[], removedFiles?: string[]): Fact[] {
  const root = path.resolve(repositoryPath);
  if (!fs.statSync(root).isDirectory()) throw new Error(`repository is not a directory: ${repositoryPath}`);
  const repository = path.basename(root);
  const facts = new FactStore(repository, root);
  const repositoryId = facts.addNode("repository", repository, ".", repository, repository);
  const { directories, files } = scanRepository(root);
  const directoryIds = new Map<string, string>();
  for (const directory of directories) {
    const name = directory === "." ? repository : path.posix.basename(directory);
    directoryIds.set(directory, facts.addNode("directory", name, directory, directory, directory));
  }
  facts.addEdge(repositoryId, directoryIds.get(".")!, "contains");
  for (const directory of directories.filter((item) => item !== ".")) {
    const parent = path.posix.dirname(directory) || ".";
    facts.addEdge(directoryIds.get(parent) ?? directoryIds.get(".")!, directoryIds.get(directory)!, "contains");
  }
  const { checker, program } = createTypeScriptProgram(root, files);
  const contexts = files.map((file) => {
    const absolutePath = path.join(root, file.split("/").join(path.sep));
    const sourceFile = program.getSourceFile(absolutePath);
    if (!sourceFile) throw new Error(`TypeScript program did not load ${file}`);
    return createFileContext(root, file, directoryIds, facts, sourceFile);
  });
  for (const context of contexts) extractDeclarations(context, facts);
  emitSemanticFacts(contexts, facts);
  resolveImports(facts, readPathMappings(root));
  addDependencyFacts(facts, root);
  resolveCalls(facts, checker);
  resolveRelationships(facts, checker);
  emitDataflow(facts, checker);
  resolveRelationships(facts);
  return emitFacts(facts, changedFiles, removedFiles);
}

export function writeFacts(repositoryPath: string, outputPath: string, changedFiles?: string[], removedFiles?: string[]): void {
  writeJsonl(buildFacts(repositoryPath, changedFiles, removedFiles), outputPath);
}
