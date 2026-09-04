"""
The configurations BC is checked under.

Only the codegen pair (/G2 vs /G3) and event polling (/V /W) change what the
pass sees; /O, /Ot and /FPi are here because a switch that does not change the
long shapes today is exactly the kind of thing that changes them tomorrow. The
tags are the predecessor's, so docs/inherited-plan.md still names the same
things.

Which switches each compiler accepts was established by feeding every one of
them to every compiler and reading the rejection -- QB 4.5 says "Option
unknown: /Q" where the other two say "Ignored unknown command line option", so
reading for the wrong string reports every switch as accepted.
"""

from pathlib import Path
from dataclasses import field
from dataclasses import dataclass

VBDOS = Path.home() / "work/other/d32x/toolchains/vbdos"
PDS71 = Path.home() / "work/other/d32x/toolchains/pds71"
QB45 = Path.home() / "work/42-labs/mini-qb/dosbox/qb45"


@dataclass(frozen=True, slots=True)
class Config:
    tag: str
    mount: Path
    bc: str
    link: str
    runtime: str
    switches: str
    # a program that overflows on purpose cannot run under /D
    overflow_checked: bool = field(default=False)

    @property
    def available(self) -> bool:
        return self.mount.is_dir()


def _vbdos(tag: str, switches: str) -> Config:
    return Config(tag, VBDOS, r"V:\BIN\BC.EXE", r"V:\BIN\LINK.EXE", r"V:\LIB\VBDCL10E.LIB", switches)


def _pds(tag: str, switches: str) -> Config:
    return Config(tag, PDS71, r"V:\BINB\BC.EXE", r"V:\BINB\LINK.EXE", r"V:\LIB\BCL71ENR.LIB", switches)


def _qb45(tag: str, switches: str) -> Config:
    return Config(tag, QB45, r"V:\BC.EXE", r"V:\LINK.EXE", r"V:\LIB\BCOM45.LIB", switches)


# /Zi on every one of them. It makes BC populate $$SYMBOLS and $$TYPES --
# the names and types of every variable, every procedure's signature, and
# each parameter and local with its own bp offset -- which is the only
# source in the object for what anything is called. Measured on six
# programs: the generated code is byte-identical, and the object gains the
# two debug segments and two bytes of padding.
CONFIGS = {
    c.tag: c
    for c in (
        # VBDOS is the only one that takes /G3, the one dword argument push
        _vbdos("v-g3", "/O /FPi /R /G3 /E /Zi"),
        _vbdos("v-g2", "/O /FPi /R /G2 /E /Zi"),
        _vbdos("v-noO", "/FPi /R /G3 /E /Zi"),
        _vbdos("v-plain", "/FPi /R /E /Zi"),
        _vbdos("v-evt", "/O /FPi /R /G3 /E /V /W /Zi"),
        # PDS rejects /G3
        _pds("p-g2", "/O /FPi /G2 /Zi"),
        _pds("p-ot", "/O /FPi /G2 /Ot /Zi"),
        _pds("p-noO", "/FPi /G2 /Zi"),
        _pds("p-evt", "/O /FPi /G2 /V /W /Zi"),
        # QB 4.5 rejects /G2 as well, and has no codegen switch at all
        _qb45("q-O", "/O /FPi /Zi"),
        _qb45("q-noO", "/FPi /Zi"),
        _qb45("q-evt", "/O /FPi /V /W /Zi"),
    )
}

# Switches a particular program needs, on top of the configuration's own. /X is
# what lets an error handler RESUME, and all three compilers take it.
EXTRA = {"divmod": "/X"}


def switches_for(config: Config, program: str) -> str:
    return f"{config.switches} {EXTRA[program]}".strip() if program in EXTRA else config.switches


# Programs whose rewritten form is meant to disagree with what BC built. For a
# name listed here e2e.judge() stops comparing the rewritten run against BC's
# own and compares it against the golden instead, which is the only thing worth
# judging either against once the two are known to differ on purpose.
#
# suite/cmpof.bas is here because BC is wrong and the rewrite is right. B$CPI4
# rebuilds a signed answer out of unsigned flags through sahf, which cannot
# write OF -- so BC's own jl/jle/jg/jge read a flag left over from an unrelated
# 16-bit compare and answer backwards whenever the high words are equal and the
# low words straddle 0x8000. That is two consecutive integers, not an exotic
# pair. calls.py absorbs the call into one real 32-bit cmp, which has no such
# problem. See the program's own header for the instruction sequence.
#
# Divide is not here: a bare idiv changes which inputs fault, not what a
# non-faulting one answers.
DIVERGES: set[str] = {"cmpof"}

TAGS = list(CONFIGS)
