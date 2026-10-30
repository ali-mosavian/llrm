#!/usr/bin/env python3
"""Ours against the pin's llc on the same IR, per program: instructions and code bytes.

    pindiff.py m32|m16 LEVEL [--src DIR] --llc LLRM_LLC --work DIR [--jobs N] [--old]

LEVEL is O1, O2, O3 or Os. Each `DIR/*.c` goes through the pinned clang with the flags llrm-cc gives the machine (`crates/driver/llrm-cc/targets.toml`) and
`-march` of the pin's target CPU, to IR with no pass run (as llrm-cc writes it). Ours takes that IR; the pin's side runs its `opt` on it
(Os is O2 with `optsize`; the pin's opt no longer takes Os) and its llc: each compiler's whole pipeline once, from one front end. `--reopt`
instead gives both llcs the pin-optimised IR, which measures the back ends only if ours does not re-optimise it (it does). `irguard` refuses an IR whose `target-cpu` is not the CPU both llcs run with. Ours is `LLRM_LLC` with
`--isel=dag` and the pin's default convention (cdecl at m16, sysv at m32), the pin's llc is `LLVM_BIN`'s.

The table has, per program, the instructions of ours and of the pin's asm (directives, labels and comments not counted; the pin's include the libgcc i64 divide members its object calls, which ours embeds: rows say `+libgcc[..]`, none at m16) and the code bytes of
ours (the OMF object's code segments, data left out) against the pin's `.text`; with `--old` also ours under `--isel=old`. A row is flagged
where ours is larger in instructions or in bytes without nops (`nopad`: alignment padding moves a small row in steps of 16). The first line names the llc (md5 and `--version`) and the pin. At m16 the pin picks 32-bit
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

BRANCH = os.environ.get("LAND_BRANCH", "release/next")  # the integration branch (tools/landing/branch.sh)

ROOT = HERE.parents[1]


def abi_flags(cfg: dict, ir: str) -> list[str]:
    """The llc's convention flags: an IR whose functions are regcall (llrm-cc's m32 default) needs the Watcom ABI that gives it registers."""
    return ["-mabi=watcom", "-mregcall-default"] if "x86_regcallcc" in ir else [f"-mabi={cfg['abi']}"]


# the comparison's fixed choices: llrm-cc's target, the -march of ours, and the convention the pin's llc has by default
MACHINES = {
    "m16": {"cc": "x86_16-wat", "march": "i386", "abi": "cdecl", "bits": "16", "libgcc": False},
    "m32": {"cc": "x86_32-wat", "march": "i486", "abi": "sysv", "bits": "32", "libgcc": True},
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
    entry = tomllib.loads((ROOT / "crates/driver/llrm-cc/targets.toml").read_text())[MACHINES[machine]["cc"]]
    # `{datalayout:NAME}` stands for that target's datalayout.toml string, as llrm-cc expands it.
    entry["datalayout"] = re.sub(
        r"\{datalayout:([\w-]+)\}",
        lambda found: tomllib.loads((ROOT / f"crates/target/llrm-{found.group(1)}/src/machines/datalayout.toml").read_text())["datalayout"],
        entry["datalayout"],
    )
    return entry


def clang_flags(machine: str) -> tuple[str, list[str]]:
    """The clang binary and flags llrm-cc uses for `machine`, `{root}` and `{env:NAME}` filled in."""
    entry = cc_entry(machine)

    def fill(word: str) -> str:
        word = word.replace("{root}", str(ROOT))
        return re.sub(r"\{env:(\w+)\}", lambda found: os.environ[found.group(1)], word)

    binary = os.environ.get(entry.get("clang_env", ""), "") or os.environ.get("LLRM_PIN_CLANG", "") or str(pin.PINNED_BIN / "clang")
    words = [fill(word) for word in entry["clang"]]
    # llrm-cc's m32 default is regcall; ours runs with the pin's (sysv), so the IR must not mark every function regcall.
    for at in range(len(words) - 1):
        if words[at:at + 2] == ["-Xclang", "-fdefault-calling-conv=regcall"]:
            del words[at:at + 2]
            break
    return binary, words


def pin_text_size(obj: Path) -> int:
    out = subprocess.run(["size", "-A", str(obj)], capture_output=True, text=True).stdout
    return sum(int(line.split()[1]) for line in out.splitlines() if line.startswith(".text"))


NOP = re.compile(r"^(nop\w*|data16|cs nop\w*|xchg\s+ax,ax|xchg\s+eax,eax)\b")


def nop_bytes(listing: str) -> int:
    """Bytes of the nops in an `objdump -d` listing, continuation lines of a long nop included: alignment padding, never code the compilers chose."""
    total, inside = 0, False
    for line in listing.splitlines():
        found = re.match(r"\s*[0-9a-f]+:\t((?:[0-9a-f]{2} )+)\s*(.*)", line)
        if not found:
            continue
        text = found.group(2).strip()
        if text:
            inside = bool(NOP.match(text))
        if inside:
            total += len(found.group(1).split())
    return total


def objdump_mode(bits: int) -> list[str]:
    return ["-M", "intel,i8086" if bits == 16 else "intel"]


def padding_free(blob: bytes, bits: int) -> int:
    """Code bytes of a raw blob without its nops. Decoded, since 0x90 is also a displacement or immediate byte."""
    with tempfile.NamedTemporaryFile(suffix=".bin") as handle:
        handle.write(blob)
        handle.flush()
        listing = subprocess.run(["objdump", "-D", "-b", "binary", "-m", "i386", *objdump_mode(bits), handle.name], capture_output=True, text=True).stdout
    return len(blob) - nop_bytes(listing)


def pin_padding_free(obj: Path, bits: int) -> int:
    """The pin's `.text` without its nops."""
    listing = subprocess.run(["objdump", "-d", *objdump_mode(bits), str(obj)], capture_output=True, text=True).stdout
    return pin_text_size(obj) - nop_bytes(listing)


