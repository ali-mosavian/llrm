import os, tempfile, unittest
import ab_rows

def tsv(rows):
    f = tempfile.NamedTemporaryFile("w", suffix=".tsv", delete=False)
    f.write("".join(f"{n}\tours\t{b}\t{c}\n" for n, b, c in rows)); f.close()
    return f.name

class Rows(unittest.TestCase):
    """The 2026-11-07 A/B of #7698 printed PASS counts and said "none worse" while 13 programs grew in bytes or clocks:
    a grown row must make the script fail."""
    def test_a_row_with_more_clocks_is_worse_and_fails_the_run(self):
        a, b = tsv([("p", 100, 1000), ("q", 50, 10)]), tsv([("p", 100, 1001), ("q", 49, 10)])
        self.assertEqual(ab_rows.main(["x", a, b]), 1)
        self.assertEqual(ab_rows.main(["x", a, b, "--allow-worse", "1"]), 0)

    def test_a_row_with_more_bytes_is_worse_even_when_it_runs_faster(self):
        a, b = tsv([("p", 100, 1000)]), tsv([("p", 101, 900)])
        self.assertEqual(ab_rows.main(["x", a, b]), 1)

    def test_a_row_only_one_side_has_fails_the_run(self):
        a, b = tsv([("p", 100, 1000), ("q", 1, 1)]), tsv([("p", 100, 1000)])
        self.assertEqual(ab_rows.main(["x", a, b]), 1)

    def test_equal_and_smaller_rows_pass(self):
        a, b = tsv([("p", 100, 1000), ("q", 50, 10)]), tsv([("p", 100, 1000), ("q", 49, 9)])
        self.assertEqual(ab_rows.main(["x", a, b]), 0)

if __name__ == "__main__":
    unittest.main()
