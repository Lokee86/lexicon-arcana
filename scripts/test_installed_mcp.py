#!/usr/bin/env python3
"""Build-layout to release-bundle to installed-MCP end-to-end smoke test."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import queue
import shutil
import stat
import subprocess
import tempfile
import threading
import time
import zipfile

import workflow
from installed_warning_contract import run_warning_contract


def remove_readonly(function, path, _):
    os.chmod(path, stat.S_IWRITE)
    function(path)


def all_strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for item in value.values():
            yield from all_strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from all_strings(item)


class MCPClient:
    def __init__(self, process: subprocess.Popen[str]):
        self.process = process
        self.responses: queue.Queue[dict] = queue.Queue()
        self.stderr: list[str] = []
        threading.Thread(target=self._read_stdout, daemon=True).start()
        threading.Thread(target=self._read_stderr, daemon=True).start()

    def _read_stdout(self) -> None:
        assert self.process.stdout is not None
        for line in self.process.stdout:
            if line.strip():
                self.responses.put(json.loads(line))

    def _read_stderr(self) -> None:
        assert self.process.stderr is not None
        self.stderr.extend(line.rstrip() for line in self.process.stderr)

    def send(self, message: dict) -> None:
        if self.process.poll() is not None:
            raise RuntimeError(f"MCP exited {self.process.returncode}: {self.stderr}")
        assert self.process.stdin is not None
        self.process.stdin.write(json.dumps(message, separators=(",", ":")) + "\n")
        self.process.stdin.flush()

    def call(self, identifier, method: str, params: dict, timeout: float = 240) -> dict:
        self.send({"jsonrpc": "2.0", "id": identifier, "method": method, "params": params})
        deadline = time.monotonic() + timeout
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError(f"MCP response {identifier} timed out: {self.stderr}")
            response = self.responses.get(timeout=remaining)
            if response.get("id") == identifier:
                return response

    def close(self) -> None:
        if self.process.stdin is not None:
            self.process.stdin.close()
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            self.process.wait(timeout=10)


def tool_content(response: dict) -> dict:
    if "error" in response:
        raise RuntimeError(f"JSON-RPC error: {response['error']}")
    result = response.get("result") or {}
    if result.get("isError"):
        raise RuntimeError(f"tool error: {result}")
    content = result.get("structuredContent")
    if not isinstance(content, dict):
        raise RuntimeError(f"missing structuredContent: {result}")
    return content


def write_fixture(root: Path) -> None:
    root.mkdir(parents=True)
    (root / "go.mod").write_text("module example.com/installedsmoke\n\ngo 1.26\n", encoding="utf-8")
    (root / "target.go").write_text(
        "package installedsmoke\n\n"
        "func TargetThing() string {\n\treturn HelperThing()\n}\n\n"
        "func HelperThing() string {\n\treturn \"ok\"\n}\n",
        encoding="utf-8",
    )
    (root / "unrelated.go").write_text(
        "package installedsmoke\n\nfunc UnrelatedThing() string { return \"other\" }\n",
        encoding="utf-8",
    )


def run_smoke(source: Path, version: str) -> dict:
    temporary = Path(tempfile.mkdtemp(prefix="grimoire-installed-mcp-"))
    try:
        release_root = workflow.package_artifacts(source, temporary / "dist", version)
        bundles = list(release_root.glob("grimoire-bundle-*.zip"))
        if len(bundles) != 1:
            raise RuntimeError(f"expected one combined bundle: {bundles}")
        extracted = temporary / "bundle"
        with zipfile.ZipFile(bundles[0]) as archive:
            archive.extractall(extracted)

        installed = temporary / "installed"
        subprocess.run(
            [
                os.fspath(Path(os.sys.executable)), os.fspath(extracted / "install.py"),
                "--source", os.fspath(extracted),
                "--bin-dir", os.fspath(installed / "bin"),
                "--skills-dir", os.fspath(installed / "skills"),
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        binary = installed / "bin" / workflow.executable_name("grimoire")
        fixture = temporary / "fixture"
        write_fixture(fixture)
        state = fixture / ".warlock" / "tools"

        environment = os.environ.copy()
        for name in (
            "LEXICON_ADAPTERS", "LEXICON_STATE_DIR", "ARCANA_STATE_DIR",
            "GRIMOIRE_STATE_DIR", "GRIMOIRE_VECTOR_ENGINE", "LODESTONE_ROOT",
        ):
            environment.pop(name, None)
        environment["PATH"] = os.fspath(installed / "bin") + os.pathsep + environment.get("PATH", "")
        warning_contract = run_warning_contract(installed, temporary, environment)
        process = subprocess.Popen(
            [
                os.fspath(binary), "mcp", "--root", os.fspath(fixture),
                "--state", os.fspath(state / "grimoire"),
                "--state-mode", "refresh-if-needed",
            ],
            cwd=fixture,
            env=environment,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        client = MCPClient(process)
        try:
            initialized = client.call("init", "initialize", {"protocolVersion": "2025-11-25"}, 30)
            if initialized.get("result", {}).get("serverInfo", {}).get("name") != "grimoire":
                raise RuntimeError(f"unexpected initialize response: {initialized}")
            client.send({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}})
            listed = client.call("list", "tools/list", {}, 30)
            names = [tool.get("name") for tool in listed.get("result", {}).get("tools", [])]
            if names != ["grimoire_discover"]:
                raise RuntimeError(f"unexpected tools: {names}")

            base = {
                "schema": "grimoire.discovery.v1",
                "root": os.fspath(fixture),
                "state": os.fspath(state / "grimoire"),
                "lexicon_state": os.fspath(state / "lexicon"),
                "arcana_state": os.fspath(state / "arcana"),
                "session": "installed-e2e",
                "code_only": True,
                "include_documents": False,
                "limit": 8,
            }
            search_args = base | {"mode": "search", "query": "TargetThing", "state_mode": "force-refresh"}
            search = tool_content(client.call(
                "search", "tools/call",
                {"name": "grimoire_discover", "arguments": search_args},
            ))
            preparation = search.get("preparation") or {}
            statuses = {item.get("name"): item.get("status") for item in preparation.get("actions") or []}
            for action in ("refresh-lexicon", "synchronize-arcana", "prepare-grimoire"):
                if statuses.get(action) != "completed":
                    raise RuntimeError(f"{action} did not complete: {statuses}")
            warnings = list(preparation.get("warnings") or []) + list(search.get("warnings") or [])
            if warnings:
                raise RuntimeError(f"unexpected provider warnings: {warnings}")

            handle = None
            for hit in (search.get("delta") or {}).get("retrieval_hits") or []:
                if hit.get("lane") != "symbol_matches" or hit.get("provider") != "lexicon":
                    continue
                for related in hit.get("related_evidence") or []:
                    if related.get("kind") == "node":
                        handle = related.get("handle")
                        break
                if handle is not None:
                    break
            if not isinstance(handle, str) or not handle.startswith("g2_"):
                raise RuntimeError(f"search did not return a current opaque TargetThing symbol handle: {handle!r}")
            selected = next(
                (item.get("evidence") or {} for item in (search.get("delta") or {}).get("new_nodes") or [] if item.get("handle") == handle),
                {},
            )
            if selected.get("path") != "target.go" or "TargetThing" not in selected.get("label", ""):
                raise RuntimeError(f"symbol handle selected unexpected evidence: {selected}")

            inspect = tool_content(client.call(
                "inspect", "tools/call",
                {"name": "grimoire_discover", "arguments": base | {
                    "mode": "inspect", "handles": [handle], "state_mode": "current-only",
                }},
                60,
            ))
            inspect_delta = inspect.get("delta") or {}
            ranges = inspect_delta.get("new_source_ranges") or []
            if ranges:
                evidence = ranges[0].get("evidence") or {}
                if evidence.get("path") != "target.go" or "TargetThing" not in evidence.get("text", ""):
                    raise RuntimeError(f"opaque handle resolved incorrectly: {evidence}")
            else:
                prior = inspect_delta.get("prior_evidence") or {}
                hits = inspect_delta.get("retrieval_hits") or []
                if prior.get("source_ranges", 0) < 1 or not any(hit.get("provider") == "lexicon" for hit in hits):
                    raise RuntimeError(f"inspect neither returned nor reused TargetThing source evidence: {inspect_delta}")
            evidence = {"path": "target.go"}

            trace = tool_content(client.call(
                "trace", "tools/call",
                {"name": "grimoire_discover", "arguments": base | {
                    "mode": "trace", "anchor": handle, "state_mode": "current-only",
                    "direction": "outgoing", "relations": ["calls"], "depth": 2,
                }},
                60,
            ))
            if "HelperThing" not in "\n".join(all_strings(trace)):
                raise RuntimeError(f"trace did not reach HelperThing: {trace}")

            required = (
                state / "lexicon" / "CURRENT",
                state / "arcana" / "CURRENT",
                state / "grimoire" / "HEAD",
                state / "grimoire" / "refs" / "grimoire" / "state",
            )
            missing = [os.fspath(path) for path in required if not path.is_file()]
            leaks = [
                os.fspath(path) for path in (fixture / ".lexicon", fixture / ".arcana", fixture / ".grimoire")
                if path.exists()
            ]
            if missing or leaks:
                raise RuntimeError(f"managed state failure: missing={missing}, leaks={leaks}")
            return {
                "bundle": bundles[0].name,
                "preparation_actions": statuses,
                "opaque_handle": handle,
                "inspect_path": evidence.get("path"),
                "trace_reached": "HelperThing",
                "provider_warnings": warnings,
                "warning_contract": warning_contract,
                "default_state_leaks": leaks,
                "mcp_stderr": client.stderr,
            }
        finally:
            client.close()
    finally:
        shutil.rmtree(temporary, onerror=remove_readonly)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=workflow.DEFAULT_BUILD)
    parser.add_argument("--version", default="installed-smoke")
    args = parser.parse_args()
    print(json.dumps(run_smoke(args.source.resolve(), args.version), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
