"""Compile every QB source in the corpus at m16 with two llrm-qb binaries and compare the objects: a refactor of
the runtime tables must leave all of them byte-identical."""
import subprocess, sys, hashlib, os
from pathlib import Path
sys.path.insert(0, "tools/dosbatch"); sys.path.insert(0, "tools")
from run_tests import compiler_arguments, header
# usage: LLRM_BIN=<any dir without stale bins> python3 tools/qb_identical.py BASE_LLRM_QB NEW_LLRM_QB
bins = [Path(sys.argv[1]), Path(sys.argv[2])]
scratch = Path(os.environ.get("TMPDIR", "/tmp"))
srcs = []
for d in "tests/qbrt tests/run/qb tests/run/huge examples tests/differential/conformance/qb45 tests/differential/conformance/vbdos tests/differential/conformance/pds71 tests/differential/qb-bc".split():
    srcs += sorted(Path(d).rglob("*.bas"))
same = diff = both_fail = 0
for s in srcs:
    h = header(s); args = compiler_arguments(s)
    if "dialect" not in h:
        d = "vbdos" if "vbdos" in s.parts else "pds71" if "pds71" in s.parts else "qb45"
        args = ["--dialect", d, "--runtime", d, *args]
    if not any(a.startswith("-O") for a in args): args.insert(0, "-O2")
    out = []
    for b in bins:
        o = scratch / "qb_identical.obj"; o.unlink(missing_ok=True)
        r = subprocess.run([str(b), *args, "-m16", "-o", str(o), str(s)], capture_output=True, text=True, timeout=300)
        out.append((r.returncode, hashlib.md5(o.read_bytes()).hexdigest() if r.returncode == 0 and o.exists() else r.stderr.strip()[:80]))
    if out[0] == out[1]:
        if out[0][0]: both_fail += 1
        else: same += 1
    else:
        diff += 1; print("DIFF", s, out)
print(f"identical {same} both-refused {both_fail} different {diff}")
