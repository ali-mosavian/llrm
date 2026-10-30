#!/usr/bin/env python3
"""The one way a script runs the pin's `opt` or `llc` on llrm's IR.

    pin.py opt -m 32 --passes 'default<O2>' FILE.ll      # prints `target: ...`, then opt's output
    pin.py llc -m 32 FILE.ll

A module from llrm's dumps names no triple, CPU or features, so the pin's tools take the host's (x86-64, SSE): another inliner and TTI cost
model, another SLP, another datalayout (w-4 and w-5 each took numbers that way). Every run here is given a `Target` (`target("m32")`,
read from the target description: triple, CPU and datalayout; features and `optsize` stated here) and refuses without one; the module is
written to carry it (triple, datalayout where it has none, `target-cpu`, `target-features` and `optsize` on every function) and the
`target:` line says what was run. A number taken without the line is not quoted.
"""
from __future__ import annotations

import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
# Where `ld` places the code of an ELF object linked against the pin's stub (the stub sits below it): any address above the stub.
LINK_TEXT_BASE = 1 << 16
sys.path.insert(0, str(HERE.parent))
import targetdesc  # noqa: E402

PINNED_BIN = Path(os.environ.get("LLVM_BIN") or Path.home() / "scratch/llvm-build/bin")  # LLVM d1106deb71cc
FEATURES = {"x86": "-sse,-sse2,-mmx"}  # what an i486 has not: stated, so no default of the tool's can add it


class Refused(Exception):
    """A run asked for with no target, or a target missing a field."""


@dataclass(frozen=True)
class Target:
    triple: str
    cpu: str
    datalayout: str
    features: str
    optsize: bool = False

    def __post_init__(self):
        for field in ("triple", "cpu", "datalayout", "features"):
            if not getattr(self, field):
                raise Refused(f"target has no {field}")


def target(mode: str, optsize: bool = False) -> Target:
    """llrm's `m<mode>` as the pin's tools are told it: the target description's triple, CPU and datalayout."""
    found = targetdesc.targets().get(mode)
    if found is None:
        raise Refused(f"no target `{mode}` in the target description: {sorted(targetdesc.targets())}")
    arch = found.get("tti", "X86").lower()
    return Target(found["llvm_triple"], found["llvm_cpu"], found["datalayout"], FEATURES.get(arch, ""), optsize)


def header(of: Target) -> str:
    return f"target: triple={of.triple} cpu={of.cpu} features={of.features} datalayout={of.datalayout} optsize={str(of.optsize).lower()}"


def stack_alignment(of: Target) -> int | None:
    """The stack alignment in bytes the target's datalayout states (`S32` is 4)."""
    found = re.search(r"-S(\d+)\b", "-" + of.datalayout)
    return int(found.group(1)) // 8 if found else None


def _with_stack_alignment(text: str, of: Target) -> str:
    """The module told our target's stack alignment (`override-stack-alignment`, as clang's -mstack-alignment): the pin takes 16 from a
    Linux triple whatever the datalayout says, and with 16 it keeps a reserved call frame where, at 4, it pushes (pair csr, 2026-10-25:
    push + add esp counted 197 + 206 against the pin's 71 + 146, none of it ours to fix)."""
    align = stack_alignment(of)
    if align is None or "override-stack-alignment" in text:
        return text
    node = f'!9999 = !{{i32 1, !"override-stack-alignment", i32 {align}}}'
    flags = re.search(r"^!llvm\.module\.flags = !\{([^}]*)\}$", text, re.M)
    if flags:
        joined = flags.group(1).strip()
        text = text.replace(flags.group(0), "!llvm.module.flags = !{" + (joined + ", " if joined else "") + "!9999}", 1)
        return text + "\n" + node + "\n"
    return text + "\n!llvm.module.flags = !{!9999}\n" + node + "\n"


