import shutil
import struct
import hashlib
import argparse
import subprocess
from pathlib import Path

from qbopt.objectfile import omf
from qbopt.objectfile import module
from qbopt.objectfile import relocate


def built(source: bytes, assembled: bytes, expected_digest: str) -> bytes:
    if hashlib.sha256(source).hexdigest() != expected_digest:
        raise ValueError("the reference requires the audited PDS FPDEEP object")
    if assembled[-4:] != b"QREF":
        raise ValueError("missing reference relocation manifest")
    count = struct.unpack("<I", assembled[-8:-4])[0]
    split = len(assembled) - 8 - 12 * count
    if split <= 0:
        raise ValueError("invalid reference relocation manifest")
    records = omf.parse(source)
    found = module.of(records)
    assert found is not None
    fixups = {fixup.offset: fixup for fixup in omf.fixups(records) if fixup.seg == found.seg}
    group = list(omf.groups(records)).index("DGROUP") + 1
    relocations: dict[int, list[int]] = {}
    added: list[tuple[int, omf.Fixup]] = []
    for new, old, delta in struct.iter_unpack("<III", assembled[split:-8]):
        fixup = fixups[old]
        if not 0x30 <= new < split + 0x30:
            raise ValueError("relocation is outside reference code")
        if delta:
            if fixup.loc != omf.LOC_OFF16 or fixup.target != "segment":
                raise ValueError("only data offsets may have an added displacement")
            added.append((new, omf.offset_fixup(found.seg, new, fixup.target, fixup.index, fixup.disp + delta, group)))
        else:
            relocations.setdefault(old, []).append(new)
    image = found.code[:0x30] + assembled[:split]
    written = relocate.as_records(
        records,
        found.seg,
        0x30,
        image,
        {0x30: 0x30},
        {old: tuple(destinations) for old, destinations in relocations.items()},
        frozenset(offset for offset in fixups if offset >= 0x30 and offset not in relocations),
        tuple(added),
    )
    if isinstance(written, str):
        raise ValueError(written)
    return b"".join(record.emit() for record in written)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    parser.add_argument("--program", choices=("fpdeep", "fpcsex"), default="fpdeep")
    parser.add_argument(
        "--assembler", default=shutil.which("jwasm") or "/Users/alim/work/other/d32x/toolchains/native/bin/jwasm"
    )
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    assembly = Path(__file__).with_name(f"{args.program}.asm")
    binary = args.output / "reference.bin"
    subprocess.run([args.assembler, "-bin", "-FPi87", "-q", f"-Fo{binary}", str(assembly)], check=True)
    source = Path(f"fixtures/omf/{args.program}-p-g2.obj").read_bytes()
    (args.output / "BASE.OBJ").write_bytes(source)
    digest = {
        "fpdeep": "af15b6dc1332353f4bb35130f55b9de9c1b23aa33b71ebc53009a92446c22aa5",
        "fpcsex": "1dfe04c8578e4152091558338840c77e22f93b1493cda1e0c1d4eb8d84ceaf1f",
    }[args.program]
    result = built(source, binary.read_bytes(), digest)
    (args.output / f"{args.program}-p-g2.obj").write_bytes(result)
    (args.output / "REF.OBJ").write_bytes(result)


if __name__ == "__main__":
    main()
