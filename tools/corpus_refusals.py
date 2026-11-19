#!/usr/bin/env python3
"""Which tests/run/c programs a target refuses, each compiled the way its own header says.

`classify` returns ("na", reason) where the program's `// targets:` excludes the target (`corpus_targets.applies`), ("ok",) where one
of its `// flags:` configurations compiles, else ("refused", first error line). A harness that compiled every program with the table's
flags alone counted 17 programs needing `-ffixed-point`, `-Wno-incompatible-ms-struct` or the runtime include dirs as refusals.
"""
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import corpus_targets  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
INCLUDES = ["-I", str(REPO / "runtime/shared/posix"), "-I", str(REPO / "bench")]
MODE = ("-m16", "-m32", "-m64", "-march=i386", "-march=i486", "-march=pentium")  # the target row sets the mode


def own_flags(configuration: list[str]) -> list[str]:
    """A configuration without its optimisation level and x86 mode: the harness sets the level, the target the mode."""
    return [t for t in configuration if not t.startswith("-O") and t not in MODE]


def classify(target: str, source: str, compile_with, path: Path | None = None) -> tuple:
    """`compile_with(flags) -> (returncode, stderr)`; flags are the program's own plus the include dirs."""
    if not corpus_targets.applies(target, source, path):
        return ("na", corpus_targets.reason(source, path))
    error = ""
    for configuration in corpus_targets.flag_configurations(source) or [[]]:
        code, stderr = compile_with([*own_flags(configuration), *INCLUDES])
        if code == 0:
            return ("ok",)
        error = next((line for line in stderr.splitlines() if "error" in line or "llrm-" in line), stderr[:120])
    return ("refused", error[:160])


def main(argv: list[str]) -> int:
    if len(argv) < 4:
        print("usage: corpus_refusals.py LLRM_CC TARGET LEVEL [-mXX...] -- FILE.c...", file=sys.stderr)
        return 2
    driver, target, level, *rest = argv[1:]
    split = rest.index("--")
    mode, files = rest[:split], rest[split + 1 :]
    for name in files:
        path = Path(name)

        def compile_with(flags, path=path):
            ran = subprocess.run([driver, *mode, f"--target={target}", level, *flags, "-c", str(path), "-o", "/dev/null"], capture_output=True, text=True, cwd=path.parent)
            return ran.returncode, ran.stderr

        print(path.stem, *classify(target, path.read_text(), compile_with, path), sep="\t")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