I64_DIVIDE = {"__divdi3", "__udivdi3", "__moddi3", "__umoddi3", "__divmoddi4", "__udivmoddi4"}


def archive_members(archive: Path) -> dict[str, str]:
    """Symbol -> the archive member defining it."""
    out = subprocess.run(["nm", "-g", "--defined-only", str(archive)], capture_output=True, text=True).stdout
    owner, defined = "", {}
    for line in out.splitlines():
        if line.endswith(":") and ".o" in line:
            owner = line[:-1]
        elif len(parts := line.split()) == 3 and owner:
            defined[parts[2]] = owner
    return defined


def linked_members(wanted: set[str], defined: dict[str, str], undefined_of) -> list[str]:
    """The archive members a linker pulls for `wanted`: those defining a symbol, and those their own undefined symbols need, each once."""
    pulled, queue = [], sorted(wanted)
    while queue:
        member = defined.get(queue.pop())
        if member and member not in pulled:
            pulled.append(member)
            queue.extend(sorted(undefined_of(member)))
    return pulled


def nm_undefined(obj: Path) -> set[str]:
    return set(subprocess.run(["nm", "-u", str(obj)], capture_output=True, text=True).stdout.split()[1::2])


def libgcc_cost(obj: Path, bits: int, work: Path, has_libgcc: bool) -> tuple[int, int, int, list[str]]:
    """Instructions, bytes and nop-free bytes of the libgcc members the pin's object calls (the i64 divide family): ours embeds those
    helpers (SD231/232), the pin's object only calls them, so a row that compared the two measured the link, not the compiler.
    None are counted at m16: there is no 16-bit libgcc, so the pin's calls stay unresolved there."""
    found = subprocess.run(["gcc", "-m32", "-print-libgcc-file-name"], capture_output=True, text=True).stdout.strip()
    if not has_libgcc or not found or not Path(found).exists():
        return 0, 0, 0, sorted(s for s in nm_undefined(obj) if s in I64_DIVIDE) if not has_libgcc else []
    archive, defined = Path(found), archive_members(Path(found))
    extracted: dict[str, Path] = {}

    def member_obj(member: str) -> Path:
        if member not in extracted:
            extracted[member] = work / f"libgcc.{member}"
            extracted[member].write_bytes(subprocess.run(["ar", "p", str(archive), member], capture_output=True).stdout)
        return extracted[member]

    names = linked_members({s for s in nm_undefined(obj) if s.startswith("__")}, defined, lambda member: nm_undefined(member_obj(member)))
    ins = total = free = 0
    for member in names:
        listing = subprocess.run(["objdump", "-d", "--no-show-raw-insn", *objdump_mode(bits), str(member_obj(member))], capture_output=True, text=True).stdout
        ins += sum(1 for line in listing.splitlines() if re.match(r"\s+[0-9a-f]+:\t(?!nop|data16|cs nop|xchg\s+ax,ax)\S", line))
        total += pin_text_size(member_obj(member))
        free += pin_padding_free(member_obj(member), bits)
    return ins, total, free, names


