from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

ADAPTER_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ADAPTER_ROOT))

from lexicon_python.adapter import build_facts


class PythonSemanticErrorFlowTest(unittest.TestCase):
    def test_emits_fallback_propagation_and_non_suppressing_continuation(self) -> None:
        with tempfile.TemporaryDirectory() as tempdir:
            repo = Path(tempdir) / "fixture"
            repo.mkdir()
            (repo / "sample.py").write_text(
                "def fallback_chain():\n"
                "    try:\n"
                "        import first\n"
                "    except ModuleNotFoundError:\n"
                "        pass\n"
                "    try:\n"
                "        import second\n"
                "    except ModuleNotFoundError:\n"
                "        pass\n"
                "    value = 1\n"
                "    return value\n\n"
                "def cleanup():\n"
                "    try:\n"
                "        work()\n"
                "    except Exception:\n"
                "        if True:\n"
                "            try:\n"
                "                cleanup_work()\n"
                "            except OSError:\n"
                "                pass\n"
                "        raise\n\n"
                "def nested_fallback():\n"
                "    if True:\n"
                "        try:\n"
                "            work()\n"
                "        except ValueError:\n"
                "            pass\n"
                "    return 2\n\n"
                "def continued():\n"
                "    try:\n"
                "        work()\n"
                "    except Exception:\n"
                "        pass\n"
                "    record_original_error()\n",
                encoding="utf-8",
            )
            records = build_facts(repo)
            nodes = [record for record in records if record["record"] == "node"]
            flows = [node for node in nodes if str(node["name"]).startswith("error-flow:")]
            self.assertEqual(
                sorted(node["name"] for node in flows),
                sorted(
                    [
                        "error-flow:fallback",
                        "error-flow:fallback",
                        "error-flow:enclosing-propagation",
                        "error-flow:fallback",
                        "error-flow:continuation",
                    ]
                ),
            )
            self.assertTrue(all(node.get("span") is not None for node in flows))


if __name__ == "__main__":
    unittest.main()
