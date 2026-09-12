import shutil
import struct
import subprocess
from pathlib import Path
from collections import Counter

import pytest
from iced_x86 import OpKind
from iced_x86 import Mnemonic

import corpus
from tools.references import fpdeep


@pytest.fixture(scope="module")
def reference(tmp_path_factory: pytest.TempPathFactory) -> tuple[bytes, bytes, bytes]:
    assembler = shutil.which("jwasm") or "/Users/alim/work/other/d32x/toolchains/native/bin/jwasm"
    if not Path(assembler).is_file():
        raise FileNotFoundError("JWasm is required to assemble the independent reference")
    binary = tmp_path_factory.mktemp("fpdeep-reference") / "reference.bin"
    subprocess.run([assembler, "-bin", "-FPi87", "-q", f"-Fo{binary}", "tools/references/fpdeep.asm"], check=True)
    source = Path("fixtures/omf/fpdeep-p-g2.obj").read_bytes()
    assembled = binary.read_bytes()
    return source, assembled, fpdeep.built(source, assembled)


def test_reference_retains_numeric_stores_and_observation_points(reference: tuple[bytes, bytes, bytes]) -> None:
    # The numerical-only reference omitted all 21 stores and 21 checkpoints.
    _, _, output = reference
    found = corpus.loaded(output)
    assert found is not None
    mapped = corpus.mapped(output)
    assert not isinstance(mapped, str)
    instructions = [one for block in corpus.partitioned(output) for one in block.insns]
    assert sum(one.insn.mnemonic == Mnemonic.WAIT for one in instructions) == 21
    stores = [one for one in instructions if one.insn.mnemonic == Mnemonic.MOV and one.insn.op0_kind == OpKind.MEMORY]
    assert len(stores) == 21
    values = []
    for one in stores:
        assert one.disp_at is not None
        values.append((found.resolve(one.disp_at, 0).disp, one.insn.immediate(1)))
    assert [value for offset, value in values if offset == 42] == [1, 2, 3, 4]
    expected = [144, 6, 0.5, 784, 14, 0.75, 3600, 30, 0.875]
    assert [value for offset, value in values if offset == 18] == [
        struct.unpack("<I", struct.pack("<f", value))[0] for value in expected
    ]
    assert values[-4:] == [(26, 0), (30, 0x40280000), (34, 0), (38, 0x40180000)]
    assert Counter(found.calls.values()) == {"B$PSSD": 20, "B$PSI2": 9, "B$PEI4": 11, "B$PESD": 1, "B$CEND": 1}


def test_reference_refuses_an_unaudited_input(reference: tuple[bytes, bytes, bytes]) -> None:
    source, assembled, _ = reference
    with pytest.raises(ValueError, match="audited PDS"):
        fpdeep.built(source + b"changed", assembled)


def test_reference_requires_its_relocation_manifest(reference: tuple[bytes, bytes, bytes]) -> None:
    source, assembled, _ = reference
    with pytest.raises(ValueError, match="manifest"):
        fpdeep.built(source, assembled[:-4])
