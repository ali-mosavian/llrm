#!/usr/bin/env python3
"""gvn dump replay: for each revision, gvn's instructions (the run's minus a parse-only run) and the output hash on
every input of a gvn-input set. Each revision is built into its own target with CARGO_INCREMENTAL=0.

usage: gvn_table.py INPUT_DIR REV [REV...]    (the first REV is the baseline)
"""
import glob, os, subprocess, sys

ROOT = os.path.expanduser("~/scratch/replay")
DRIVER = os.path.join(os.path.dirname(os.path.abspath(__file__)), "replay_gvn.rs")


def sh(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, **kw)


def build(rev):
    sha = sh(["git", "rev-parse", "--short=9", rev]).stdout.strip()
    tree, target = f"{ROOT}/tree-{sha}", f"{ROOT}/target-{sha}"
    if not os.path.isdir(tree):
        subprocess.run(["git", "worktree", "add", "-q", "--detach", tree, rev], check=True)
    bin_dir = f"{tree}/crates/opt/llrm-transforms/src/bin"
    os.makedirs(bin_dir, exist_ok=True)
    subprocess.run(["cp", DRIVER, bin_dir], check=True)
    env = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_TARGET_DIR=target)
    done = sh(["cargo", "build", "--release", "--bin", "replay_gvn", "-q"], cwd=tree, env=env)
    if done.returncode:
        sys.exit(done.stderr[-2000:])
    return sha, f"{target}/release/replay_gvn"


def instructions(binary, path, *flags):
    out = sh(["perf", "stat", "-x,", "-e", "instructions:u", binary, path, *flags]).stderr
    return int(next(l for l in out.splitlines() if "instructions" in l).split(",")[0])


def measure(binary, path):
    hashed = sh([binary, path]).stdout.strip()
    return instructions(binary, path) - instructions(binary, path, "--parse-only"), hashed


inputs = sorted(glob.glob(f"{sys.argv[1]}/*.in.ll"))
rows = {}
for rev in sys.argv[2:]:
    sha, binary = build(rev)
    rows[sha] = [measure(binary, path) for path in inputs]
shas = list(rows)
print("input".ljust(48), *[s.rjust(12) for s in shas], "output")
for i, path in enumerate(inputs):
    same = "same" if len({rows[s][i][1] for s in shas}) == 1 else "DIFFERENT"
    print(os.path.basename(path)[:47].ljust(48), *[f"{rows[s][i][0] / 1e6:11.2f}M" for s in shas], same)
print("total".ljust(48), *[f"{sum(r[0] for r in rows[s]) / 1e6:11.1f}M" for s in shas])
