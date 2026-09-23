"""Write every QB source and flag set tests/test_hir.py compiles to fixtures/qb/port/.

    uv run python tools/qb_port_corpus.py

Each lands as `<hash>/<name>.bas` beside `<name>.flags`, which `port_diff --qb`
passes to both compilers. `bench/parity` sources at default flags are skipped:
port_diff runs those already.
"""

import sys
import shutil
import hashlib
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
PORT = ROOT / "fixtures/qb/port"
TESTS = ("tests/test_hir.py",)
DEFAULTS = {
    "dialect": "vbdos",
    "runtime": "vbdos",
    "include_dirs": (),
    "array_order": "column-major",
    "huge_arrays": False,
    "checked_arrays": False,
    "unchecked_bounds": False,
    "mbf": False,
    "alternate_math": False,
}


class Recorder:
    """A pytest plugin recording each source and flag set the QB driver compiles."""

    # port_diff runs these sources at default flags already.
    already = ROOT / "bench/parity"

    def __init__(self) -> None:
        self.compiled: dict[str, tuple[str, bytes, tuple[str, ...]]] = {}

    def pytest_configure(self, config: pytest.Config) -> None:
        from qbopt.frontend.qb import driver

        options = driver._options

        def recorded(source: Path, **kwargs: object) -> tuple[str, ...]:
            flags = options(source, **kwargs)
            defaults = options(source, **{**kwargs, **DEFAULTS})
            self.record(source, flags[:-1], defaults[:-1])  # the source is last
            return flags

        driver._options = recorded

    def record(self, source: Path, flags: tuple[str, ...], defaults: tuple[str, ...]) -> None:
        source = Path(source).resolve()
        if source.parent == self.already and flags == defaults:
            return
        text = source.read_bytes()
        key = hashlib.sha256(text + b"\0" + source.name.encode() + b"\0" + " ".join(flags).encode()).hexdigest()[:10]
        self.compiled[key] = (source.name, text, flags)


def write(compiled: dict[str, tuple[str, bytes, tuple[str, ...]]], port: Path) -> None:
    shutil.rmtree(port, ignore_errors=True)
    for key, (name, text, flags) in compiled.items():
        folder = port / key
        folder.mkdir(parents=True)
        (folder / name).write_bytes(text)
        (folder / name).with_suffix(".flags").write_text("\n".join(flags) + "\n")


def main() -> int:
    sys.path.insert(0, str(ROOT))  # this checkout's qbopt, not the one the venv installed
    recorder = Recorder()
    code = pytest.main(
        [*(str(ROOT / one) for one in TESTS), "--full", "-q", "-p", "no:cacheprovider"], plugins=[recorder]
    )
    if code not in (pytest.ExitCode.OK, pytest.ExitCode.TESTS_FAILED):
        return int(code)
    write(recorder.compiled, PORT)
    print(f"{len(recorder.compiled)} sources in {PORT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
