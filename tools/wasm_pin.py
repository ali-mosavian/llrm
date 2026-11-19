"""The pin side of the wasm cells: the pin's clang with the shared external flag set (tools/external_flags.py), never bare."""
import sys

sys.path.insert(0, __file__.rsplit("/", 1)[0])
import external_flags  # noqa: E402

INCLUDES = ("-I", "bench", "-I", "runtime/shared/posix", "-I", "tests/run/c")


def pin_command(clang, source, out, level, cell_flags=()):
    """clang argv for one pin cell; `level` is `O2`, `Os`, ..."""
    return [clang, "--target=wasm32", "-ffreestanding", *external_flags.flags("none"), *cell_flags, f"-{level}", *INCLUDES, "-c", source, "-o", out]


def header():
    return f"# external_flags {external_flags.config_hash()}"


if __name__ == "__main__":
    print(header() if sys.argv[1:] == ["header"] else " ".join(external_flags.flags("none")))
