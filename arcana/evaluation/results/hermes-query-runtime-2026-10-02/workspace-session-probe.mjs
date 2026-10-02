import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
const [workspaceRepo, executable, snapshot, output] = process.argv.slice(2);
if (!output) throw new Error("usage: node workspace-session-probe.mjs <workspace-mcp-repo> <arcana-exe> <snapshot-dir> <output-json>");
const { ArcanaSessionManager } = await import(pathToFileURL(path.join(workspaceRepo, "shared/arcana_sessions.js")));
let spawned = 0;
const manager = new ArcanaSessionManager({
  commandResolver: async () => executable,
  snapshotResolver: async () => ({ snapshotId: path.basename(snapshot), snapshotPath: snapshot }),
  spawnImpl: (...args) => { spawned++; return spawn(...args); },
});
try {
  const start = performance.now();
  const results = await Promise.all(Array.from({ length: 20 }, (_, id) => manager.query({
    root: workspaceRepo, request: { id, op: "resolve_symbol", name: "run_agent" }, timeoutMs: 30000,
  })));
  if (results.some((response, id) => response.response.id !== id || !response.response.ok)) throw new Error("bad response");
  const report = { spawned, request_count: results.length, elapsed_ms: performance.now() - start, generation: results[0].snapshot_id };
  await manager.close();
  report.remaining_sessions = manager.sessions.size;
  await fs.writeFile(output, JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
} finally { await manager.close(); }
