"""The x87 operations the QB runtime's number formatting uses, run in the emulator without any compiler: the results against the table recorded on
the dev box (x87probe.expected). A host whose FPU rounds differently shows here, not as a red demo.

    python3 tools/dosbatch/x87probe.py        # prints one verdict line (and the differing lines); always exits 0"""
import dataclasses
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import dosbatch  # noqa: E402

EXPECTED = HERE / "x87probe.expected"


def assemble(work: Path) -> Path:
    program = work / "x87probe.com"
    done = subprocess.run([str(dosbatch.BIN / "jwasm"), "-q", "-bin", f"-Fo{program}", str(HERE / "x87probe.asm")], capture_output=True, text=True)
    if done.returncode != 0:
        raise dosbatch.BuildError(done.stdout + done.stderr)
    return program


def run(work: Path) -> list[str]:
    """The probe's lines, from one run in the emulator."""
    program = assemble(work)
    tools = dataclasses.replace(dosbatch.QB45_TOOLS, mount=work)
    result = dosbatch.run([dosbatch.Job("X87", "exe", program)], work / "run", timeout=120, budget_ms=2000, build_ms=5000, tools=tools)["X87"]
    if result.status != "ok":
        raise dosbatch.BuildError(f"x87 probe: {result.status} {result.detail}")
    return [line.strip() for line in result.text.splitlines() if line.strip()]


def verdict(got: list[str], expected: list[str]) -> tuple[str, list[str]]:
    """('matches the table' or how it differs, the differing lines)."""
    bad = [f"line {i + 1}: got {got[i] if i < len(got) else '(none)'}, table {want}" for i, want in enumerate(expected) if i >= len(got) or got[i] != want]
    bad += [f"line {i + 1}: extra {line}" for i, line in enumerate(got) if i >= len(expected)]
    return (f"{len(bad)} of {len(expected)} lines differ" if bad else f"all {len(expected)} lines match"), bad


def main() -> int:
    with tempfile.TemporaryDirectory() as where:
        got = run(Path(where))
    if "--record" in sys.argv:
        EXPECTED.write_text("\n".join(got) + "\n")
        print(f"x87 probe: recorded {len(got)} lines")
        return 0
    summary, bad = verdict(got, EXPECTED.read_text().split("\n")[:-1] if EXPECTED.exists() else [])
    print(f"x87 probe (host FPU under DOSBox-X): {summary}")
    for line in bad[:12]:
        print("  " + line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
