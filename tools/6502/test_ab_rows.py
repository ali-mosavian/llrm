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

if __name__ == "__main__":
    unittest.main()
