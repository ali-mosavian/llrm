"""How every d_faces.c compile reads the file: the wrapper, the include path and llrm's m32 flags, in one place (run.py and tools/pass_budgets.py)."""
from pathlib import Path

# The Borland-era keywords mean nothing flat; every compiler reads the file with the same convention and the same sizes.
KEYWORDS = "far huge near pascal cdecl __far _far __near _near"
LLRM_FLAGS = ["--target=x86_32-wat", "-march=i486", "-mabi=sysv"]  # llrm-cc takes its target as a row; sysv is the convention gcc uses


def includes(qcport: Path) -> list[str]:
    return [flag for d in ("", "render", "game", "host", "qgl") for flag in ("-I", str(qcport / d))]


def write_source(d_faces: Path, path: Path) -> Path:
    """d_faces.c with its keywords of the segmented models emptied, so that all compilers take the flat model and one calling convention."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(f"#define {word}\n" for word in KEYWORDS.split()) + f'#include "{d_faces}"\n')
    return path


def llrm_command(llrm, level: str, qcport: Path, source: Path, obj) -> list[str]:
    return [str(llrm), *LLRM_FLAGS, f"-{level}", "-c", *includes(qcport), "-o", str(obj), str(source)]
