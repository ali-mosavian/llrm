"""Every module of the MIR corpus (tools/mir-corpus.sh) that llrm-llc can compile still compiles.

A QB module names `@llrm.qb.*` runtime routines, which only the llrm-qb link step maps to symbols; llrm-llc alone
refuses all of them ("is no assembler symbol"), so those are skipped. A corpus entry is text written once; a datalayout or symbol rule that moved since leaves it unreadable, and no test
that parses it alone notices. Regenerate with `tools/mir-corpus.sh`."""

import subprocess
import sys
import tempfile
import unittest
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import llrmbin  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "crates" / "opt" / "llrm-analysis" / "corpus"


def refusal(llc: Path, entry: Path, out: Path) -> str | None:
    """Why llrm-llc does not compile `entry`, or None."""
    done = subprocess.run([str(llc), str(entry), "-o", str(out)], capture_output=True, text=True, timeout=600)
    if done.returncode == 0:
        return None
    lines = (done.stderr or done.stdout).strip().splitlines()
    return lines[0][:200] if lines else f"exit {done.returncode}"


class CorpusTests(unittest.TestCase):
    def test_every_corpus_module_compiles_with_llrm_llc(self):
        """A stale datalayout or symbol rule left a corpus entry unreadable and nothing said the corpus had rotted."""
        llc = llrmbin.bin_dir() / "llrm-llc"
        entries = sorted(e for e in CORPUS.glob("*/*.ll") if "@llrm.qb." not in e.read_text() and '"llrm.qb.' not in e.read_text())
        self.assertTrue(entries, "an empty corpus")
        with tempfile.TemporaryDirectory() as work, ThreadPoolExecutor(8) as pool:
            results = list(pool.map(lambda pair: refusal(llc, pair[1], Path(work) / f"{pair[0]}.o"), enumerate(entries)))
        stale = [f"{entry.parent.name}/{entry.name}: {why}" for entry, why in zip(entries, results) if why]
        self.assertEqual(stale, [], "regenerate with tools/mir-corpus.sh")


if __name__ == "__main__":
    unittest.main()
