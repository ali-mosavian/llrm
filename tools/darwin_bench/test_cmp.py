"""A cell that regresses on one metric alone must be flagged (novec.py ignored insns, so an instruction-only regression printed nothing)."""
import unittest
from cmp import regressions, regressions_vs_best, clock_note


class T(unittest.TestCase):
    def test_each_metric_alone(self):
        ref = (100, 30, 10.0)
        self.assertEqual(regressions((104, 30, 10.0), ref), ["bytes"])
        self.assertEqual(regressions((100, 31, 10.0), ref), ["insns"])
        self.assertEqual(regressions((100, 30, 12.0), ref), ["us"])

    def test_clean(self):
        self.assertEqual(regressions((100, 30, 10.5), (100, 30, 10.0)), [])

    def test_best_of_takes_min_per_metric(self):
        self.assertEqual(regressions_vs_best((100, 30, 10.0), [(90, 40, 10.0), (120, 25, 10.0)]), ["bytes", "insns"])


    def test_clock_note_marks_noisy_above_ten_percent(self):
        """Every clocks cell was printed as plain 'us' although spreads reach 10-300% on the loaded Mac."""
        self.assertEqual(clock_note(2.0, [10.0, 10.5, 10.2]), "darwin us, load 2.0, spread 5%")
        self.assertEqual(clock_note(2.0, [10.0, 12.0, 10.2]), "darwin us, load 2.0, spread 20%, noisy")
        self.assertTrue(clock_note(2.0, [10.0, 10.1, 10.2], ok=False).endswith("output differs from .out"))
unittest.main()
