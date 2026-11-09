"""Two objects of one source, compared by their code bytes: `objcmp.py BASE.obj HEAD.obj` prints `same` or `changed CODE_DELTA`.

The file's size is not the code: a call to an external adds its name and a fixup to an OMF object (os_execute -O1: file +10 B,
code -3 B, when `rep movs` became `call memcpy`). `sizes.code_size` counts the code segments (ELF: the executable sections), less
alignment padding."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sizes  # noqa: E402


def code_delta(base: Path, head: Path) -> int:
    return sizes.code_size(Path(head)).code - sizes.code_size(Path(base)).code


def compare(base: Path, head: Path) -> tuple[bool, int]:
    """(the files are identical, the code bytes head minus base)."""
    return Path(base).read_bytes() == Path(head).read_bytes(), code_delta(base, head)


if __name__ == "__main__":
    same, delta = compare(Path(sys.argv[1]), Path(sys.argv[2]))
    print("same" if same else f"changed {delta:+d}")
