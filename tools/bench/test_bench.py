"""`uv run --project tools python -m unittest discover -s tools/bench`"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import bench  # noqa: E402

BASE = {"instructions": 100, "memory_operands": 40}


class GateTests(unittest.TestCase):
    def test_the_baseline_itself_passes(self):
        self.assertEqual(bench.against("x", dict(BASE), BASE), [])

    def test_a_worse_count_fails(self):
        """The reason the gate exists: +10k instructions on one benchmark showed only as a total in a PR body."""
        problems = bench.against("textfill c -O2", {"instructions": 101, "memory_operands": 40}, BASE)
        self.assertEqual(len(problems), 1)
        self.assertIn("(worse)", problems[0])

    def test_a_better_count_fails_until_it_is_blessed(self):
        problems = bench.against("x", {"instructions": 100, "memory_operands": 39}, BASE)
        self.assertIn("(better: --bless", problems[0])

    def test_no_baseline_is_a_failure_not_a_pass(self):
        self.assertEqual(len(bench.against("x", dict(BASE), None)), 1)


if __name__ == "__main__":
    unittest.main()


class KnownTests(unittest.TestCase):
    def variant(self):
        return bench.Variant("fpbench", "c", Path("x.c"), "bench_fpbench", "#358")

    def test_a_known_variant_that_fails_only_at_one_level_is_still_measured_at_the_other(self):
        """fpbench fails at -O2 (#358) and compiles at -Os: the -Os count has to gate."""
        got = {("fpbench", "c", "O2"): {"error": "stack operands"}, ("fpbench", "c", "Os"): dict(BASE)}
        marked = bench.mark_known(got, [self.variant()], ["O2", "Os"])
        self.assertIn("known", marked[("fpbench", "c", "O2")])
        self.assertEqual(marked[("fpbench", "c", "Os")], BASE)

    def test_a_known_mark_on_a_variant_that_works_everywhere_fails(self):
        got = {("fpbench", "c", "O2"): dict(BASE), ("fpbench", "c", "Os"): dict(BASE)}
        marked = bench.mark_known(got, [self.variant()], ["O2", "Os"])
        self.assertIn("xpass", marked[("fpbench", "c", "O2")])


class BlessTests(unittest.TestCase):
    def test_a_reason_with_quotes_leaves_a_readable_file(self):
        """--reason 'VAL("0")' wrote reason = "...VAL("0")...": the next gate run died on the TOML."""
        import tempfile
        import tomllib

        with tempfile.TemporaryDirectory() as where:
            bench.write_expected(Path(where), {("x", "c", "O2"): dict(BASE)}, "x", 'folded VAL("0") \\ away')
            read = tomllib.loads((Path(where) / "expected.toml").read_text())
        self.assertEqual(read["reason"], 'folded VAL("0") \\ away')
        self.assertEqual(read["c"]["O2"], BASE)
