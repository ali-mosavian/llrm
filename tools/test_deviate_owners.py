"""Every DEVIATE in pipeline.rs names an owner and an exit date: a deviation nobody owns never gets removed."""

import re
import unittest
from pathlib import Path

PIPELINE = Path(__file__).resolve().parent.parent / "crates/opt/llrm-transforms/src/pipeline.rs"


def blocks(lines):
    """(first line number, text) of each run of comment lines that holds a DEVIATE."""
    out, start, run = [], None, []
    for number, line in enumerate(lines + [""], 1):
        if line.lstrip().startswith("//"):
            start = start or number
            run.append(line)
            continue
        text = " ".join(run)
        if run and re.search(r"(?<!numbered )DEVIATE", text):
            out.append((start, text))
        start, run = None, []
    return out


class DeviateOwners(unittest.TestCase):
    def test_every_deviate_comment_names_an_owner_and_an_exit(self):
        """A DEVIATE comment without both stays forever (11-05: nine of them had no exit date)."""
        found = blocks(PIPELINE.read_text().splitlines())
        self.assertTrue(found, "premise: pipeline.rs holds DEVIATE comments")
        bad = [
            f"pipeline.rs:{n}"
            for n, text in found
            if not (re.search(r"\bowner\b", text, re.I) and re.search(r"\bexit\b\s*20\d\d-\d\d-\d\d", text, re.I))
        ]
        self.assertEqual(bad, [], "DEVIATE without `owner X, exit YYYY-MM-DD`")


if __name__ == "__main__":
    unittest.main()
