"""tools/nib-build.sh passes $LLRM_ISEL to every compile it makes.

Without it a real-mode Nib program, its runtime and its foreign C were built by the old selector under
`LLRM_ISEL=dag`: a run of the Nib group reported 359 functions selected by the old selector (45 names: `main`,
`heap.fit`, `N$PN`, ...) and "all pass" proved nothing about the DAG path for them.
"""

import re
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "nib-build.sh"


def compiles(text: str) -> list[str]:
    return [line for line in text.splitlines() if re.search(r'"\$bin/llrm-(nib|c|cc)" ', line) and " -o " in line]


class NibBuildSelectorTests(unittest.TestCase):
    def test_every_compile_in_nib_build_passes_the_selector_the_run_asks_for(self):
        text = SCRIPT.read_text()
        lines = compiles(text)
        self.assertEqual(len(lines), 3, lines)
        self.assertTrue(all("$isel" in line for line in lines), lines)
        self.assertIn("isel=${LLRM_ISEL:+--isel=$LLRM_ISEL}", text)


if __name__ == "__main__":
    unittest.main()
