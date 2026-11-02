"""The 486's clocks per instruction form, read from the targets' `timings.times`: the table the compiler prices by is the one the
emulator counts by. A form the table has no row for is in `UNTABLED` below, each with its source, and `test_clocks.py` keeps the two
apart."""
import tomllib
from pathlib import Path

CPU = "486"
CRATES = Path(__file__).resolve().parents[2]
TUNE = CRATES.parents[1] / "machine" / "tune" / "x86"


def _read(path: Path, cpu: str) -> tuple[dict[str, int], dict[str, int]]:
    """(scalars, [clocks]) of `cpu`'s column; a `-` is no price and absent, a `=` is read from the CPU's tune file."""
    scalars: dict[str, int] = {}
    clocks: dict[str, int] = {}
    cpus: list[str] = []
    marches: list[str] = []
    section = "scalars"
    for raw in path.read_text().splitlines():
        line = raw.split("#")[0].strip()
        if not line:
            continue
        if line.startswith("["):
            section = line.strip("[]")
            continue
        name, *values = line.split()
        if name == "cpus":
            cpus = values
            continue
        if name == "march":
            marches = values
            continue
        if section not in ("scalars", "clocks") or not cpus or len(values) != len(cpus):
            continue
        value = values[cpus.index(cpu)]
        if value == "=":
            tune = tomllib.loads((TUNE / f"{marches[cpus.index(cpu)]}.toml").read_text())
            value = str((tune if section == "scalars" else tune[section])[name])
        if value != "-" and value.lstrip("-").isdigit():
            (scalars if section == "scalars" else clocks)[name] = int(value)
    return scalars, clocks


_FLAT = _read(CRATES / "llrm-x86-m32/src/timings.times", CPU)
_REAL = _read(CRATES / "llrm-x86-m16/src/timings.times", CPU)
PREFIX = _FLAT[0]["prefix"]


def clocks(form: str, far: bool = False) -> int:
    """`form`'s clocks. A far form is real mode's table's; any other is flat code's, which the real-mode table also has but for the near
    call and return (a near call is the same instruction in both)."""
    first, second = (_REAL[1], _FLAT[1]) if far else (_FLAT[1], _REAL[1])
    if form in first:
        return first[form]
    return second[form]


def audited(key: str) -> int:
    """A row of the audited bounds (`sdiv_w2`, `udiv_w4`, ...)."""
    return _FLAT[1].get(key, _REAL[1].get(key))


# Forms the tables have no row for: the 486 data sheet's figure (Intel 240440-002, the least of each range, so that no compiler is
# favoured by a price it lacks), by the instruction's operands.
UNTABLED = {
    "shift_cl": 3, "shift_cl_mem": 5, "shift_imm_mem": 4,       # SHL/SHR/SAR/ROL/ROR r,CL; m,CL; m,imm
    "shld_imm": 2, "shld_imm_mem": 3, "shld_cl": 3, "shld_cl_mem": 4,  # SHLD/SHRD r,r,imm; m,r,imm; r,r,CL; m,r,CL (page of 240440-002 to cite)
    "div_b": 16, "idiv_b": 19,                                    # DIV/IDIV r/m8
    "enter": 14, "loads": 2, "scas": 6, "cmps": 7,   # ENTER; SAHF; LODS; SCAS and CMPS per iteration
    "scas_first": 5, "cmps_first": 5,                             # ... and once more for the first
    "fld_reg": 4, "fst_reg": 3, "fchs": 6, "fabs": 3, "fcom": 4, "fnstsw": 3, "fnstcw": 4, "fild": 13, "fist": 29, "fsqrt": 83,
    "fiadd32": 19, "fiadd16": 20, "fimul32": 22, "fimul16": 23, "fidiv32": 84, "fidiv16": 85,
}


def prefixes(raw: bytes) -> int:
    """How many operand-size and address-size prefixes (66h, 67h) lead `raw`: 16-bit code carries 66h on every dword instruction and
    flat code on every word one, and each costs the 486 a clock (`prefix` in timings.times)."""
    count = 0
    for byte in raw[:4]:
        if byte in (0x66, 0x67):
            count += 1
        elif byte not in (0xF2, 0xF3, 0x2E, 0x36, 0x3E, 0x26, 0x64, 0x65):
            break
    return count
