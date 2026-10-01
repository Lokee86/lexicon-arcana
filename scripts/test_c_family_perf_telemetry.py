"""Stage aggregation and full-log retention for concurrent Clang profiles."""
from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import c_family_phase2_baseline as baseline


class PerfTelemetryTests(unittest.TestCase):
    def test_phase_rank_events_sum_durations_and_take_peak_gauges(self):
        lines = [
            "[lexicon-perf] stage=c-family.clang.tu.parsed elapsed_ms=1.250 "
            "phase=real rank=0 current_rss_bytes=200 peak_rss_bytes=220 "
            "observation_estimated_bytes=20\n",
            "[lexicon-perf] stage=c-family.clang.tu.parsed elapsed_ms=3.250 "
            "phase=synthetic rank=0 current_rss_bytes=150 peak_rss_bytes=180 "
            "observation_estimated_bytes=10\n",
        ]
        result = baseline.parse_perf("".join(lines))["c-family.clang.tu.parsed"]
        self.assertEqual(result["elapsed_ms"], 4.5)
        self.assertEqual(result["peak_rss_bytes"], 220)
        self.assertEqual(result["current_rss_bytes"], 200)
        self.assertEqual(result["observation_estimated_bytes"], 20)
        self.assertNotIn("phase", result)
        self.assertNotIn("rank", result)

    def test_complete_log_spooled_including_diagnostics(self):
        raw = ("bad option from driver\n"
               "[lexicon-perf] stage=c-family.clang.heap_reclaim "
               "elapsed_ms=0.123 phase=real rank=0 attempted=1 trimmed=0\n"
               "[lexicon-perf] stage=c-family.clang.heap_reclaim "
               "elapsed_ms=0.456 phase=real rank=1 attempted=0 trimmed=0\n")
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            log_path = root / "logs" / "sample.log"

            class Process:
                def poll(self): return 0
                def wait(self): return 0

            def write_log(*_, stderr, **__):
                stderr.write(raw)
                return Process()

            with patch.object(baseline.subprocess, "Popen", side_effect=write_log), \
                 patch.object(baseline, "sample_tree_rss", return_value=(0, False)):
                result = baseline.run_once(
                    root / "helper", root, root / "facts", 5.0, log_path=log_path)
            self.assertTrue(result["completed"])
            self.assertEqual(log_path.read_text(encoding="utf-8"), raw)
            metrics = result["performance_stages"]["c-family.clang.heap_reclaim"]
            self.assertAlmostEqual(metrics["elapsed_ms"], 0.579)
            self.assertEqual(metrics["attempted"], 1)
            self.assertNotIn("rank", metrics)


if __name__ == "__main__":
    unittest.main()
