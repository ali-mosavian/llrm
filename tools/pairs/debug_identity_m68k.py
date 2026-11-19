#!/usr/bin/env python3
"""`-g` must not change m68k code: the assembly with `-g`, minus its `.loc`
lines, is the assembly without it, over a corpus of C programs (pin clang
and opt make the IR). A leg whose `-g` output has no `.loc` is refused, never
counted identical."""
import argparse, os, subprocess, sys, tempfile
from pathlib import Path


def run(command):
    done = subprocess.run(command, capture_output=True, text=True, timeout=600)
    if done.returncode:
        raise RuntimeError(done.stderr.strip().splitlines()[-1] if done.stderr.strip() else "failed")
    return done.stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("llc", help="llrm-m68k-llc")
    parser.add_argument("--src", required=True)
    parser.add_argument("--limit", type=int, default=40)
    parser.add_argument("--levels", default="O2,Os")
    parser.add_argument("--pin", default=os.environ.get("LLRM_PIN_BIN"))
    args = parser.parse_args()
    if not args.pin:
        parser.error("--pin or LLRM_PIN_BIN names the pin LLVM bin directory")
    work = Path(tempfile.mkdtemp(prefix="dbgid68k-"))
    same = changed = refused = 0
    for source in sorted(Path(args.src).glob("*.c"))[: args.limit]:
        try:
            ir, opt = work / f"{source.stem}.ll", work / f"{source.stem}.opt.ll"
            run([f"{args.pin}/clang", "--target=m68k-unknown-linux-gnu", "-O2", "-g", "-S", "-emit-llvm", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-O0-optnone", str(source), "-o", str(ir)])
            run([f"{args.pin}/opt", "-O2", str(ir), "-S", "-o", str(opt)])
        except (RuntimeError, subprocess.TimeoutExpired) as why:
            refused += 2
            print(f"  refused {source.name}: {why}")
            continue
        for level in args.levels.split(","):
            tag = f"{source.stem}-{level}"
            try:
                plain = run([args.llc, "-S", f"-{level}", "-o", "/dev/stdout", str(opt)])
                debug = run([args.llc, "-S", "-g", f"-{level}", "-o", "/dev/stdout", str(opt)])
            except (RuntimeError, subprocess.TimeoutExpired) as why:
                refused += 1
                print(f"  refused {tag}: {why}")
                continue
            if "\t.loc\t" not in debug:
                refused += 1
                print(f"  no .loc with -g: {tag}")
                continue
            kept = "".join(line + "\n" for line in debug.splitlines() if not line.strip().startswith((".loc", ".file")))
            if kept == plain:
                same += 1
            else:
                changed += 1
                print(f"  differs {tag}")
    print(f"debug_identity_m68k: {same + changed} compared, {refused} refused; {changed} differ, {same} identical")
    return 1 if changed else 0


if __name__ == "__main__":
    sys.exit(main())
