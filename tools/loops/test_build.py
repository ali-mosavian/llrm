"""`uv run --project tools python -m unittest discover -s tools/loops`"""

import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import build  # noqa: E402

NAMES = ("llrm-c", "llrm-qb", "llrm-nib", "llrm-mir")


def tree(where: Path, binaries: float, source: float) -> None:
    (where / "crates/x/src").mkdir(parents=True)
    (where / "target/release").mkdir(parents=True)
    (where / "crates/x/src/a.rs").write_text("fn a() {}")
    os.utime(where / "crates/x/src/a.rs", (source, source))
    for name in NAMES:
        (where / "target/release" / name).write_text("")
        os.utime(where / "target/release" / name, (binaries, binaries))


class StaleTests(unittest.TestCase):
    def test_binaries_older_than_the_sources_are_refused_with_what_to_rebuild(self):
        """repo-programs saw 14 shortfalls main's own binaries do not give: a run
        used frontends older than the tools and the file it judged against."""
        with tempfile.TemporaryDirectory() as where:
            tree(Path(where), binaries=1_000, source=2_000)
            why = build.stale_binaries(Path(where), Path(where) / "target/release")
            self.assertTrue(why and "cargo build --release --bins" in why[0], why)

    def test_binaries_newer_than_the_sources_are_current(self):
        with tempfile.TemporaryDirectory() as where:
            tree(Path(where), binaries=3_000, source=2_000)
            self.assertEqual(build.stale_binaries(Path(where), Path(where) / "target/release"), [])

    def test_a_missing_binary_is_refused(self):
        with tempfile.TemporaryDirectory() as where:
            tree(Path(where), binaries=3_000, source=2_000)
            (Path(where) / "target/release/llrm-mir").unlink()
            self.assertTrue(build.stale_binaries(Path(where), Path(where) / "target/release"))


if __name__ == "__main__":
    unittest.main()
