from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ADAPTER_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = ADAPTER_ROOT.parents[1]


class PythonAdapterTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.repo = Path(self.tempdir.name) / "fixture-repository"
        (self.repo / "pkg").mkdir(parents=True)
        (self.repo / "vendor").mkdir()
        (self.repo / "build").mkdir()
        (self.repo / "__pycache__").mkdir()
        (self.repo / ".next").mkdir()
        self.warlock_state_directories = (
            ".ddocs",
            ".lexicon",
            ".arcana",
            ".grimoire",
            ".pitlord",
            ".cantrip",
            ".homunculus",
            ".incubus",
            ".ritual",
            ".warlock",
        )
        for directory in self.warlock_state_directories:
            (self.repo / directory).mkdir()
        self._write("pkg/__init__.py", "from .base import Base\n")
        self._write(
            "pkg/base.py",
            "class Base:\n"
            "    def run(self):\n"
            "        return 1\n",
        )
        self._write(
            "pkg/known.py",
            "class Worker:\n"
            "    def run(self):\n"
            "        return 1\n",
        )
        self._write(
            "pkg/other.py",
            "class Alternate:\n"
            "    def run(self):\n"
            "        return 1\n",
        )
        self._write(
            "pkg/child.py",
            "from .base import Base\n"
            "import pkg.base as base\n"
            "class Child(Base):\n"
            "    def run(self):\n"
            "        return base.Base()\n",
        )
        self._write(
            "main.py",
            "from pkg.child import Child\n"
            "class Root(Child):\n"
            "    pass\n\n"
            "def factory():\n"
            "    return Root\n\n"
            "def use():\n"
            "    return factory()\n",
        )
        self._write("dynamic.py", "import importlib\nmodule = importlib.import_module(name)\n")
        self._write("vendor/ignored.py", "class Ignored: pass\n")
        self._write("build/ignored.py", "def ignored(): pass\n")
        self._write("__pycache__/ignored.py", "def ignored(): pass\n")
        self._write(".next/generated.py", "def ignored(): pass\n")
        for directory in self.warlock_state_directories:
            self._write(f"{directory}/ignored.py", "def ignored(): pass\n")
        self._write("tools/data_sync/config.py", "class NestedConfig:\n    pass\n")
        self._write(
            "nested_consumer.py",
            "from data_sync.config import NestedConfig\n\n"
            "def build_nested():\n"
            "    return NestedConfig()\n",
        )

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def _write(self, relative: str, content: str) -> None:
        path = self.repo / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def _run(self, output: Path) -> list[dict[str, object]]:
        environment = os.environ.copy()
        environment["PYTHONPATH"] = str(ADAPTER_ROOT)
        result = subprocess.run(
            [sys.executable, "-m", "lexicon_python", "--repo", str(self.repo), "--output", str(output)],
            cwd=REPO_ROOT,
            env=environment,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.stderr, "")
        return [json.loads(line) for line in output.read_text(encoding="utf-8").splitlines()]

    def test_declarations_imports_inheritance_and_exclusions(self) -> None:
        records = self._run(self.repo / "facts.jsonl")
        nodes = [record for record in records if record["record"] == "node"]
        edges = [record for record in records if record["record"] == "edge"]
        unresolved = [record for record in records if record["record"] == "unresolved"]

        kinds = {record["kind"] for record in nodes}
        self.assertTrue({"repository", "directory", "file", "module", "type", "function", "method", "import"} <= kinds)
        paths = {record["path"] for record in nodes}
        self.assertNotIn("vendor/ignored.py", paths)
        self.assertNotIn("build/ignored.py", paths)
        self.assertNotIn("__pycache__/ignored.py", paths)
        self.assertNotIn(".next/generated.py", paths)
        for directory in self.warlock_state_directories:
            self.assertNotIn(f"{directory}/ignored.py", paths)

        relations = {record["relation"] for record in edges}
        self.assertTrue({"contains", "defines", "imports", "extends"} <= relations)
        self.assertTrue(any(record["relation"] == "extends" for record in edges))
        self.assertTrue(any(record["relation"] == "imports" for record in edges))
        self.assertTrue(any(record["relation"] == "calls" for record in unresolved))
        self.assertTrue(any(record["reason"] == "dynamic-target" for record in unresolved))

    def test_nested_package_prefix_imports_resolve(self) -> None:
        records = self._run(self.repo / "facts.jsonl")
        nodes = [record for record in records if record["record"] == "node"]
        edges = [record for record in records if record["record"] == "edge"]
        consumer_module = next(
            record["id"]
            for record in nodes
            if record["kind"] == "module" and record["path"] == "nested_consumer.py"
        )
        nested_type = next(
            record["id"]
            for record in nodes
            if record["kind"] == "type" and record["qualified_name"] == "tools.data_sync.config.NestedConfig"
        )
        builder = next(
            record["id"]
            for record in nodes
            if record["kind"] == "function" and record["qualified_name"] == "nested_consumer.build_nested"
        )

        self.assertTrue(
            any(
                record["relation"] == "imports"
                and record["source"] == consumer_module
                and record["target"] == nested_type
                for record in edges
            )
        )
        self.assertTrue(
            any(
                record["relation"] == "calls"
                and record["source"] == builder
                and record["target"] == nested_type
                for record in edges
            )
        )

    def test_dataflow_reads_writes_compound_members_shadowing_and_lambda_parameters(self) -> None:
        self._write(
            "dataflow.py",
            "outer = 3\n"
            "class Box:\n"
            "    field = 0\n"
            "    def run(self, value):\n"
            "        local = value\n"
            "        local += outer\n"
            "        local += 1\n"
            "        self.field = local\n"
            "        def inner(value):\n"
            "            shadow = value\n"
            "            return shadow\n"
            "        callback = lambda item: item\n"
            "        return local + value + inner(value) + callback(value)\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        nodes = [record for record in records if record["record"] == "node"]
        edges = [record for record in records if record["record"] == "edge"]
        data_edges = [record for record in edges if record["relation"] in {"reads", "writes"}]
        node_by_id = {record["id"]: record for record in nodes}
        run = next(record for record in nodes if record.get("qualified_name") == "dataflow.Box.run")
        inner = next(record for record in nodes if record.get("qualified_name") == "dataflow.Box.run.inner")
        run_targets = {record["target"] for record in data_edges if record["source"] == run["id"]}
        inner_targets = {record["target"] for record in data_edges if record["source"] == inner["id"]}
        assert any(node_by_id[target]["name"] == "outer" for target in run_targets)
        assert any(node_by_id[target]["name"] == "field" for target in run_targets)
        assert any(node_by_id[target]["name"] == "value" for target in run_targets)
        assert any(node_by_id[target]["name"] == "value" for target in inner_targets)
        assert run_targets.isdisjoint(inner_targets)
        assert any(record["relation"] == "reads" and node_by_id[record["target"]]["name"] == "local" for record in data_edges if record["source"] == run["id"])
        assert any(record["relation"] == "writes" and node_by_id[record["target"]]["name"] == "local" for record in data_edges if record["source"] == run["id"])
        assert any(record["kind"] == "parameter" and record["name"] == "item" for record in nodes)
        assert all(record["target"] in node_by_id for record in data_edges)
        assert not any(record["record"] == "unresolved" and record["relation"] in {"reads", "writes"} for record in records)

    def test_local_constructor_calls_are_precise_and_conservative(self) -> None:
        self._write("left/config.py", "class Worker:\n    def run(self): pass\n")
        self._write("right/config.py", "class Worker:\n    def run(self): pass\n")
        self._write(
            "local_precision.py",
            "from pkg.known import Worker\n"
            "from pkg.other import Alternate\n"
            "from config import Worker as AmbiguousWorker\n\n"
            "def precise():\n"
            "    worker = Worker()\n"
            "    return worker.run()\n\n"
            "def same_type_branch(flag):\n"
            "    worker = Worker()\n"
            "    if flag:\n"
            "        worker = Worker()\n"
            "    return worker.run()\n\n"
            "def unconditional_reassignment():\n"
            "    worker = Worker()\n"
            "    worker = Alternate()\n"
            "    return worker.run()\n\n"
            "def branch_union(flag):\n"
            "    worker = Worker()\n"
            "    if flag:\n"
            "        worker = Alternate()\n"
            "    return worker.run()\n\n"
            "def attribute_based():\n"
            "    holder.worker = Worker()\n"
            "    return holder.worker.run()\n\n"
            "def unknown_method():\n"
            "    worker = Worker()\n"
            "    return worker.missing()\n\n"
            "def ambiguous_class():\n"
            "    worker = AmbiguousWorker()\n"
            "    return worker.run()\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        nodes = [record for record in records if record["record"] == "node"]
        edges = [record for record in records if record["record"] == "edge"]
        unresolved = [record for record in records if record["record"] == "unresolved"]
        worker_run = next(record for record in nodes if record.get("qualified_name") == "pkg.known.Worker.run")
        alternate_run = next(record for record in nodes if record.get("qualified_name") == "pkg.other.Alternate.run")
        function_ids = {
            record["qualified_name"].rsplit(".", 1)[-1]: record["id"]
            for record in nodes
            if record["kind"] == "function" and record["path"] == "local_precision.py"
        }

        definite_targets = {
            (record["source"], record["target"])
            for record in edges
            if record["relation"] == "calls"
        }
        possible_targets = {
            (record["source"], record["target"])
            for record in edges
            if record["relation"] == "possible-calls"
        }
        self.assertIn((function_ids["precise"], worker_run["id"]), definite_targets)
        self.assertIn((function_ids["same_type_branch"], worker_run["id"]), definite_targets)
        self.assertIn((function_ids["unconditional_reassignment"], alternate_run["id"]), definite_targets)
        self.assertEqual(
            {
                target
                for source, target in possible_targets
                if source == function_ids["branch_union"]
            },
            {worker_run["id"], alternate_run["id"]},
        )
        for function_name in ("attribute_based", "unknown_method", "ambiguous_class"):
            self.assertFalse(
                any(
                    source == function_ids[function_name]
                    and target in {worker_run["id"], alternate_run["id"]}
                    for source, target in definite_targets | possible_targets
                )
            )
        self.assertTrue(any(record["expression"] == "holder.worker.run()" for record in unresolved))
        self.assertTrue(any(record["expression"] == "worker.missing()" and record["reason"] == "missing-target" for record in unresolved))

    def test_flow_aware_receivers_factories_fields_and_loops(self) -> None:
        self._write(
            "pkg/flow.py",
            "from .known import Worker\n\n"
            "class Holder:\n"
            "    worker: Worker\n"
            "    def use(self):\n"
            "        return self.worker.run()\n\n"
            "class Builder:\n"
            "    @classmethod\n"
            "    def create(cls):\n"
            "        return cls()\n"
            "    def run(self):\n"
            "        return 1\n\n"
            "def make_worker() -> Worker:\n"
            "    return Worker()\n\n"
            "def consume(worker: Worker):\n"
            "    return worker.run()\n\n"
            "def consume_many(workers: list[Worker]):\n"
            "    for worker in workers:\n"
            "        worker.run()\n"
            "    return [worker.run() for worker in workers]\n\n"
            "def use():\n"
            "    make_worker().run()\n"
            "    consume(Worker())\n"
            "    consume_many([Worker()])\n"
            "    Builder.create().run()\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        nodes = {
            record["qualified_name"]: record["id"]
            for record in records
            if record["record"] == "node" and "qualified_name" in record
        }
        calls = {
            (record["source"], record["target"])
            for record in records
            if record["record"] == "edge" and record["relation"] == "calls"
        }
        worker_run = nodes["pkg.known.Worker.run"]
        builder = nodes["pkg.flow.Builder"]
        builder_run = nodes["pkg.flow.Builder.run"]

        for source_qname in (
            "pkg.flow.Holder.use",
            "pkg.flow.consume",
            "pkg.flow.consume_many",
            "pkg.flow.use",
        ):
            self.assertIn((nodes[source_qname], worker_run), calls)
        self.assertIn((nodes["pkg.flow.Builder.create"], builder), calls)
        self.assertIn((nodes["pkg.flow.use"], builder_run), calls)

    def test_annotated_base_types_include_overrides_as_possible_calls(self) -> None:
        self._write(
            "dispatch.py",
            "class Base:\n"
            "    def run(self):\n"
            "        return 1\n\n"
            "class Child(Base):\n"
            "    def run(self):\n"
            "        return 2\n\n"
            "def polymorphic(value: Base):\n"
            "    return value.run()\n\n"
            "def concrete():\n"
            "    return Child().run()\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        nodes = {
            record["qualified_name"]: record["id"]
            for record in records
            if record["record"] == "node" and "qualified_name" in record
        }
        possible = {
            record["target"]
            for record in records
            if record["record"] == "edge"
            and record["relation"] == "possible-calls"
            and record["source"] == nodes["dispatch.polymorphic"]
        }
        self.assertEqual(possible, {nodes["dispatch.Base.run"], nodes["dispatch.Child.run"]})
        self.assertTrue(
            any(
                record["record"] == "edge"
                and record["relation"] == "calls"
                and record["source"] == nodes["dispatch.concrete"]
                and record["target"] == nodes["dispatch.Child.run"]
                for record in records
            )
        )

    def test_multiple_inheritance_uses_c3_method_order(self) -> None:
        self._write(
            "mro.py",
            "class Left:\n"
            "    def run(self):\n"
            "        return 1\n\n"
            "class Right:\n"
            "    def run(self):\n"
            "        return 2\n\n"
            "class Child(Left, Right):\n"
            "    pass\n\n"
            "class Override(Left, Right):\n"
            "    def run(self):\n"
            "        return 3\n\n"
            "def inherited():\n"
            "    return Child().run()\n\n"
            "def overridden():\n"
            "    return Override().run()\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        nodes = {
            record["qualified_name"]: record["id"]
            for record in records
            if record["record"] == "node" and "qualified_name" in record
        }
        calls = {
            (record["source"], record["target"])
            for record in records
            if record["record"] == "edge" and record["relation"] == "calls"
        }
        self.assertIn((nodes["mro.inherited"], nodes["mro.Left.run"]), calls)
        self.assertNotIn((nodes["mro.inherited"], nodes["mro.Right.run"]), calls)
        self.assertIn((nodes["mro.overridden"], nodes["mro.Override.run"]), calls)

    def test_higher_order_lambdas_containers_and_parametrize(self) -> None:
        self._write(
            "callbacks.py",
            "import pytest\n\n"
            "def alpha():\n"
            "    return 1\n\n"
            "def beta():\n"
            "    return 2\n\n"
            "CALLBACKS = {\"a\": alpha, \"b\": beta}\n\n"
            "def invoke(callback):\n"
            "    return callback()\n\n"
            "def choose(name):\n"
            "    return CALLBACKS[name]()\n\n"
            "def choose_get(name):\n"
            "    return CALLBACKS.get(name)()\n\n"
            "def use():\n"
            "    invoke(alpha)\n"
            "    invoke(beta)\n"
            "    local = lambda: alpha()\n"
            "    return local()\n\n"
            "@pytest.mark.parametrize(\"callback\", [alpha, beta])\n"
            "def parametrized(callback):\n"
            "    return callback()\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        nodes = {
            record["qualified_name"]: record
            for record in records
            if record["record"] == "node" and "qualified_name" in record
        }
        edges = [record for record in records if record["record"] == "edge"]
        alpha = nodes["callbacks.alpha"]["id"]
        beta = nodes["callbacks.beta"]["id"]
        expected = {alpha, beta}

        for source_qname in ("callbacks.invoke", "callbacks.choose", "callbacks.choose_get", "callbacks.parametrize"):
            if source_qname == "callbacks.parametrize":
                source_qname = "callbacks.parametrized"
            targets = {
                record["target"]
                for record in edges
                if record["relation"] == "possible-calls"
                and record["source"] == nodes[source_qname]["id"]
            }
            self.assertEqual(targets, expected)

        lambda_node = next(
            record
            for record in nodes.values()
            if record.get("attributes", {}).get("lambda") is True
        )
        definite = {
            (record["source"], record["target"])
            for record in edges
            if record["relation"] == "calls"
        }
        self.assertIn((nodes["callbacks.use"]["id"], lambda_node["id"]), definite)
        self.assertIn((lambda_node["id"], alpha), definite)

    def test_bound_methods_callable_instances_getattr_and_partial(self) -> None:
        self._write(
            "callable_forms.py",
            "import functools\n\n"
            "class CallableThing:\n"
            "    def __call__(self):\n"
            "        return 1\n"
            "    def run(self):\n"
            "        return 2\n\n"
            "def use():\n"
            "    item = CallableThing()\n"
            "    item()\n"
            "    bound = item.run\n"
            "    bound()\n"
            "    getattr(item, \"run\")()\n"
            "    wrapped = functools.partial(bound)\n"
            "    return wrapped()\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        nodes = {
            record["qualified_name"]: record["id"]
            for record in records
            if record["record"] == "node" and "qualified_name" in record
        }
        targets = [
            record["target"]
            for record in records
            if record["record"] == "edge"
            and record["relation"] == "calls"
            and record["source"] == nodes["callable_forms.use"]
        ]
        self.assertIn(nodes["callable_forms.CallableThing.__call__"], targets)
        self.assertGreaterEqual(targets.count(nodes["callable_forms.CallableThing.run"]), 3)

    def test_nearest_module_and_package_reexports_resolve(self) -> None:
        self._write("suite/main.py", "def run():\n    return 1\n")
        self._write("other/main.py", "def run():\n    return 2\n")
        self._write("values.py", "from pkg.known import Worker\nWORKER = Worker()\n")
        self._write(
            "imported_value.py",
            "from values import WORKER\n\ndef use_value():\n    return WORKER.run()\n",
        )
        self._write(
            "suite/tests/test_use.py",
            "from main import run\n"
            "from pkg import Base\n\n"
            "def use():\n"
            "    run()\n"
            "    Base().run()\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        nodes = {
            record["qualified_name"]: record["id"]
            for record in records
            if record["record"] == "node" and "qualified_name" in record
        }
        calls = {
            (record["source"], record["target"])
            for record in records
            if record["record"] == "edge" and record["relation"] == "calls"
        }
        source = nodes["suite.tests.test_use.use"]
        self.assertIn((source, nodes["suite.main.run"]), calls)
        self.assertNotIn((source, nodes["other.main.run"]), calls)
        self.assertIn((source, nodes["pkg.base.Base"]), calls)
        self.assertIn((source, nodes["pkg.base.Base.run"]), calls)
        self.assertIn(
            (nodes["imported_value.use_value"], nodes["pkg.known.Worker.run"]),
            calls,
        )

    def test_nested_lexical_callables_resolve(self) -> None:
        self._write(
            "nested.py",
            "def outer():\n"
            "    class Visitor:\n"
            "        def run(self):\n"
            "            return 1\n"
            "    def helper():\n"
            "        return Visitor().run()\n"
            "    return helper()\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        nodes = {
            record["qualified_name"]: record["id"]
            for record in records
            if record["record"] == "node" and "qualified_name" in record
        }
        calls = {
            (record["source"], record["target"])
            for record in records
            if record["record"] == "edge" and record["relation"] == "calls"
        }
        self.assertIn(
            (nodes["nested.outer.helper"], nodes["nested.outer.Visitor"]),
            calls,
        )
        self.assertIn(
            (nodes["nested.outer.helper"], nodes["nested.outer.Visitor.run"]),
            calls,
        )
        self.assertIn(
            (nodes["nested.outer"], nodes["nested.outer.helper"]),
            calls,
        )

    def test_nested_functions_capture_outer_callable_flow(self) -> None:
        self._write(
            "closures.py",
            "def alpha():\n"
            "    return 1\n\n"
            "def beta():\n"
            "    return 2\n\n"
            "def wrap(callback):\n"
            "    def inner():\n"
            "        return callback()\n"
            "    return inner\n\n"
            "def use():\n"
            "    wrap(alpha)()\n"
            "    return wrap(beta)()\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        nodes = {
            record["qualified_name"]: record["id"]
            for record in records
            if record["record"] == "node" and "qualified_name" in record
        }
        possible = {
            record["target"]
            for record in records
            if record["record"] == "edge"
            and record["relation"] == "possible-calls"
            and record["source"] == nodes["closures.wrap.inner"]
        }
        self.assertEqual(possible, {nodes["closures.alpha"], nodes["closures.beta"]})
        self.assertTrue(
            any(
                record["record"] == "edge"
                and record["relation"] == "calls"
                and record["source"] == nodes["closures.use"]
                and record["target"] == nodes["closures.wrap.inner"]
                for record in records
            )
        )

    def test_local_decorators_rebind_calls_to_wrappers(self) -> None:
        self._write(
            "decorators.py",
            "def decorate(func):\n"
            "    def wrapper():\n"
            "        return func()\n"
            "    return wrapper\n\n"
            "@decorate\n"
            "def target():\n"
            "    return 1\n\n"
            "def use():\n"
            "    return target()\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        nodes = {
            record["qualified_name"]: record["id"]
            for record in records
            if record["record"] == "node" and "qualified_name" in record
        }
        calls = {
            (record["source"], record["target"])
            for record in records
            if record["record"] == "edge" and record["relation"] == "calls"
        }
        wrapper = nodes["decorators.decorate.wrapper"]
        target = nodes["decorators.target"]
        use = nodes["decorators.use"]
        self.assertIn((use, wrapper), calls)
        self.assertNotIn((use, target), calls)
        self.assertIn((wrapper, target), calls)

    def test_runtime_builtin_namespace_is_classified(self) -> None:
        self._write(
            "builtins_usage.py",
            "def use(values):\n"
            "    sorted(values)\n"
            "    frozenset(values)\n"
            "    KeyError('missing')\n"
            "    Exception('failed')\n"
            "    SystemExit(1)\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        unresolved = [
            record
            for record in records
            if record["record"] == "unresolved"
            and record["relation"] == "calls"
            and record.get("span", {}).get("path") == "builtins_usage.py"
        ]
        self.assertEqual(len(unresolved), 5)
        self.assertEqual({record["reason"] for record in unresolved}, {"builtin-target"})

    def test_imported_module_callbacks_in_mapping_resolve(self) -> None:
        self._write(
            "pkg/generators.py",
            "def first(value):\n"
            "    return value\n\n"
            "def second(value):\n"
            "    return value\n",
        )
        self._write(
            "callback_registry.py",
            "from typing import Callable\n"
            "from pkg import generators\n\n"
            "Generator = Callable[[str], str]\n"
            "GENERATORS: dict[str, Generator] = {\n"
            "    'first': generators.first,\n"
            "    'second': generators.second,\n"
            "}\n\n"
            "def use(name: str, value: str):\n"
            "    generator = GENERATORS.get(name)\n"
            "    if generator is None:\n"
            "        return value\n"
            "    return generator(value)\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        nodes = {
            record["qualified_name"]: record["id"]
            for record in records
            if record["record"] == "node" and "qualified_name" in record
        }
        possible = {
            record["target"]
            for record in records
            if record["record"] == "edge"
            and record["relation"] == "possible-calls"
            and record["source"] == nodes["callback_registry.use"]
        }
        self.assertEqual(
            possible,
            {nodes["pkg.generators.first"], nodes["pkg.generators.second"]},
        )

    def test_runtime_receiver_provenance_classifies_common_python_calls(self) -> None:
        self._write(
            "runtime_receivers.py",
            "from collections.abc import Callable\n"
            "from pathlib import Path\n"
            "import argparse\n"
            "import unittest\n\n"
            "class Failure(Exception):\n"
            "    def __init__(self):\n"
            "        super().__init__('failed')\n\n"
            "class Case(unittest.TestCase):\n"
            "    def check(self):\n"
            "        self.assertEqual(1, 1)\n\n"
            "class Runner:\n"
            "    def __init__(self, clock: Callable[[], float]):\n"
            "        self.clock = clock\n"
            "    def run(self):\n"
            "        return self.clock()\n\n"
            "def use(values: list[str], mapping: dict[str, list[str]], root: Path):\n"
            "    lines = []\n"
            "    lines.append('value')\n"
            "    values.extend(lines)\n"
            "    ' value '.strip().lower()\n"
            "    mapping.setdefault('key', []).append('value')\n"
            "    (root / 'file.txt').read_text(encoding='utf-8').strip()\n"
            "    parser = argparse.ArgumentParser()\n"
            "    return parser.parse_args([])\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        unresolved = [
            record
            for record in records
            if record["record"] == "unresolved"
            and record["relation"] == "calls"
            and record.get("span", {}).get("path") == "runtime_receivers.py"
        ]
        by_expression = {record["expression"]: record["reason"] for record in unresolved}

        for expression in (
            "super().__init__('failed')",
            "lines.append('value')",
            "values.extend(lines)",
            "' value '.strip()",
            "' value '.strip().lower()",
            "mapping.setdefault('key', [])",
            "mapping.setdefault('key', []).append('value')",
        ):
            self.assertEqual(by_expression[expression], "builtin-target", expression)
        for expression in (
            "self.assertEqual(1, 1)",
            "(root / 'file.txt').read_text(encoding='utf-8')",
            "(root / 'file.txt').read_text(encoding='utf-8').strip()",
            "argparse.ArgumentParser()",
            "parser.parse_args([])",
        ):
            self.assertEqual(by_expression[expression], "external-target")
        self.assertEqual(by_expression["self.clock()"], "dynamic-target")
        self.assertFalse(
            {"ambiguous-target", "missing-target", "unsupported-form"}
            & {record["reason"] for record in unresolved}
        )

    def test_contract_dispatch_override_inheritance_narrowing_and_dynamic_fallback(self) -> None:
        self._write(
            "pkg/contracts.py",
            "from typing import Protocol\n"
            "class Runner(Protocol):\n"
            "    def run(self): ...\n"
            "class One(Runner):\n"
            "    def run(self): return 1\n"
            "class Two(Runner):\n"
            "    def run(self): return 2\n"
            "class Base:\n"
            "    def inherited(self): return 1\n"
            "class Child(Base):\n"
            "    pass\n"
            "def invoke(runner: Runner):\n"
            "    return runner.run()\n"
            "def exact():\n"
            "    return One().run()\n"
            "def inherited():\n"
            "    return Child().inherited()\n"
            "def dynamic(value, name):\n"
            "    return getattr(value, name)()\n",
        )
        records = self._run(self.repo / "facts.jsonl")
        nodes = [record for record in records if record["record"] == "node"]
        edges = [record for record in records if record["record"] == "edge"]
        unresolved = [record for record in records if record["record"] == "unresolved"]
        node = lambda qualified: next(record for record in nodes if record.get("qualified_name") == qualified)
        invoke = node("pkg.contracts.invoke")
        exact = node("pkg.contracts.exact")
        inherited = node("pkg.contracts.inherited")
        one_run = node("pkg.contracts.One.run")
        two_run = node("pkg.contracts.Two.run")
        base_run = node("pkg.contracts.Base.inherited")
        runner = node("pkg.contracts.Runner")
        runner_method = node("pkg.contracts.Runner.run")
        one = node("pkg.contracts.One")
        two = node("pkg.contracts.Two")
        child = node("pkg.contracts.Child")
        self.assertEqual(runner["kind"], "interface")
        self.assertTrue(any(edge["source"] == one["id"] and edge["target"] == runner["id"] and edge["relation"] == "implements" for edge in edges))
        self.assertTrue(any(edge["source"] == two["id"] and edge["target"] == runner["id"] and edge["relation"] == "implements" for edge in edges))
        self.assertTrue(any(edge["source"] == invoke["id"] and edge["target"] == one_run["id"] and edge["relation"] == "possible-calls" for edge in edges))
        self.assertTrue(any(edge["source"] == invoke["id"] and edge["target"] == two_run["id"] and edge["relation"] == "possible-calls" for edge in edges))
        self.assertTrue(any(edge["source"] == exact["id"] and edge["target"] == one_run["id"] and edge["relation"] == "calls" for edge in edges))
        self.assertTrue(any(edge["source"] == inherited["id"] and edge["target"] == base_run["id"] and edge["relation"] == "calls" for edge in edges))
        self.assertTrue(any(edge["source"] == one_run["id"] and edge["target"] == runner_method["id"] and edge["relation"] == "overrides" for edge in edges))
        self.assertTrue(any(edge["source"] == two_run["id"] and edge["target"] == runner_method["id"] and edge["relation"] == "overrides" for edge in edges))
        self.assertTrue(any(edge["source"] == child["id"] and edge["target"] == node("pkg.contracts.Base")["id"] and edge["relation"] == "extends" for edge in edges))
        self.assertTrue(any(record["source"] == node("pkg.contracts.dynamic")["id"] and record["reason"] == "dynamic-target" for record in unresolved))

        output = self.repo / "facts.jsonl"
        records = self._run(output)
        header = records[0]
        self.assertEqual(header, {
            "adapter_version": "0.5.0",
            "language": "python",
            "record": "lexicon",
            "repository": "fixture-repository",
            "schema_version": 1,
        })
        file_data = (self.repo / "main.py").read_bytes()
        expected_content = "sha256:" + hashlib.sha256(file_data).hexdigest()
        expected_node = "sha256:" + hashlib.sha256(
            b"lexicon:v1\0python\0file\0main.py"
        ).hexdigest()
        main_node = next(record for record in records if record.get("path") == "main.py" and record.get("kind") == "file")
        self.assertEqual(main_node["id"], expected_node)
        self.assertEqual(main_node["content_id"], expected_content)
        self.assertTrue(all(record["record"] == "lexicon" or "D:\\" not in json.dumps(record) for record in records))

    def test_deterministic_ordering_and_repeat_runs(self) -> None:
        first = self.repo / "first.jsonl"
        second = self.repo / "second.jsonl"
        first_records = self._run(first)
        second_records = self._run(second)
        self.assertEqual(first.read_bytes(), second.read_bytes())
        self.assertEqual(first_records[0]["record"], "lexicon")
        records = first_records[1:]
        phases = [0 if item["record"] == "node" else 1 if item["record"] == "edge" else 2 for item in records]
        self.assertEqual(phases, sorted(phases))
        self.assertEqual(len({item["id"] for item in records if item["record"] == "node"}), len([item for item in records if item["record"] == "node"]))

    def test_contract_validator_accepts_cli_output(self) -> None:
        output = self.repo / "facts.jsonl"
        self._run(output)
        result = subprocess.run(
            [sys.executable, str(REPO_ROOT / "tools" / "validate_jsonl.py"), str(output)],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_dependency_manifests_and_local_imports_are_deterministic(self) -> None:
        self._write(
            "pyproject.toml",
            "[project]\n"
            "dependencies = [\"requests>=2\", \"broken [\"]\n"
            "[project.optional-dependencies]\n"
            "test = [\"pytest~=8\"]\n",
        )
        self._write("pkg/local.py", "VALUE = 1\n")
        self._write("dependency_user.py", "from pkg.local import VALUE\n")
        first = self._run(self.repo / "dependencies-first.jsonl")
        second = self._run(self.repo / "dependencies-second.jsonl")
        self.assertEqual((self.repo / "dependencies-first.jsonl").read_bytes(), (self.repo / "dependencies-second.jsonl").read_bytes())
        edges = [record for record in first if record["record"] == "edge"]
        dependency_edges = [record for record in edges if record["relation"] == "depends-on"]
        self.assertGreaterEqual(len(dependency_edges), 3)
        self.assertTrue(any(record["attributes"]["category"] == "runtime" and record["attributes"]["constraint"] == ">=2" for record in dependency_edges))
        self.assertTrue(any(record["attributes"]["category"] == "test" and record["attributes"]["dev"] for record in dependency_edges))
        self.assertTrue(any(record["attributes"]["category"] == "local" and record["attributes"]["path"] for record in dependency_edges))
        self.assertFalse(any(record.get("qualified_name") == "dependency:python:broken" for record in first if record["record"] == "node"))


if __name__ == "__main__":
    unittest.main()
