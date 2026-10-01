"""Disposable source preparation and per-command timing for incremental Phase 0."""
from __future__ import annotations

import ctypes
import json
import os
from pathlib import Path
import subprocess
import tarfile
import threading
import time

try:
    import psutil
except ImportError:
    psutil = None


def seed_source(repo: Path, source: Path | None, count: int) -> str:
    repo.mkdir(parents=True)
    if source:
        revision = subprocess.check_output(
            ["git", "-C", str(source), "rev-parse", "HEAD"], text=True
        ).strip()
        archive = repo.parent / "source.tar"
        with archive.open("wb") as output:
            subprocess.run(
                ["git", "-C", str(source), "archive", "--format=tar", "HEAD"],
                stdout=output, check=True,
            )
        with tarfile.open(archive) as content:
            content.extractall(repo, filter="data")
        archive.unlink()
        return revision
    pkg = repo / "phase0"
    pkg.mkdir()
    (pkg / "__init__.py").write_text("", encoding="utf-8")
    for i in range(count):
        imports = f"from .module_{i - 1} import value\n" if i else ""
        (pkg / f"module_{i}.py").write_text(
            f"{imports}value = {i}\n", encoding="utf-8"
        )
    return f"synthetic-python-{count}-files"


def windows_peak(process: subprocess.Popen) -> int:
    class Counters(ctypes.Structure):
        fields = [("cb", ctypes.c_ulong), ("page_faults", ctypes.c_ulong)]
        fields += [(name, ctypes.c_size_t) for name in (
            "peak_working_set", "working_set", "quota_peak_paged_pool",
            "quota_paged_pool", "quota_peak_non_paged_pool",
            "quota_non_paged_pool", "pagefile", "peak_pagefile",
        )]
        _fields_ = fields

    counters = Counters()
    counters.cb = ctypes.sizeof(counters)
    psapi = ctypes.WinDLL("psapi")
    success = psapi.GetProcessMemoryInfo(
        ctypes.c_void_p(int(process._handle)),
        ctypes.byref(counters),
        counters.cb,
    )
    return int(counters.peak_working_set) if success else 0


def measure(binary: Path, args: list[str], timeout: int, log: Path) -> dict:
    start = time.monotonic()
    process = subprocess.Popen(
        [str(binary), *args], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, errors="replace", env=dict(os.environ, LEXICON_PERF="1"),
    )
    peak_rss = 0
    stop = threading.Event()

    def sample() -> None:
        nonlocal peak_rss
        while not stop.is_set():
            try:
                if psutil is not None:
                    parent = psutil.Process(process.pid)
                    family = [parent, *parent.children(recursive=True)]
                    rss = sum(p.memory_info().rss for p in family if p.is_running())
                elif os.name == "nt":
                    rss = windows_peak(process)
                else:
                    rss = 0
                peak_rss = max(peak_rss, rss)
            except (OSError, Exception) as error:
                # Transient exits/races are expected during a sampled subprocess.
                if not isinstance(error, (OSError,)) and (
                    psutil is None or not isinstance(error, psutil.Error)
                ):
                    return
            time.sleep(0.1)

    sampler = threading.Thread(target=sample, daemon=True)
    sampler.start()
    timed_out = False
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        if os.name == "nt":
            subprocess.run(
                ["taskkill", "/F", "/T", "/PID", str(process.pid)],
                capture_output=True, check=False,
            )
        else:
            process.kill()
        stdout, stderr = process.communicate()
    finally:
        stop.set()
        sampler.join(timeout=1)
    log.write_text(stderr, encoding="utf-8")

    metrics = []
    for line in stderr.splitlines():
        if not line.startswith("[lexicon-perf] "):
            continue
        fields = {}
        for item in line.removeprefix("[lexicon-perf] ").split():
            if "=" not in item:
                continue
            key, value = item.split("=", 1)
            try:
                fields[key] = float(value) if key == "elapsed_ms" else int(value)
            except ValueError:
                fields[key] = value
        metrics.append(fields)
    return {
        "exit_code": process.returncode,
        "timed_out": timed_out,
        "wall_seconds": round(time.monotonic() - start, 3),
        "peak_sampled_rss_bytes": peak_rss or None,
        "rss_scope": "parent + children" if psutil else "Windows parent only",
        "stdout_tail": stdout[-2000:],
        "stderr_tail": stderr[-1200:],
        "metrics": metrics,
        "log": str(log),
    }
