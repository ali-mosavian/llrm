import shutil
import subprocess
from pathlib import Path
from collections import Counter

from iced_x86 import Mnemonic

import corpus
from tools.references import reference


def test_reference_retains_source_order_and_single_rounding(tmp_path: Path) -> None:
    assembler = shutil.which("jwasm") or "/Users/alim/work/other/d32x/toolchains/native/bin/jwasm"
    binary = tmp_path / "reference.bin"
    subprocess.run([assembler, "-bin", "-FPi87", "-q", f"-Fo{binary}", "tools/references/fpcsex.asm"], check=True)
    source = Path("fixtures/omf/fpcsex-p-g2.obj").read_bytes()
    output = reference.built(
        source, binary.read_bytes(), "1dfe04c8578e4152091558338840c77e22f93b1493cda1e0c1d4eb8d84ceaf1f"
    )
    found = corpus.loaded(output)
    assert found is not None
    mapped = corpus.mapped(output)
    assert not isinstance(mapped, str)
    instructions = [one for block in corpus.partitioned(output) for one in block.insns]
    floating = []
    for one in instructions:
        if one.insn.mnemonic in (Mnemonic.FLD, Mnemonic.FADD, Mnemonic.FMUL, Mnemonic.FDIV, Mnemonic.FSTP):
            assert one.disp_at is not None
            address = found.resolve(one.disp_at, 0)
            assert address is not None
            floating.append((one.insn.mnemonic, address.disp))
    assert floating == [
        (Mnemonic.FLD, 6),
        (Mnemonic.FADD, 10),
        (Mnemonic.FMUL, 14),
        (Mnemonic.FSTP, 18),
        (Mnemonic.FLD, 6),
        (Mnemonic.FADD, 10),
        (Mnemonic.FDIV, 14),
        (Mnemonic.FSTP, 22),
        (Mnemonic.FLD, 26),
        (Mnemonic.FADD, 18),
        (Mnemonic.FADD, 22),
        (Mnemonic.FSTP, 26),
    ]
    assert sum(one.insn.mnemonic == Mnemonic.WAIT for one in instructions) == 15
    assert Counter(found.calls.values()) == {"B$RDR4": 3, "B$PSSD": 1, "B$PER4": 1, "B$PESD": 1, "B$CENP": 1}
