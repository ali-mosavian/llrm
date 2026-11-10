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
        self.assertEqual(ab_rows.main(["x", a, b, "--level", "O1"]), 3)  # 2 layouts: the mean's failure is UNCONFIRMED (class Residues)
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

    def test_a_row_that_fails_on_both_sides_is_unmeasured_and_fails_the_run(self):
        """2026-11-09: 72 of 162 cells failed ca65 on both sides, left both tsv files, and the A/B printed "-12 B, 0 larger" over 90 cells."""
        import contextlib, io, json
        a, b = tsv([("p", 10, 10)]), tsv([("p", 10, 10)])
        for path in (a, b):
            json.dump([["p", "PASS", {}], ["q", "LINK", "ca65 range error"]], open(path + ".json", "w"))
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            status = ab_rows.main(["x", a, b, "--level", "Os"])
        self.assertEqual(status, 1)
        self.assertIn("UNMEASURED q", out.getvalue())
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(ab_rows.main(["x", a, b, "--level", "Os", "--allow-unmeasured", "1"]), 0)

    def test_a_helper_source_without_an_out_is_not_a_program_and_does_not_void_the_run(self):
        import contextlib, io, json
        a, b = tsv([("p", 10, 10)]), tsv([("p", 10, 10)])
        for path in (a, b):
            json.dump([["p", "PASS", {}], ["helper", "NO-OUT", "no .out"]], open(path + ".json", "w"))
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(ab_rows.main(["x", a, b, "--level", "Os"]), 0)

class Residues(unittest.TestCase):
    """2026-11-10: the 16-pad mean (pads 0,16..240: 16 of the 256 address residues a page crossing depends on) called 8 cells worse that
    all 256 residues show better (6502 O1 mul_constants -5 B, +8888 cy on 16, -2000 cy on 256). A clocks-only failure read from fewer
    than 256 layouts is UNCONFIRMED, not FAIL, and says how to confirm it; bytes do not depend on the layout and still fail."""
    def table(self, rows, layouts):
        import json
        path = tsv([(n, b, c) for n, b, c in rows])
        json.dump([[n, "PASS", {"layouts": [c] * layouts}] for n, b, c in rows], open(path + ".json", "w"))
        return path
    def go(self, base, new, layouts, level="O1"):
        import contextlib, io
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            status = ab_rows.main(["x", self.table(base, layouts), self.table(new, layouts), "--level", level])
        return status, out.getvalue()

    def test_a_clocks_only_failure_on_16_layouts_is_unconfirmed_not_failing(self):
        status, text = self.go([("p", 100, 1000)], [("p", 99, 1010)], 16)
        self.assertEqual(status, 3)
        self.assertIn("UNCONFIRMED p", text)
        self.assertNotIn("FAIL", text)
        self.assertIn("seq -s, 0 255", text)
    def test_the_same_failure_on_all_256_layouts_fails(self):
        status, text = self.go([("p", 100, 1000)], [("p", 99, 1010)], 256)
        self.assertEqual(status, 1)
        self.assertIn("FAIL", text)
    def test_a_cell_better_on_all_256_layouts_passes(self):
        self.assertEqual(self.go([("p", 100, 1010)], [("p", 99, 1000)], 256)[0], 0)
    def test_bytes_added_fail_whatever_the_layout_count(self):
        status, text = self.go([("p", 100, 1000)], [("p", 101, 1000)], 16, "Os")
        self.assertEqual(status, 1)
        self.assertIn("FAIL", text)
    def test_a_table_without_layouts_keeps_its_old_verdict(self):
        self.assertEqual(run("O1", [("p", 100, 1000)], [("p", 99, 1010)]), 1)

if __name__ == "__main__":
    unittest.main()
