#!/usr/bin/env python3
"""What the DAG builder refuses across the corpora, by reason and program.

    tools/dagisel/builder_corpus.py BIN_DIR EXAMPLE [--jobs N]

BIN_DIR holds llrm-c, llrm-nib and llrm-qb; EXAMPLE is `cargo build --release -p llrm-codegen --example
builder_refusals`. Each source is compiled to the last IR its `--dump` writes (tests/run c/alias/huge at m16
and m32, nib and qb at m16, tests/qbrt, the QCport modules at m16 -Os), and the example reports the refused
blocks. Prints reasons with block and program counts. Needs the work dir on a disk, not tmpfs ($TMPDIR).
"""
from __future__ import annotations

import argparse
import os
import subprocess
import tempfile
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
QCPORT = Path(os.environ.get("QCPORT", Path.home() / "scratch/qcport/src"))
QCINC = Path(os.environ.get("QCPORT_INC", Path.home() / "scratch/qctc/inc"))


def jobs() -> list[tuple[str, str, str, list[str]]]:
    """(group, mode, program, command without `--dump DIR -o OBJ`)."""
    out = []
    inc = [one for d in ("host", "render", "model", "game", "sound", "ui", "qgl") for one in ("-I", str(QCPORT / d))] + ["-I", str(QCINC)]
    for sub in ("c", "alias", "huge"):
        for src in sorted((ROOT / "tests/run" / sub).glob("*.c")):
            for mode in ("m16", "m32"):
                out.append((f"run/{sub}", mode, src.stem, ["llrm-c", str(src), f"-{mode}", "-O2", "-I", str(ROOT / "tests/run" / sub)]))
    for src in sorted((ROOT / "tests/run/nib").glob("*.nib")):
        for mode in ("m16", "m32"):
            out.append(("run/nib", mode, src.stem, ["llrm-nib", str(src), f"-{mode}", "-O2"]))
    for src in sorted((ROOT / "tests/run/qb").glob("*.bas")):
        for mode in ("m16", "m32"):
            out.append(("run/qb", mode, src.stem, ["llrm-qb", str(src), "--dialect", "qb45", "-fqb-runtime=qb45", f"-{mode}", "-O2"]))
    for src in sorted((ROOT / "tests/qbrt").glob("*.bas")):
        for mode in ("m16", "m32"):
            out.append(("qbrt", mode, src.stem, ["llrm-qb", str(src), "--dialect", "qb45", "-fqb-runtime=qb45", f"-{mode}", "-O2"]))
    if QCPORT.is_dir():
        for d in ("host", "render", "model", "game", "sound", "ui"):
            for src in sorted((QCPORT / d).glob("*.c")):
                out.append(("qcport", "m16", src.stem, ["llrm-c", str(src), "-m16", "-Os", *inc]))
    return out


def compile_one(job, bins: Path, work: Path):
    group, mode, program, cmd = job
    dump = work / f"{group.replace('/', '_')}-{mode}-{program}"
    dump.mkdir(parents=True, exist_ok=True)
    cmd = [str(bins / cmd[0]), *cmd[1:], "--dump", str(dump), "-o", str(dump / "out.obj")]
    done = subprocess.run(cmd, capture_output=True, text=True)
    files = sorted(dump.glob("**/[0-9]*.ll"), key=lambda path: (path.stat().st_mtime, path.name))
    return job, (files[-1] if files else None), done.returncode


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("bins")
    parser.add_argument("example")
    parser.add_argument("--jobs", type=int, default=6)
    args = parser.parse_args()
    work = Path(tempfile.mkdtemp(prefix="builder-corpus-", dir=os.environ.get("TMPDIR")))
    with ThreadPoolExecutor(args.jobs) as pool:
        compiled = list(pool.map(lambda job: compile_one(job, Path(args.bins), work), jobs()))
    reasons: dict[str, Counter] = defaultdict(Counter)
    programs: dict[str, set] = defaultdict(set)
    no_ir = Counter()
    totals = Counter()
    by_mode: dict[str, Counter] = defaultdict(Counter)
    for (group, mode, program, _), ir, _code in compiled:
        if ir is None:
            no_ir[f"{group}/{mode}"] += 1
            continue
        totals[f"{group}/{mode}"] += 1
        run = subprocess.run([args.example, mode, str(ir)], capture_output=True, text=True)
        lines = [line for line in run.stdout.splitlines() if line.startswith("REFUSED ")]
        # a function whose entry block is refused makes the later blocks fail on its arguments: the first reason is the cause
        causes = {line.split(" ", 2)[1].split(":")[0] for line in lines if "argument read before" not in line}
        for line in run.stdout.splitlines():
            if line.startswith("REFUSED "):
                if "argument read before" in line and line.split(" ", 2)[1].split(":")[0] in causes:
                    continue
                reason = line.split(" ", 2)[2] if line.count(" ") >= 2 else line
                reasons[reason][group] += 1
                by_mode[reason][mode] += 1
                programs[reason].add(f"{group}:{program}:{mode}")
            elif line.startswith("FILE ") and "unparsed" in line:
                reasons["MIR parse: " + line.split("unparsed ", 1)[1][:80]][group] += 1
                programs["MIR parse: " + line.split("unparsed ", 1)[1][:80]].add(f"{group}:{program}:{mode}")
    print("compiled:", dict(totals), "no IR:", dict(no_ir))
    for reason, by_group in sorted(reasons.items(), key=lambda item: -sum(item[1].values())):
        print(f"{sum(by_group.values()):6} blocks {len(programs[reason]):4} programs  {reason}  m16={by_mode[reason]['m16']} m32={by_mode[reason]['m32']}  {dict(by_group)}")


if __name__ == "__main__":
    main()
