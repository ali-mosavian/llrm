#!/usr/bin/env python3
"""Clocks of old and dag (OMF), dag (ELF) and the pin's `llc` on dag's pre-isel IR (ELF), one harness, one price table.

    tools/pairs/ambt_pin.py PROG LLRM_C       -> one JSON line; PIN_TRIPLE (default i486-unknown-none-elf, stack alignment 4), PIN_EXTRA (llc flags)

A harness that does not answer (its stale-binary guard exits with a message, a program that fails to link) stops the run with that message:
a null clock once looked like a result. Needs `cargo build --release --bins` in this tree and the rows corpus in ~/scratch/rows/main24d.
"""
import dataclasses, json, os, shutil, subprocess, sys, tempfile
from pathlib import Path

R = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(R / "tools/pairs"))
sys.path.insert(0, str(R / "tools/difftest"))
import difftest  # noqa: E402
import pin  # noqa: E402


def answer(prog: str, variant: str, done: subprocess.CompletedProcess) -> dict:
    """The harness's JSON line, or the reason it gave none."""
    lines = done.stdout.strip().splitlines()
    if done.returncode or not lines:
        raise SystemExit(f"ambt_pin: harness gave no clocks for {prog} {variant} (exit {done.returncode}): {(done.stderr or done.stdout).strip()[-600:]}")
    return json.loads(lines[-1])


def sh(cmd: list[str], **kw) -> subprocess.CompletedProcess:
    done = subprocess.run(cmd, capture_output=True, text=True, **kw)
    if done.returncode:
        raise SystemExit(f"ambt_pin: {' '.join(map(str, cmd[:3]))} failed: {done.stderr[-400:]}")
    return done


def main(prog: str, llrm: str) -> dict:
    harness_dir = next((R / "crates/target").glob("*/vsgcc"))
    base = Path(os.environ.get("ROWS_MAIN24D", Path.home() / "scratch/rows/main24d"))
    work = Path(tempfile.mkdtemp(prefix="ambt-", dir=Path.home() / "scratch"))
    (work / "o").mkdir()
    (work / "b").mkdir()
    for f in (base / "b").glob(f"{prog}.c"):
        shutil.copy(f, work / "b" / f.name)
    shutil.copy(base / "abi", work / "abi")
    sh(["bash", "-c", f'R={R}; H={harness_dir}; VSGCC_WORK={work}; eval "$(grep -E \'^(gcc -m32 -c|sha256sum) \' $H/run.sh)"'])
    env = dict(os.environ, VSGCC_WORK=str(work), LLRM=llrm)
    src = work / "b" / f"{prog}.c"
    flags = ["-m32", "-mabi=sysv", "-march=i486", "-O2"]
    emulation = sh(["python3", str(R / "tools/linkrecipe.py"), "x86-m32", "ld-emulation"]).stdout.strip()
    libgcc = sh(["gcc", "-m32", "-print-libgcc-file-name"]).stdout.strip()

    def link(obj: str, variant: str) -> None:
        sh(["ld", "-m", emulation, "-static", "-e", "0", f"-Ttext={pin.LINK_TEXT_BASE:#x}", f"--just-symbols={work}/stub.elf", "-o", f"{work}/b/{prog}.{variant}.elf", obj, libgcc])

    def clocks(variant: str) -> dict:
        done = subprocess.run(["uv", "run", "-q", "--project", str(R / "tools"), "python", str(harness_dir / "harness.py"), prog, variant], capture_output=True, text=True, env=env)
        return answer(prog, variant, done)

    res = {}
    for selector in ("old", "dag"):
        sh([llrm, *flags, f"--isel={selector}", "-o", f"{work}/o/{prog}.llrm.obj", str(src)])
        res[f"{selector}_omf"] = clocks("llrm")
    sh([llrm, *flags, "--isel=dag", "-fobject-format=elf", "-o", f"{work}/b/{prog}.dagElf.o", str(src)])
    link(f"{work}/b/{prog}.dagElf.o", "dagElf")
    res["dag_elf"] = clocks("dagElf")
    dump = work / "dump"
    sh([llrm, *flags, "--isel=dag", "--dump", str(dump), "-S", "-o", str(work / "x.s"), str(src)])
    stage = pin.backend_ir(dump)
    if stage is None:
        raise SystemExit(f"ambt_pin: no switchconv stage in {dump} for {prog}")
    ir = difftest.normalise(stage.read_text())
    target = dataclasses.replace(pin.target("m32"), triple=os.environ.get("PIN_TRIPLE", "i486-unknown-none-elf"))
    done = pin.llc(["-O2", "-disable-lsr", *os.environ.get("PIN_EXTRA", "").split(), "-filetype=obj", "-o", f"{work}/b/{prog}.pinllc.o"], ir, target)
    if done.returncode:
        raise SystemExit(f"ambt_pin: the pin's llc refused {prog}: {done.stderr[-400:]}")
    link(f"{work}/b/{prog}.pinllc.o", "pinllc")
    res["pin_llc"] = clocks("pinllc")
    res["work"] = str(work)
    return {k: (v if k == "work" else {x: v[x] for x in ("clocks", "ins", "code", "ok") if x in v}) for k, v in res.items()}


if __name__ == "__main__":
    print(json.dumps(main(sys.argv[1], sys.argv[2])))
