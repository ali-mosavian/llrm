import unittest

from worse import compare


class Compare(unittest.TestCase):
    def test_a_cell_where_only_the_clocks_regress_is_worse(self):
        """A bytes-only comparator printed "0 worse" for a cell that got slower at the same size."""
        worse, better, mixed = compare({("p", "O2"): (10, 100, 20)}, {("p", "O2"): (10, 101, 20)})
        self.assertEqual([c for c, _, _ in worse], [("p", "O2")])
        self.assertEqual((better, mixed), ([], []))

    def test_a_cell_where_only_the_instrs_regress_is_worse(self):
        worse, _, _ = compare({("p", "O2"): (10, 100, 20)}, {("p", "O2"): (10, 100, 21)})
        self.assertEqual(len(worse), 1)

    def test_a_trade_is_mixed_and_a_pure_gain_is_better(self):
        worse, better, mixed = compare(
            {("a", "O2"): (10, 100, 20), ("b", "O2"): (10, 100, 20)},
            {("a", "O2"): (9, 101, 20), ("b", "O2"): (9, 100, 20)},
        )
        self.assertEqual(([c for c, _, _ in mixed], [c for c, _, _ in better], worse), ([("a", "O2")], [("b", "O2")], []))


if __name__ == "__main__":
    unittest.main()
