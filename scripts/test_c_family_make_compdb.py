"""Regression tests for Make dry-run compilation-database capture."""

from __future__ import annotations

import json
import shlex
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import c_family_make_compdb as compdb


class MakeCompilationDatabaseTests(unittest.TestCase):
    def test_continued_cbm_recipe_preserves_each_source_and_build_flags(self):
        for newline in ("\n", "\r\n"):
            with self.subTest(newline=repr(newline)):
                repository = Path.cwd()
                output = (chr(92) + newline).join(
                    [
                        "clang -std=c11 -D_GNU_SOURCE -Isrc -Iinternal/cbm "
                        "-o build/c/codebase-memory-mcp ",
                        " src/main.c src/store/store.c ",
                        " build/c/prod_lsp_all.o -lm -lpthread",
                    ]
                )
                records = compdb.compile_output_records(output, repository)
                self.assertEqual(
                    [record["file"] for record in records],
                    [
                        str(repository / "src/main.c"),
                        str(repository / "src/store/store.c"),
                    ],
                )
                for record, source in zip(
                    records, ("src/main.c", "src/store/store.c")
                ):
                    self.assertEqual(
                        shlex.split(record["command"]),
                        [
                            "clang", "-std=c11", "-D_GNU_SOURCE", "-Isrc",
                            "-Iinternal/cbm", "-fsyntax-only", source,
                        ],
                    )

    def test_continuation_inside_double_quotes_does_not_insert_whitespace(self):
        repository = Path.cwd()
        output = (
            'clang -DNAME="code'
            + chr(92) + "\n"
            + 'base" -c src/main.c'
        )
        records = compdb.compile_output_records(output, repository)
        self.assertEqual(
            shlex.split(records[0]["command"]),
            ["clang", "-DNAME=codebase", "-c", "src/main.c"],
        )

    def test_single_line_commands_and_git_quiet_rules_remain_supported(self):
        repository = Path.cwd()
        command = "clang -std=c11 -c -o main.o src/main.c"
        records = compdb.compile_output_records(
            command + "\necho ' CC main.o';" + command, repository
        )
        self.assertEqual(len(records), 1)
        self.assertEqual(shlex.split(records[0]["command"]), shlex.split(command))

    def test_duplicate_commands_are_removed_but_distinct_contexts_remain(self):
        repository = Path.cwd()
        output = "\n".join(
            [
                "clang -DPRODUCTION=1 -c src/main.c",
                "clang -DTEST=1 -c src/main.c",
                "clang -DPRODUCTION=1 -c src/main.c",
            ]
        )
        records = compdb.compile_output_records(output, repository)
        self.assertEqual(len(records), 2)
        self.assertIn("-DPRODUCTION=1", records[0]["command"])
        self.assertIn("-DTEST=1", records[1]["command"])

    def test_cli_consumes_complete_make_recipe(self):
        output = (
            "clang -Isrc -o cbm " + chr(92) + "\n"
            + " src/main.c src/store/store.c -lm\n"
        )
        completed = subprocess.CompletedProcess(
            args=["make"], returncode=0, stdout=output, stderr=""
        )
        with tempfile.TemporaryDirectory() as raw:
            repository = Path(raw).resolve()
            destination = repository / "compile_commands.json"
            with (
                patch("sys.argv", [
                    "c_family_make_compdb.py", "--repository", str(repository),
                    "--target", "cbm",
                ]),
                patch.object(compdb.subprocess, "run", return_value=completed),
                patch("builtins.print"),
            ):
                self.assertEqual(compdb.main(), 0)
            records = json.loads(destination.read_text(encoding="utf-8"))
            self.assertEqual(len(records), 2)
            self.assertEqual(records[0]["file"], str(repository / "src/main.c"))
            self.assertEqual(
                records[1]["file"], str(repository / "src/store/store.c")
            )


if __name__ == "__main__":
    unittest.main()
