"""The flat runtime's sources: a target's own code replaces the portable C of the same name.

    uv run --project tools python -m pytest tools/dosbatch/test_qb32_sources.py
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import qb32  # noqa: E402


def test_a_portable_source_the_target_replaces_is_not_built():
    """smid.c (the wasm32 B$SMID) beside dos/m32/smid.asm overwrote the assembly's object: DOS ran the C routine, 5.0M -> 7.4M instructions in jpeg_fixed bq32 -Os."""
    portable = [Path("rt/fixstr.c"), Path("rt/smid.c")]
    own = [Path("rt/dos/m32/smid.asm"), Path("rt/dos/m32/startup.c")]
    built = qb32.runtime_sources(portable, own)
    assert Path("rt/smid.c") not in built
    assert Path("rt/dos/m32/smid.asm") in built
    assert Path("rt/fixstr.c") in built


def test_the_real_runtime_builds_one_source_per_name():
    sources = qb32.runtime_sources(list(qb32.RUNTIME.glob("*.c")), qb32.qbruntime.platform_sources_of(qb32.PLATFORM))
    stems = [source.stem for source in sources]
    assert len(stems) == len(set(stems)), sorted({stem for stem in stems if stems.count(stem) > 1})


def test_the_real_mode_runtime_builds_one_source_per_name():
    """The same collision on the real-mode build (qbruntime.build, qbbuild): one `smid.obj` per name, the target's own."""
    own = qb32.qbruntime.platform_sources()
    sources = [*qb32.qbruntime.portable_not_replaced(qb32.qbruntime.portable_sources(), own), *own]
    stems = [source.stem for source in sources]
    assert len(stems) == len(set(stems)), sorted({stem for stem in stems if stems.count(stem) > 1})
    assert Path(qb32.qbruntime.RUNTIME / "smid.c") not in sources
