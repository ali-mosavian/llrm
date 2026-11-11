import unittest
from ratesum import compare, counts

def row(bytes_, *layouts):
    return ("PASS", {"bytes": bytes_, "layouts": list(layouts)})

class RateSum(unittest.TestCase):
    def test_sums_clock_mean_and_three_per_byte(self):
        base = {"a": row(10, 100, 200), "idioms": row(5, 1000, 1000)}
        head = {"a": row(12, 110, 210), "idioms": row(5, 900, 1100)}
        total, clk, size, _, none = compare(base, head, ["a", "idioms"])
        self.assertEqual((total, clk, size, none), (16.0, 10.0, 2, []))

    def test_without_idioms_skips_it(self):
        base = {"a": row(10, 100), "idioms": row(5, 1000)}
        head = {"a": row(10, 100), "idioms": row(5, 5000)}
        self.assertEqual(compare(base, head, ["a", "idioms"], skip=("idioms",))[0], 0.0)

    def test_a_program_not_passing_on_both_sides_is_listed_not_summed(self):
        """A WRONG/N/A/missing cell once read as zero rate change and hid a 1.4k regression."""
        base = {"a": row(10, 100), "b": row(10, 100)}
        head = {"a": ("N/A", "n/a"), "b": row(10, 150)}
        total, _, _, _, none = compare(base, head, ["a", "b", "c"])
        self.assertEqual(total, 50.0)
        self.assertEqual(none, [("a", "PASS", "N/A"), ("c", "missing", "missing")])

    def test_counts_name_each_status(self):
        self.assertEqual(counts({"a": row(1, 1), "b": ("WRONG", "x")}, ["a", "b", "c"]), {"PASS": 1, "WRONG": 1, "missing": 1})

if __name__ == "__main__":
    unittest.main()
