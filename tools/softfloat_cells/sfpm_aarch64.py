"""usage: sfpm_aarch64.py LIBGCC_DIR OUT_DIR -> OUT_DIR/sfp-machine.h: GCC's config/aarch64/sfp-machine.h from the fetched tree, edited for the instrument.
The file is read at run time and not copied into this repository (GPLv3). Edits: S and D multiply/divide meats as config/riscv (gcc builds no aarch64 soft S/D),
and FP_INIT_ROUNDMODE without the fpcr read (round to nearest, -mgeneral-regs-only)."""
import re
import sys
from pathlib import Path

src = Path(sys.argv[1]) / "config/aarch64/sfp-machine.h"
if not src.is_file():
    sys.exit(f"sfpm_aarch64: {src} not found: fetch GCC's libgcc (LIBGCC=<dir with config/aarch64/sfp-machine.h>)")
text = src.read_text()
meats = """/* instrument: S and D meats as config/riscv (rv64 block; same 64-bit word) */
#define _FP_MUL_MEAT_S(R,X,Y) _FP_MUL_MEAT_1_imm(_FP_WFRACBITS_S,R,X,Y)
#define _FP_MUL_MEAT_D(R,X,Y) _FP_MUL_MEAT_1_wide(_FP_WFRACBITS_D,R,X,Y,umul_ppmm)
#define _FP_DIV_MEAT_S(R,X,Y) _FP_DIV_MEAT_1_imm(S,R,X,Y,_FP_DIV_HELP_imm)
#define _FP_DIV_MEAT_D(R,X,Y) _FP_DIV_MEAT_1_udiv_norm(D,R,X,Y)
"""
text, n = re.subn(r"(#define _FP_MUL_MEAT_Q)", lambda m: meats + m.group(1), text, count=1)
if n != 1:
    sys.exit("sfpm_aarch64: no _FP_MUL_MEAT_Q to put the S/D meats after")
text, n = re.subn(r"#define FP_INIT_ROUNDMODE\s*\\\n.*?\n  \} while \(0\)", "#define FP_INIT_ROUNDMODE do { } while (0) /* instrument: round to nearest, no fpcr read */", text, count=1, flags=re.S)
if n != 1:
    sys.exit("sfpm_aarch64: FP_INIT_ROUNDMODE not found")
out = Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
(out / "sfp-machine.h").write_text(text)