def retarget(text: str, of: Target) -> str:
    """`text` with the target written into the module: the triple, the datalayout when it has none, and one attribute group on every
    function; a module that already names a triple or a layout keeps its own (it is the claim the dump made). llrm's own words
    (`threeway`, `sysvcc`, `declare internal`) are written in the dialect LLVM reads first."""
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "difftest"))
    from difftest import normalise  # noqa: PLC0415 (difftest guards its environment on import)

    text = normalise(text)
    if not re.search(r"^target datalayout", text, re.M):
        text = f'target datalayout = "{of.datalayout}"\n{text}'
    if not re.search(r"^target triple", text, re.M):
        text = re.sub(r"^(target datalayout = .*)$", rf'\1\ntarget triple = "{of.triple}"', text, count=1, flags=re.M)
    text = re.sub(r"^(define [^\n]*?) \{$", r"\1 #9999 {", text, flags=re.M)
    # An intrinsic lives in address space 0 in LLVM; ours declares and calls it in the code space (m16 `addrspace(1)` after the
    # parameters of a declare and after `call cc N`). A pointer parameter in that space stays.
    text = re.sub(r"^(declare [^\n]*@llvm\.[^\n]*?\)) addrspace\(1\)(?= [a-z]|$)", r"\1", text, flags=re.M)
    text = re.sub(r"^(\s+(?:%\S+ = )?(?:(?:tail|musttail|notail) )?call(?: cc \d+)?) addrspace\(1\)( [^\n]*@llvm\.)", r"\1\2", text, flags=re.M)
    text = _with_stack_alignment(text, of)
    group = f'"target-cpu"="{of.cpu}" "target-features"="{of.features}"' + (" optsize" if of.optsize else "")
    return text + f"\nattributes #9999 = {{ {group} }}\n"


def _tool(name: str, bin_dir: Path | None) -> str:
    return str((bin_dir or PINNED_BIN) / name)


def _need(of) -> Target:
    if not isinstance(of, Target):
        raise Refused("the pin's tools run only with an explicit Target: pin.target('m32')")
    return of


def opt(passes: str, text: str, of: Target | None = None, extra: list[str] | None = None, bin_dir: Path | None = None) -> subprocess.CompletedProcess:
    """`opt -passes=PASSES -S` over `text` retargeted to `of`; `stdout` starts with the `target:` line as a comment."""
    of = _need(of)
    done = subprocess.run([_tool("opt", bin_dir), f"-passes={passes}", "-S", *(extra or []), "-"], input=retarget(text, of), capture_output=True, text=True)
    done.stdout = f"; {header(of)}\n" + done.stdout
    return done


def backend_ir(dump: Path) -> Path | None:
    """The IR a `--dump` hands to the backend: the `switchconv` stage, whatever number the pipeline gives it."""
    found = sorted(dump.glob("*-switchconv.ll"))
    return found[-1] if found else None


def llc(args: list[str], text: str, of: Target | None = None, bin_dir: Path | None = None) -> subprocess.CompletedProcess:
    """`llc` over `text` retargeted to `of`, told `-mtriple`, `-mcpu` and `-mattr` besides."""
    of = _need(of)
    cmd = [_tool("llc", bin_dir), f"-mtriple={of.triple}", f"-mcpu={of.cpu}", f"-mattr={of.features}", *args, "-"]
    done = subprocess.run(cmd, input=retarget(text, of), capture_output=True, text=True)
    done.stdout = f"; {header(of)}\n" + done.stdout
    return done


def main(argv: list[str]) -> int:
    if len(argv) < 2 or argv[0] not in ("opt", "llc") or "-m" not in argv:
        print(__doc__)
        return 2
    mode = "m" + argv[argv.index("-m") + 1]
    of = target(mode, optsize="--optsize" in argv)
    path = argv[-1]
    passes = argv[argv.index("--passes") + 1] if "--passes" in argv else "default<O2>"
    text = Path(path).read_text()
    done = opt(passes, text, of) if argv[0] == "opt" else llc(["-O2", "-filetype=asm", "-o", "-"], text, of)
    sys.stdout.write(done.stdout)
    sys.stderr.write(done.stderr)
    return done.returncode


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