def corpus_sources(into: Path) -> Path:
    """The comparison's programs, `vsgcc/programs.py`'s (bench/ and the x_ kernels, written with u32/i32 for m16), copied flat into `into`."""
    sys.path.insert(0, str(next((ROOT / "crates/target").glob("*/vsgcc"))))
    import programs

    into.mkdir(parents=True, exist_ok=True)
    for name, path in programs.sources().items():
        (into / f"{name}.c").write_text(path.read_text())
    return into


def failure_summary(results: dict[str, dict | str], old: bool) -> str:
    """One line counting the rows that did not compile, per side, so a compiler crash is not one `no figures` line among the pin's and
    clang's: ours (dag; panics counted), ours-old (the old selector failed where dag did not), ours-obj (dag's object), pin, clang, source."""
    groups: dict[str, list[str]] = {"ours": [], "ours-old": [], "ours-obj": [], "pin": [], "clang": [], "source": []}
    panics = 0
    for name, result in results.items():
        if isinstance(result, str):
            side = "ours" if result.startswith("ours") else "clang" if result.startswith("clang") else "source" if result.startswith("undefined shift") else "pin"
            groups[side].append(name)
            panics += side == "ours" and "panicked" in result
        elif result["bytes"] is None:
            groups["ours-obj"].append(name)
        elif old and result["old_bytes"] is None:
            groups["ours-old"].append(name)
    parts = [f"{side} {len(names)} ({', '.join(sorted(names))})" + (f" [{panics} panic]" if side == "ours" and panics else "") for side, names in groups.items() if names]
    return "# FAILED " + ("; ".join(parts) if parts else "none")


def layout_note(wanted: str, used: str) -> str:
    """A header line when the pin's llc runs with another pointer size than the target's datalayout states."""

    def pointer(layout: str) -> str:
        found = re.search(r"(?:^|-)(p:\d+:\d+)", layout)
        return found.group(1) if found else "p:64:64"

    if pointer(wanted) == pointer(used):
        return ""
    return f"# NOTE: the pin's llc replaces the module datalayout (it takes its triple's): pointers are {pointer(used)} there, {pointer(wanted)} in ours and in the pin's opt"


def pin_llc_datalayout(of: pin.Target) -> str:
    """The datalayout the pin's llc really runs with, read from the IR it prints after CodeGenPrepare."""
    ran = pin.llc(["-O1", "-stop-after=codegenprepare", "-o", "-"], "define void @f() {\n  ret void\n}\n", of)
    found = re.search(r'target datalayout = "([^"]*)"', ran.stdout)
    return found.group(1) if found else of.datalayout


