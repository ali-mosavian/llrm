#!/usr/bin/env python3
"""Ours against the pin's llc on the same IR, per program: instructions and code bytes.

    pindiff.py m32|m16 LEVEL --src DIR --llc LLRM_LLC --work DIR [--jobs N] [--old]

LEVEL is O1, O2, O3 or Os. Each `DIR/*.c` goes through the pinned clang with the flags llrm-cc gives the machine (`crates/driver/llrm-cc/targets.toml`) and
`-march` of the pin's target CPU, to IR with no pass run (as llrm-cc writes it). Ours takes that IR; the pin's side runs its `opt` on it
(Os is O2 with `optsize`; the pin's opt no longer takes Os) and its llc: each compiler's whole pipeline once, from one front end. `--reopt`
instead gives both llcs the pin-optimised IR, which measures the back ends only if ours does not re-optimise it (it does). `irguard` refuses an IR whose `target-cpu` is not the CPU both llcs run with. Ours is `LLRM_LLC` with
`--isel=dag` and the pin's default convention (cdecl at m16, sysv at m32), the pin's llc is `LLVM_BIN`'s.

The table has, per program, the instructions of ours and of the pin's asm (directives, labels and comments not counted) and the code bytes of
ours (the OMF object's code segments, data left out) against the pin's `.text`; with `--old` also ours under `--isel=old`. A row is flagged
where ours is larger in instructions or in bytes. The first line names the llc (md5 and `--version`) and the pin. At m16 the pin picks 32-bit
registers and prefixes: its bytes are longer than any 16-bit selection could be, so read that column as a size bar only.
"""
from __future__ import annotations

import argparse
import hashlib
import os
import re
import subprocess
import sys
import tempfile
import tomllib
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import irguard  # noqa: E402
import omfcode  # noqa: E402
import pin  # noqa: E402

ROOT = HERE.parents[1]
# the comparison's fixed choices: llrm-cc's target, the -march of ours, and the convention the pin's llc has by default
MACHINES = {
    "m16": {"cc": "x86_16-wat", "march": "i386", "abi": "cdecl", "bits": "16"},
    "m32": {"cc": "x86_32-wat", "march": "i486", "abi": "sysv", "bits": "32"},
}
NOT_INSTRUCTIONS = {"align", "db", "dw", "dd", "public", "extern", "proc", "endp", "label", "end", "assume"}


def instructions(text: str) -> int:
    """Instructions in asm text of either dialect: indented lines that are not a directive, a label or a comment."""
    count = 0
    for line in text.splitlines():
        line = re.split(r"[#;]", line)[0].rstrip()
        if not line or not line[0].isspace():
            continue
        word = line.split()[0]
        if word.startswith(".") or word in NOT_INSTRUCTIONS:
            continue
        count += 1
    return count


def cc_entry(machine: str) -> dict:
    return tomllib.loads((ROOT / "crates/driver/llrm-cc/targets.toml").read_text())[MACHINES[machine]["cc"]]


def clang_flags(machine: str) -> tuple[str, list[str]]:
    """The clang binary and flags llrm-cc uses for `machine`, `{root}` and `{env:NAME}` filled in."""
    entry = cc_entry(machine)

    def fill(word: str) -> str:
        word = word.replace("{root}", str(ROOT))
        return re.sub(r"\{env:(\w+)\}", lambda found: os.environ[found.group(1)], word)

    binary = os.environ.get(entry.get("clang_env", ""), "") or os.environ.get("LLRM_PIN_CLANG", "") or str(pin.PINNED_BIN / "clang")
    return binary, [fill(word) for word in entry["clang"]]


def pin_text_size(obj: Path) -> int:
    out = subprocess.run(["size", "-A", str(obj)], capture_output=True, text=True).stdout
    return sum(int(line.split()[1]) for line in out.splitlines() if line.startswith(".text"))


