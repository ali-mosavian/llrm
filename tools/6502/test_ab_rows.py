import tempfile, unittest
import ab_rows

def tsv(rows):
    f = tempfile.NamedTemporaryFile("w", suffix=".tsv", delete=False)
    f.write("".join(f"{n}\tours\t{b}\t{c}\n" for n, b, c in rows)); f.close()
    return f.name

def run(level, a, b, *extra):
    return ab_rows.main(["x", tsv(a), tsv(b), "--level", level, *extra])

class Rows(unittest.TestCase):
    """The 2026-11-07 A/B of #7698 printed PASS counts and said "none worse" while 13 programs grew in bytes or clocks:
    a row that breaks the level's rule must make the script fail."""
    def test_o1_a_row_with_more_clocks_fails_even_when_smaller(self):
        self.assertEqual(run("O1", [("p", 100, 1000)], [("p", 50, 1001)]), 1)

    def test_o1_an_equal_or_smaller_row_passes(self):
        self.assertEqual(run("O1", [("p", 100, 1000), ("q", 5, 5)], [("p", 100, 1000), ("q", 4, 5)]), 0)

    def test_os_judges_bytes_only(self):
        self.assertEqual(run("Os", [("p", 100, 1000)], [("p", 99, 5000)]), 0)
        self.assertEqual(run("Os", [("p", 100, 1000)], [("p", 101, 10)]), 1)

    def test_o2_clocks_added_must_be_covered_by_bytes_saved_at_three_to_one(self):
        self.assertEqual(run("O2", [("p", 100, 1000)], [("p", 90, 1030)]), 0)
        self.assertEqual(run("O2", [("p", 100, 1000)], [("p", 90, 1031)]), 1)

    def test_o2_bytes_added_must_be_paid_for_in_clocks(self):
        self.assertEqual(run("O2", [("p", 100, 1000)], [("p", 101, 997)]), 0)
        self.assertEqual(run("O2", [("p", 100, 1000)], [("p", 101, 1000)]), 1)

    def test_a_row_only_one_side_has_fails_the_run(self):
        self.assertEqual(run("O1", [("p", 1, 1), ("q", 1, 1)], [("p", 1, 1)]), 1)

    def test_allow_worse_counts_failing_rows(self):
        self.assertEqual(run("O1", [("p", 1, 1)], [("p", 1, 2)], "--allow-worse", "1"), 0)

    def test_noise_forgives_clocks_within_the_fraction_only(self):
        self.assertEqual(run("O1", [("p", 100, 10000)], [("p", 90, 10005)], "--noise", "0.001"), 0)
        self.assertEqual(run("O1", [("p", 100, 10000)], [("p", 90, 10011)], "--noise", "0.001"), 1)
        self.assertEqual(run("O1", [("p", 100, 10000)], [("p", 101, 10000)], "--noise", "0.001"), 1)

    def test_min_clocks_read_the_fastest_layout_beside_the_tsv(self):
        import json
        def with_layouts(rows):
            path = tsv([(n, b, c) for n, b, c, _ in rows])
            json.dump([[n, "PASS", {"layouts": l}] for n, _, _, l in rows], open(path + ".json", "w"))
            return path
        a = with_layouts([("p", 100, 1050, [1000, 1100])])
        b = with_layouts([("p", 100, 1100, [1000, 1200])])
        self.assertEqual(ab_rows.main(["x", a, b, "--level", "O1"]), 1)
        self.assertEqual(ab_rows.main(["x", a, b, "--level", "O1", "--clocks", "min"]), 0)

class Void(unittest.TestCase):
    """The 2026-11-07 funnel A/B printed "failing 0 better 9" for a build that panicked on 42 of 86 programs:
    the first line read as a verdict. A run with rows on one side only says VOID first."""
    def first_line(self, a, b):
        import io, contextlib
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            ab_rows.main(["x", tsv(a), tsv(b), "--level", "O1"])
        return out.getvalue().splitlines()[0]

    def test_rows_on_one_side_only_make_the_first_line_void(self):
        self.assertIn("VOID", self.first_line([("p", 1, 1), ("q", 1, 1)], [("p", 1, 1)]))

    def test_a_complete_run_has_no_void_line(self):
        self.assertNotIn("VOID", self.first_line([("p", 1, 1)], [("p", 1, 1)]))

    def test_a_compiler_that_panics_leaves_its_program_out_of_the_table(self):
        import json, os, pathlib, subprocess, sys, tempfile
        here = pathlib.Path(__file__).resolve().parent
        work = pathlib.Path(tempfile.mkdtemp())
        cc = work / "panicking-cc"
        cc.write_text("#!/bin/sh\necho \"thread 'main' panicked at dag.rs:1080:29:\" >&2\nexit 101\n"); cc.chmod(0o755)
        out = work / "p.tsv"
        env = dict(os.environ, OURS_RT=str(here.parents[1] / "crates/target/llrm-6502/runtime/mos_rt.s"),
                   OURS_CORPUS=str(here.parents[1] / "tests/run/c"))
        subprocess.run([sys.executable, str(here / "ours_cr.py"), str(cc), "O1", str(out), "bit_setclear"], env=env, cwd=here,
                       capture_output=True, timeout=120)
        status = [row[1] for row in json.load(open(str(out) + ".json"))]
        self.assertEqual(status, ["REFUSED"])
        good = tsv([("bit_setclear", 10, 10)])
        self.assertEqual(ab_rows.main(["x", good, str(out), "--level", "O1"]), 1)

if __name__ == "__main__":
    unittest.main()
