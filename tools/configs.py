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


CONFIGS = {
    c.tag: c
    for c in (
        # VBDOS is the only one that takes /G3, the one dword argument push
        _vbdos("v-g3", "/O /FPi /R /G3 /E"),
        _vbdos("v-g2", "/O /FPi /R /G2 /E"),
        _vbdos("v-noO", "/FPi /R /G3 /E"),
        _vbdos("v-plain", "/FPi /R /E"),
        _vbdos("v-evt", "/O /FPi /R /G3 /E /V /W"),
        # PDS rejects /G3
        _pds("p-g2", "/O /FPi /G2"),
        _pds("p-ot", "/O /FPi /G2 /Ot"),
        _pds("p-noO", "/FPi /G2"),
        _pds("p-evt", "/O /FPi /G2 /V /W"),
        # QB 4.5 rejects /G2 as well, and has no codegen switch at all
        _qb45("q-O", "/O /FPi"),
        _qb45("q-noO", "/FPi"),
        _qb45("q-evt", "/O /FPi /V /W"),
    )
}

# Switches a particular program needs, on top of the configuration's own. /X is
# what lets an error handler RESUME, and all three compilers take it.
EXTRA = {"divmod": "/X", "ctrap": "/X"}


def switches_for(config: Config, program: str) -> str:
    return f"{config.switches} {EXTRA[program]}".strip() if program in EXTRA else config.switches


# Programs whose rewritten form is meant to disagree with what BC built. There
# is one, and it is where the divide's C semantics are pinned: the baseline
# raises a BASIC error on a zero divisor and qbopt's divide does not fault at
# all. These are checked against the golden alone.
DIVERGES = {"ctrap"}

TAGS = list(CONFIGS)