def front_end_flags(level: str, reopt: bool) -> list[str]:
    """What clang is told besides the machine's flags. Raw (default): the front end's IR with no pass run, as llrm-cc writes it, so each
    side runs its whole pipeline once. `reopt`: clang's own `-O` IR, then the pin's opt, then both llcs; ours re-optimises that IR, so it
    measures a different thing (w3, 2026-10-27: the pin's -O3 output is not a fixpoint of its own -O3)."""
    common = ["-fno-vectorize", "-fno-slp-vectorize", "-S", "-emit-llvm"]
    if reopt:
        return [f"-{level}", *common]
    return [f"-{level}", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-O0-optnone", *common]


def one(source: Path, machine: str, level: str, llc: str, work: Path, old: bool, reopt: bool = False) -> dict | str:
    """The figures of one program, or why it has none."""
    cfg = MACHINES[machine]
    target = pin.target(machine, optsize=level == "Os")
    stem = source.stem
    binary, flags = clang_flags(machine)
    raw = work / f"{stem}.raw.ll"
    done = subprocess.run([binary, *flags, f"-march={target.cpu}", *front_end_flags(level, reopt), "-o", str(raw), str(source)], capture_output=True, text=True)
    if done.returncode:
        return "clang: " + (done.stderr.strip().splitlines() or ["failed"])[0]
    text = raw.read_text()
    irguard.check(text, target.cpu, str(raw))
    opted = pin.opt("default<O2>" if level == "Os" else f"default<{level}>", text, target)
    if opted.returncode:
        return "pin opt: " + (opted.stderr.strip().splitlines() or ["failed"])[0]
    ours_text = opted.stdout if reopt else text
    text = opted.stdout
    ours_in = work / f"{stem}.ll"
    ours_text = re.sub(r"^target triple.*\n", "", ours_text, flags=re.M)
    ours_text = re.sub(r"^target datalayout.*$", f'target datalayout = "{cc_entry(machine)["datalayout"]}"', ours_text, flags=re.M)
    ours_in.write_text(ours_text)
    level_flag = "-O2" if level in ("O2", "Os") else f"-{level}"
    pin_asm, pin_obj = work / f"{stem}.pin.s", work / f"{stem}.pin.o"
    for args, out in ((["-x86-asm-syntax=intel", "-filetype=asm"], pin_asm), (["-filetype=obj"], pin_obj)):
        run = pin.llc([level_flag, "-frame-pointer=all", *args, "-o", str(out)], text, target)
        if run.returncode or not out.exists():
            return "pin llc: " + (run.stderr.strip().splitlines() or ["failed"])[0]

    def ours(isel: str, *extra: str) -> subprocess.CompletedProcess:
        return subprocess.run([llc, f"-m{cfg['bits']}", f"-{level}", f"-march={cfg['march']}", f"--isel={isel}", f"-mabi={cfg['abi']}", *extra, str(ours_in)], capture_output=True, text=True)

    asm = work / f"{stem}.ours.s"
    made = ours("dag", "-S", "-o", str(asm))
    if made.returncode:
        return "ours: " + (made.stderr.strip().splitlines() or ["failed"])[0]

    def bytes_of(isel: str) -> int | None:
        obj = Path(tempfile.mktemp(suffix=".obj", dir=work))
        ran = ours(isel, "-o", str(obj))
        return omfcode.code_bytes(str(obj))[0] if ran.returncode == 0 and obj.exists() else None

    return {
        "name": stem,
        "ins": instructions(asm.read_text()),
        "pin_ins": instructions(pin_asm.read_text()),
        "bytes": bytes_of("dag"),
        "pin_bytes": pin_text_size(pin_obj),
        "old_bytes": bytes_of("old") if old else None,
    }


def table(rows: list[dict], old: bool) -> list[str]:
    head = f"{'program':16} {'ins':>5} {'pin':>5} {'d':>5} | {'bytes':>6} {'pin':>6} {'d':>5}" + (f" | {'old':>6} {'d':>5}" if old else "") + "  flag"
    lines = [head]
    flagged = 0
    for row in sorted(rows, key=lambda one: one["name"]):
        flags = []
        if row["ins"] > row["pin_ins"]:
            flags.append("INS>pin")
        if row["bytes"] and row["pin_bytes"] and row["bytes"] > row["pin_bytes"]:
            flags.append("BYTES>pin")
        if old and row["bytes"] and row["old_bytes"] and row["bytes"] > row["old_bytes"]:
            flags.append("BYTES>old")
        flagged += bool(flags)
        text = f"{row['name']:16} {row['ins']:5} {row['pin_ins']:5} {row['ins'] - row['pin_ins']:+5} | {row['bytes'] or -1:6} {row['pin_bytes']:6} {(row['bytes'] or 0) - row['pin_bytes']:+5}"
        if old:
            text += f" | {row['old_bytes'] or -1:6} {((row['bytes'] or 0) - row['old_bytes']) if row['old_bytes'] else 0:+5}"
        lines.append(text + "  " + " ".join(flags))
    lines.append(
        f"programs {len(rows)} flagged {flagged} | ins {sum(r['ins'] for r in rows)} vs {sum(r['pin_ins'] for r in rows)}"
        f" | bytes {sum(r['bytes'] or 0 for r in rows)} vs {sum(r['pin_bytes'] for r in rows)}"
    )
    return lines


def commits_behind(version: str, ref: str, repo: str) -> tuple[str | None, int | None]:
    """The sha `--version` names and how many commits `ref` has beyond it; (None, None) when it names none or the repo lacks it."""
    found = re.search(r"\b[0-9a-f]{40}\b", version)
    if not found:
        return None, None
    counted = subprocess.run(["git", "-C", repo, "rev-list", "--count", f"{found.group(0)}..{ref}"], capture_output=True, text=True)
    return (found.group(0), int(counted.stdout)) if counted.returncode == 0 else (None, None)


def staleness(version: str, ref: str, repo: str, limit: int) -> str:
    sha, behind = commits_behind(version, ref, repo)
    if sha is None:
        return f"# WARNING: the llc's build sha is unknown to {repo}; its distance from {ref} is not known"
    return f"# WARNING: the llc is built on {sha[:9]}, {behind} commits behind {ref} (limit {limit})" if behind > limit else ""


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("machine", choices=sorted(MACHINES))
    parser.add_argument("level", choices=["O1", "O2", "O3", "Os"])
    parser.add_argument("--src", required=True)
    parser.add_argument("--llc", required=True)
    parser.add_argument("--work", required=True)
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--old", action="store_true")
    parser.add_argument("--ref", default="origin/feat/parity", help="the branch the llc should be built on")
    parser.add_argument("--max-behind", type=int, default=20, help="commits behind --ref before the header warns")
    parser.add_argument("--reopt", action="store_true", help="clang's -O IR re-run through the pin's opt, then both llcs (not the default: see front_end_flags)")
    args = parser.parse_args(argv)
    work = Path(args.work)
    work.mkdir(parents=True, exist_ok=True)
    version = subprocess.run([args.llc, "--version"], capture_output=True, text=True).stdout.strip()
    md5 = hashlib.md5(Path(args.llc).read_bytes()).hexdigest()[:12]
    pin_version = subprocess.run([str(pin.PINNED_BIN / "llc"), "--version"], capture_output=True, text=True).stdout.splitlines()[:2]
    print(f"# pindiff {args.machine} -{args.level}: ours {version} md5 {md5}; pin {' '.join(pin_version)}; {pin.header(pin.target(args.machine, args.level == 'Os'))}")
    stale = staleness(version, args.ref, str(Path(__file__).resolve().parents[2]), args.max_behind)
    if stale:
        print(stale)
    sources = sorted(Path(args.src).glob("*.c"))
    with ThreadPoolExecutor(args.jobs) as pool:
        results = list(pool.map(lambda source: one(source, args.machine, args.level, args.llc, work, args.old, args.reopt), sources))
    rows = [r for r in results if isinstance(r, dict)]
    for source, result in zip(sources, results):
        if isinstance(result, str):
            print(f"# no figures for {source.stem}: {result}")
    print("\n".join(table(rows, args.old)))
    return 0 if rows else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
