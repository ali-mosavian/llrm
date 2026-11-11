"""The flags a program's own `// flags:` header asks for, at one harness level.

  headerflags.py llrm|gcc|clang SOURCE LEVEL   prints the flags to add to that column's compile line (empty: none)

A tests/run/c header lists configurations, `// flags: -O2 -fwrapv | -Os -fwrapv`. The m32 harness runs the program at -O1, -O2, -O3
and -Os on one target (-march=i486, -mabi=sysv): the configuration is the first one whose -O is the level and whose machine flags
are that target's, else the first at -O2. Its other flags are the program's meaning, not an option: `-fwrapv` decides what a signed
overflow test returns (wrapv_overflow_check built without it returned 0 0 0 1 where the test wants 1 1 0 1, `ok: false`, and
its sizes were another program's). Every column gets them; gcc takes the spelling `external_flags.gcc_program_flags` gives, and a
flag gcc has no spelling for leaves the gcc column n/a (exit 3, build.sh builds none).
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "tools"))
import external_flags  # noqa: E402

# What the harness states itself on every line: the level, the CPU, the ABI, the long double width and the target.
HARNESS_TOKENS = ("-march=i486", "-m32", "-mabi=sysv", "-mlong-double-80", "-mlong-double-64")
LEVEL = re.compile(r"-O([0-3sz]|max)$")


def configs(text):
    """The header's configurations as token lists; none for a file without a header."""
    for line in text.splitlines():
        if line.startswith("// flags:"):
            return [alternative.split() for alternative in line[len("// flags:"):].split("|")]
    return []


def level_of(tokens):
    levels = [m.group(1) for t in tokens if (m := LEVEL.match(t))]
    return levels[-1] if levels else None


def on_this_target(tokens):
    """Whether a configuration is for the m32 harness: every machine flag it states is one the harness states."""
    machine = [t for t in tokens if t.startswith("-m") and not t.startswith(("-mlong-double", "-mno-", "-mpreferred"))]
    return all(t in HARNESS_TOKENS for t in machine) and not any(t.startswith("-march=") and t != "-march=i486" for t in tokens)


def program_tokens(text, level):
    """The program's flags at `level` without the ones the harness states: the first compatible configuration at the level, else at O2."""
    compatible = [c for c in configs(text) if on_this_target(c)]
    for want in (level, "2"):
        for config in compatible:
            if level_of(config) == want.removeprefix("O"):
                return [t for t in config if not LEVEL.match(t) and t not in HARNESS_TOKENS and not t.startswith("-W")]
    return []


def column_flags(kind, text, level):
    """The tokens for the `llrm`, `gcc` or `clang` column; ValueError where gcc has no spelling for one."""
    tokens = program_tokens(text, level)
    return external_flags.gcc_program_flags(tokens) if kind == "gcc" else tokens


if __name__ == "__main__":
    kind, source, level = sys.argv[1:4]
    try:
        print(" ".join(column_flags(kind, Path(source).read_text(errors="replace"), level)))
    except ValueError as e:
        print(f"headerflags: {e}", file=sys.stderr)
        sys.exit(3)
