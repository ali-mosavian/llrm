#!/usr/bin/env python3
"""The 6502 runtime as an archive of per-routine objects, for ld65 to pull members of by undefined symbol (what cc65 and llvm-mos ship).
usage: mkrt.py LLRM_CC OUT.lib [WORKDIR]. Members: every top-level label of runtime/mos_rt.s (a helper that was private is renamed
`__rt_<name>` and exported, so a user symbol cannot meet it), the BSS cells as one member, and every runtime/compiler-rt file of the groups
`tests/rt_index.sh` lists, compiled per file (-Os). rc0..rc25 are equated at ZP (default $80)."""
import re, subprocess, sys, tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
MOS_RT = ROOT / "crates/target/llrm-6502/runtime/mos_rt.s"

def split_rt(text, zp=0x80, cells=26):
    """{member name: ca65 source} for mos_rt.s text."""
    equates = "".join(f"rc{i} = ${zp + i:02X}\n" for i in range(cells))
    macros, bss, blocks, pending, in_macro, in_bss = [], [], [], [], False, False
    for line in text.splitlines():
        if line.startswith(".macro"):
            in_macro = True
        if in_macro:
            macros.append(line)
            in_macro = not line.startswith(".endmacro")
            continue
        if line.startswith('.segment "BSS"'):
            in_bss = True
        if in_bss:
            bss.append(line)
            in_bss = not line.startswith('.segment "CODE"')
            continue
        m = re.match(r"([A-Za-z_]\w*):", line)
        if line.startswith(".export"):
            pending += [n.strip() for n in line[len(".export"):].split(",")]
            continue
        if m:
            blocks.append([m.group(1), pending, []])
            pending = []
        if blocks:
            blocks[-1][2].append(line)
    exported = {n for _, e, _ in blocks for n in e}
    cell_names = [m.group(1) for l in bss if (m := re.match(r"([A-Za-z_]\w*):", l))]
    rename = {name: f"__rt_{name}" for name, _, _ in blocks if name not in exported}
    def tokens(s):
        return re.sub(r"[A-Za-z_]\w*", lambda m: rename.get(m.group(0), m.group(0)), s)
    # a branch reaches no further than its own object, and a block that does not end in rts/jmp falls into the next: such blocks are one member
    index = {name: i for i, (name, _, _) in enumerate(blocks)}
    parent = list(range(len(blocks)))
    def find(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i
    for i, (name, _, body) in enumerate(blocks):
        code = [re.sub(r";.*", "", l).split() for l in body[1:]]
        code = [c for c in code if c and not c[0].startswith((".", "@")) and not c[0].endswith(":")]
        for c in code:
            if c[0].lower() in {"bcc", "bcs", "beq", "bne", "bmi", "bpl", "bvc", "bvs", "bra"} and len(c) > 1 and c[1] in index and index[c[1]] != i:
                parent[find(index[c[1]])] = find(i)
        if i + 1 < len(blocks) and (not code or code[-1][0].lower() not in {"rts", "jmp", "rti", "bra"}):
            parent[find(i + 1)] = find(i)
    groups = {}
    for i in range(len(blocks)):
        groups.setdefault(find(i), []).append(i)
    head = ".autoimport +\n" + equates + "\n".join(macros) + "\n"
    out = {}
    for members in groups.values():
        names = [rename.get(blocks[i][0], blocks[i][0]) for i in members]
        body = "\n".join(tokens("\n".join(blocks[i][2])) for i in members)
        out[names[0]] = head + ".export " + ", ".join(names) + "\n" + body + "\n"
    out["__rt_cells"] = head + ".export " + ", ".join(cell_names) + "\n" + "\n".join(bss) + "\n"
    return out

def pulled(map_text):
    """{object or archive member: bytes of CODE+RODATA+DATA} the link took, without the driver and cc65's own library."""
    sizes, cur = {}, None
    for line in map_text[map_text.index("Modules list"):map_text.index("Segment list")].splitlines():
        if line.endswith(":") and not line.startswith(" "):
            cur = line[:-1]
        elif cur and re.match(r"\s+(CODE|RODATA|DATA)\s", line) and "sim6502.lib" not in cur and cur != "d.o":
            name = re.sub(r".*\((.*)\)", r"\1", cur)
            sizes[name] = sizes.get(name, 0) + int(re.search(r"Size=([0-9A-F]+)", line).group(1), 16)
    return sizes

def main():
    cc, out = sys.argv[1], Path(sys.argv[2]).resolve()
    work = Path(sys.argv[3]) if len(sys.argv) > 3 else Path(tempfile.mkdtemp(prefix="rt-"))
    work.mkdir(parents=True, exist_ok=True)
    sys.path.insert(0, str(HERE))
    import crlib
    rt = ROOT / "runtime" / "compiler-rt"
    sources = {f"mos_{k}": v for k, v in split_rt(MOS_RT.read_text()).items()}
    for f in sorted(set(crlib.index(rt).values())):
        sources[f.replace("/", "_")[:-2]] = ".autoimport +\n" + crlib.member(rt, cc, f, ["-Os"])
    objs = []
    for stem, text in sources.items():
        (work / f"{stem}.s").write_text(text)
        a = subprocess.run(["ca65", "-t", "sim6502", "-o", f"{stem}.o", f"{stem}.s"], cwd=work, capture_output=True, text=True)
        if a.returncode:
            sys.exit(f"ca65 {stem}: {a.stderr[-200:]}")
        objs.append(f"{stem}.o")
    out.unlink(missing_ok=True)
    subprocess.run(["ar65", "a", str(out), *objs], cwd=work, check=True)
    print(f"{out}: {len(objs)} members")

if __name__ == "__main__":
    main()