def front_end_flags(level: str, reopt: bool) -> list[str]:
    """What clang is told besides the machine's flags. Raw (default): the front end's IR with no pass run, as llrm-cc writes it, so each
    side runs its whole pipeline once. `reopt`: clang's own `-O` IR, then the pin's opt, then both llcs; ours re-optimises that IR, so it
    measures a different thing (w3, 2026-10-27: the pin's -O3 output is not a fixpoint of its own -O3)."""
    common = ["-fno-builtin", "-fno-vectorize", "-fno-slp-vectorize", "-S", "-emit-llvm"]
    if reopt:
        return [f"-{level}", *common]
    return [f"-{level}", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-O0-optnone", *common]


def forced_frame(asm: str, ir: str) -> bool:
    """`push ebp; mov ebp, esp` in the pin's asm for IR that does not ask for one (frame-pointer=all made hanoi2/queens -Os look 4 bytes bigger). Stack realignment (`and esp, -N`) needs one."""
    return bool(re.search(r"push\s+ebp\n\s+mov\s+ebp, esp", asm)) and not re.search(r"and\s+esp, -\d+", asm) and '"frame-pointer"' not in ir and "alloca i8, i32 %" not in ir and "dynamic_alloca" not in ir


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
    if shifts := irguard.undefined_shifts(text):
        return f"undefined shift in the source: {shifts[0]}"
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
        run = pin.llc([level_flag, "-frame-pointer=none", *args, "-o", str(out)], text, target)
        if run.returncode or not out.exists():
            return "pin llc: " + (run.stderr.strip().splitlines() or ["failed"])[0]
    if forced_frame(pin_asm.read_text(), text):
        return "pin asm keeps a frame pointer the source did not force"

    def ours(isel: str, *extra: str) -> subprocess.CompletedProcess:
        return subprocess.run([llc, f"-m{cfg['bits']}", f"-{level}", f"-march={cfg['march']}", f"--isel={isel}", *abi_flags(cfg, ours_text), *extra, str(ours_in)], capture_output=True, text=True)

    asm = work / f"{stem}.ours.s"
    made = ours("dag", "-S", "-o", str(asm))
    if made.returncode:
        return "ours: " + (made.stderr.strip().splitlines() or ["failed"])[0]

    def bytes_of(isel: str) -> tuple[int, int] | None:
        """Code bytes of the object, and the same without its nops."""
        obj = Path(tempfile.mktemp(suffix=".obj", dir=work))
        ran = ours(isel, "-o", str(obj))
        if ran.returncode or not obj.exists():
            return None
        data = obj.read_bytes()
        return omfcode.code_bytes_of(data)[0], padding_free(omfcode.code_blob_of(data), int(cfg["bits"]))

    ours_bytes = bytes_of("dag")
    old_bytes = bytes_of("old") if old else None
    lib_ins, lib_bytes, lib_free, lib_names = libgcc_cost(pin_obj, int(cfg["bits"]), work, cfg["libgcc"])
    return {
        "name": stem,
        "ins": instructions(asm.read_text()),
        "pin_ins": instructions(pin_asm.read_text()) + lib_ins,
        "lib": lib_names if cfg["libgcc"] else [],
        "nolib": lib_names if not cfg["libgcc"] else [],
        "bytes": ours_bytes and ours_bytes[0],
        "free": ours_bytes and ours_bytes[1],
        "pin_bytes": pin_text_size(pin_obj) + lib_bytes,
        "pin_free": pin_padding_free(pin_obj, int(cfg["bits"])) + lib_free,
        "old_bytes": old_bytes and old_bytes[0],
        "old_free": old_bytes and old_bytes[1],
    }


def table(rows: list[dict], old: bool, dyn: dict | None = None) -> list[str]:
    head = f"{'program':16} {'ins':>5} {'pin':>5} {'d':>5} | {'bytes':>6} {'pin':>6} {'d':>5} | {'nopad':>6} {'pin':>6} {'d':>5}" + (f" | {'old':>6} {'d':>5}" if old else "") + "  flag"
    if dyn is not None:
        head = head.replace("  flag", f" | {'clk':>9} {'pin':>9} {'d%':>6}  flag")
    lines = [head]
    flagged = 0

    def worst(one: dict) -> tuple:
        # With --dyn the rows that run slower than the pin come first, the slowest first.
        figures = (dyn or {}).get(one["name"])
        lost = figures["clk"] / figures["pin_clk"] if isinstance(figures, dict) and figures["pin_clk"] else 0
        return (-lost, one["name"]) if dyn is not None else (one["name"],)

    for row in sorted(rows, key=worst):
        flags = []
        figures = (dyn or {}).get(row["name"])
        if isinstance(figures, dict):
            if not figures["same"]:
                flags.append("OUTPUT-DIFFERS")
            if figures["clk"] > figures["pin_clk"]:
                flags.append("CLK>pin")
        compared = not row.get("nolib")
        if compared and row["ins"] > row["pin_ins"]:
            flags.append("INS>pin")
        if compared and row["free"] and row["pin_free"] and row["free"] > row["pin_free"]:
            flags.append("BYTES>pin")
        if old and row["free"] and row["old_free"] and row["free"] > row["old_free"]:
            flags.append("BYTES>old")
        flagged += bool(flags)
        text = f"{row['name']:16} {row['ins']:5} {row['pin_ins']:5} {row['ins'] - row['pin_ins']:+5} | {row['bytes'] or -1:6} {row['pin_bytes']:6} {(row['bytes'] or 0) - row['pin_bytes']:+5} | {row['free'] or -1:6} {row['pin_free']:6} {(row['free'] or 0) - row['pin_free']:+5}"
        if old:
            text += f" | {row['old_free'] or -1:6} {((row['free'] or 0) - row['old_free']) if row['old_free'] else 0:+5}"
        if dyn is not None:
            if isinstance(figures, dict):
                text += f" | {figures['clk']:9} {figures['pin_clk']:9} {100 * (figures['clk'] - figures['pin_clk']) / max(figures['pin_clk'], 1):+6.1f}"
            else:
                text += f" | {'-':>9} {'-':>9} {'-':>6}"
        lines.append(text + "  " + " ".join(flags) + (f" +libgcc[{','.join(m.removesuffix('.o').strip('_') for m in row['lib'])}]" if row.get("lib") else "") + (" no pin comparison (no 16-bit libgcc)" if row.get("nolib") else ""))
    lines.append(
        f"programs {len(rows)} flagged {flagged} | ins {sum(r['ins'] for r in rows)} vs {sum(r['pin_ins'] for r in rows)}"
        f" | bytes {sum(r['bytes'] or 0 for r in rows)} vs {sum(r['pin_bytes'] for r in rows)}"
        f" | nopad {sum(r['free'] or 0 for r in rows)} vs {sum(r['pin_free'] for r in rows)}"
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
    parser.add_argument("--src", help="a directory of *.c; default: the repo's corpus (vsgcc/programs.py)")
    parser.add_argument("--llc", required=True)
    parser.add_argument("--work", required=True)
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--old", action="store_true")
    parser.add_argument("--ref", default=f"origin/{BRANCH}", help="the branch the llc should be built on")
    parser.add_argument("--max-behind", type=int, default=20, help="commits behind --ref before the header warns")
    parser.add_argument("--dyn", action="store_true", help="m32 only: also run each kernel in the vsgcc harness (dyn.py) and rank flagged rows by dynamic clocks")
    parser.add_argument("--reopt", action="store_true", help="clang's -O IR re-run through the pin's opt, then both llcs (not the default: see front_end_flags)")
    args = parser.parse_args(argv)
    work = Path(args.work)
    work.mkdir(parents=True, exist_ok=True)
    version = subprocess.run([args.llc, "--version"], capture_output=True, text=True).stdout.strip()
    md5 = hashlib.md5(Path(args.llc).read_bytes()).hexdigest()[:12]
    pin_version = subprocess.run([str(pin.PINNED_BIN / "llc"), "--version"], capture_output=True, text=True).stdout.splitlines()[:2]
    scope = " (backend only: --reopt feeds pin-optimised IR, ours' middle-end size gates do not run)" if args.reopt and args.level == "Os" else ""
    print(f"# pindiff {args.machine} -{args.level}{scope} frontend clang -fno-builtin isel=dag libgcc-divide-helpers=pin side: ours {version} md5 {md5}; pin {' '.join(pin_version)}; {pin.header(pin.target(args.machine, args.level == 'Os'))}")
    note = layout_note(pin.target(args.machine).datalayout, pin_llc_datalayout(pin.target(args.machine)))
    if note:
        print(note)
    stale = staleness(version, args.ref, str(Path(__file__).resolve().parents[2]), args.max_behind)
    if stale:
        print(stale)
    sources = sorted((Path(args.src) if args.src else corpus_sources(work / "src")).glob("*.c"))
    with ThreadPoolExecutor(args.jobs) as pool:
        results = list(pool.map(lambda source: one(source, args.machine, args.level, args.llc, work, args.old, args.reopt), sources))
    rows = [r for r in results if isinstance(r, dict)]
    print(failure_summary({source.stem: result for source, result in zip(sources, results)}, args.old))
    for source, result in zip(sources, results):
        if isinstance(result, str):
            print(f"# no figures for {source.stem}: {result}")
    dyn = None
    if args.dyn:
        if args.machine != "m32":
            parser.error("--dyn is m32 only")
        import dyn as dynmod

        dyn = dynmod.measure([row["name"] for row in rows], args.level, MACHINES["m32"]["march"], args.llc, work / "dyn", one)
        for name, result in dyn.items():
            if isinstance(result, str):
                print(f"# no dynamic figures for {name}: {result}")
    print("\n".join(table(rows, args.old, dyn)))
    return 0 if rows else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
