from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from benchmark_provenance import assert_frozen, sha256_file, sha256_tree, verify_build_version
from run_agent_benchmark import relevant_harness_changes


class BenchmarkProvenanceTests(unittest.TestCase):
    def test_file_and_tree_hashes_change_with_content(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first = root / "a.txt"
            second = root / "nested" / "b.txt"
            second.parent.mkdir()
            first.write_text("one", encoding="utf-8")
            second.write_text("two", encoding="utf-8")
            initial_file = sha256_file(first)
            initial_tree = sha256_tree(root)

            first.write_text("changed", encoding="utf-8")

            self.assertNotEqual(sha256_file(first), initial_file)
            self.assertNotEqual(sha256_tree(root), initial_tree)

    def test_frozen_provenance_rejects_any_change(self) -> None:
        expected = {"tools": {"grimoire": {"sha256": "sha256:one"}}}
        assert_frozen(expected, expected)
        with self.assertRaisesRegex(RuntimeError, "provenance changed"):
            assert_frozen(
                {"tools": {"grimoire": {"sha256": "sha256:two"}}},
                expected,
            )

    def test_build_versions_are_commit_qualified(self) -> None:
        version = "benchmark-123456789abc"
        provenance = {
            "tools": {
                "grimoire": {"version": version},
                "lexicon": {"version": f"lexicon version {version}"},
                "arcana": {"version": f"Arcana {version}"},
            }
        }
        verify_build_version(provenance, version)
        provenance["tools"]["arcana"]["version"] = "Arcana dev"
        with self.assertRaisesRegex(RuntimeError, "expected frozen build version"):
            verify_build_version(provenance, version)

    def test_component_only_build_versions_do_not_require_grimoire(self) -> None:
        version = "benchmark-123456789abc"
        verify_build_version(
            {
                "tools": {
                    "lexicon": {"version": f"lexicon version {version}"},
                    "arcana": {"version": f"Arcana {version}"},
                }
            },
            version,
        )

    def test_generated_outputs_do_not_dirty_harness(self) -> None:
        changes = [
            " M evaluation/results/run/summary.json",
            "?? build/bin/grimoire.exe",
            " M internal/agentruntime/runtime.go",
        ]
        self.assertEqual(
            relevant_harness_changes(changes),
            [" M internal/agentruntime/runtime.go"],
        )


if __name__ == "__main__":
    unittest.main()
