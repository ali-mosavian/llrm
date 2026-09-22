"""Write every modern source and flag set the modern tests compile to fixtures/modern/port/.

    uv run python tools/modern_port_corpus.py

Each lands as `<hash>/<name>.mod` beside `<name>.flags`, which `port_diff
--modern` passes to both compilers. A parsed source is recorded at default
flags; one compiled is recorded again with its `--entry` and `-O`. Nothing
runs: an executable build, and the interpreter checking it, are refused once
the source is recorded.
`fixtures/modern` sources at default flags are skipped: port_diff runs those
already.
"""

import sys
from pathlib import Path

import pytest
from qb_port_corpus import ROOT
from qb_port_corpus import Recorder
from qb_port_corpus import write

PORT = ROOT / "fixtures/modern/port"
TESTS = (
    "tests/test_modern_frontend.py",
    "tests/test_modernstages.py",
    "tests/test_modern_e2e.py",
    "tests/test_hir_execute.py",
    "tests/test_mir_execute.py",
    "tests/test_farload.py",
    "tests/test_cpu_profile.py",
)


def flags(entry: str, options: object, levels: dict) -> tuple[str, ...] | None:
    """The CLI spelling of one compile; None where the CLI cannot say it."""
    level = next((name for name, one in levels.items() if one == options), None)
    if level is None:
        return None
    return (*(("--entry", entry) if entry != "main" else ()), *(("-O", level[1:]) if level != "O2" else ()))


class ModernRecorder(Recorder):
    """A pytest plugin recording each source the modern driver parses and the flags it compiles at."""

    already = ROOT / "fixtures/modern"

    def pytest_configure(self, config: pytest.Config) -> None:
        from qbopt.model.passes import O2
        from qbopt.model.passes import LEVELS
        from qbopt.frontend.modern import driver
        from qbopt.frontend.modern import compile as modern

        parsed, assembled = driver.parsed, modern.assembled
        sources: dict[int, tuple[object, Path]] = {}

        def parsing(source: Path, **kwargs: object):
            program = parsed(source, **kwargs)
            sources[id(program)] = (program, Path(source))  # the program keeps its id unique
            self.record(source, (), ())
            return program

        def assembling(program, *, entry: str, cpu="386", options=O2):
            known = sources.get(id(program))
            spelled = flags(entry, options, LEVELS)
            if known is not None and known[0] is program and cpu == "386" and spelled is not None:
                self.record(known[1], spelled, ())
            return assembled(program, entry=entry, cpu=cpu, options=options)

        driver.parsed = parsing
        modern.assembled = assembling

        import modernexe

        def undosed(*_: object, **__: object) -> None:
            raise modernexe.BuildError("the corpus records sources; it runs no DOSBox")

        modernexe.launch = undosed
        self.build = modernexe.build

    def pytest_runtest_setup(self, item: pytest.Item) -> None:
        # A test that builds an executable runs the interpreter only to check it,
        # and matmul takes the interpreter half an hour.
        module = getattr(item, "module", None)
        if getattr(module, "build", None) is self.build and hasattr(module, "execute"):
            module.execute = Unexecuted


class Unexecuted:
    """The interpreter, refused where only an executable's reference output wants it."""

    @staticmethod
    def run(*_: object, **__: object) -> None:
        raise RuntimeError("the corpus records sources; it runs no interpreter here")


def main() -> int:
    sys.path.insert(0, str(ROOT))  # this checkout's qbopt, not the one the venv installed
    recorder = ModernRecorder()
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
