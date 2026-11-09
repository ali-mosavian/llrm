"""The one flag set every external bar (gcc, clang, the pin) is compiled with; per-target fields are explicit, never forked.

A distro compiler changes codegen by default: Ubuntu's gcc keeps the frame pointer at -Os (x_fir read 176 B against 166 B
for upstream gcc), gcc -m64 adds endbr64 and a canary, and a distro builds PIE. The bar is the compiler's own code, so every
default that is the distro's is named here, in one place.

  python3 tools/external_flags.py [--target x86|arm64|none] [--reloc=FLAG]... [--gcc-major N] [--hash]

Relocation model is not in the shared list: it is what OUR compiler emits for the target (m32: -fno-pic -fno-pie; m64: ours is
PIE, -fPIE; -fno-pie faked +10.6% on hanoi2). The target's owner passes it with --reloc and asserts it with a relocation-kind probe.
"""

import argparse
import hashlib

COMMON = (
    "-fomit-frame-pointer",
    "-fno-stack-protector",
    "-fno-stack-clash-protection",
    "-U_FORTIFY_SOURCE",
    "-fno-asynchronous-unwind-tables",  # last: build.sh swaps it for the .eh_frame column
)
TARGET = {
    "x86": ("-fcf-protection=none",),  # no endbr
    "arm64": ("-mbranch-protection=none",),  # no PAC/BTI
    "none": (),  # targets with no branch-protection option (m68k, wasm, 6502, z80)
}
ZERO_INIT_PADDING = "-fzero-init-padding-bits=standard"  # gcc 15's upstream default, pinned; gcc < 15 rejects the option


def flags(target, reloc=(), gcc_major=None):
    return (*reloc, *TARGET[target], *COMMON, *((ZERO_INIT_PADDING,) if gcc_major and gcc_major >= 15 else ()))


def config_hash():
    text = repr((COMMON, sorted(TARGET.items()), ZERO_INIT_PADDING))
    return hashlib.sha256(text.encode()).hexdigest()[:12]


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--target", choices=sorted(TARGET), default="x86")
    ap.add_argument("--reloc", action="append", default=[])
    ap.add_argument("--gcc-major", type=int)
    ap.add_argument("--hash", action="store_true")
    args = ap.parse_args()
    print(config_hash() if args.hash else " ".join(flags(args.target, args.reloc, args.gcc_major)))
