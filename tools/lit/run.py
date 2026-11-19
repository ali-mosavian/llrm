#!/usr/bin/env python3
"""Runs the LLVM pin's lit tests with `opt` swapped for `llrm-opt`.

    run.py [--mode llrm|pin] [--out DIR] [--llrm-opt PATH] [lit args] TEST_DIR_OR_FILE...

`--mode pin` keeps the pin's own `opt`: the tests that fail there fail on the
pin's build, not on ours. The pin's `FileCheck`, `not` and `count` serve both.
Features follow the pin's `CMakeCache.txt`: `asserts` from
`LLVM_ENABLE_ASSERTIONS`, `<target>-registered-target` from
`LLVM_TARGETS_TO_BUILD`.
"""
import argparse
import os
import sys
from pathlib import Path

HOME = Path.home()
PIN_SRC = HOME / "scratch/llvm-src"
PIN_BUILD = HOME / "scratch/llvm-build"
REPO = Path(__file__).resolve().parents[2]


def cache(build):
    """The `KEY=VALUE` pairs of a CMakeCache.txt, types dropped."""
    values = {}
    for line in (build / "CMakeCache.txt").read_text().splitlines():
        if "=" in line and not line.startswith(("//", "#")):
            key, value = line.split("=", 1)
            values[key.split(":")[0]] = value
    return values


def site(args, bin_dir):
    values = cache(args.pin_build)
    targets = values.get("LLVM_TARGETS_TO_BUILD", "").replace(" ", ";").split(";")
    lines = {
        "llrm_test_root": str(args.pin_src / "llvm/test"),
        "llrm_exec_root": str(args.out / "exec"),
        "llrm_bin_dir": str(bin_dir),
        "llrm_host_triple": args.host_triple,
        "llrm_features": [],
        "llrm_asserts": values.get("LLVM_ENABLE_ASSERTIONS", "OFF") in ("ON", "TRUE", "1"),
        "llrm_targets": [target for target in targets if target],
    }
    body = "".join(f"config.{key} = {value!r}\n" for key, value in lines.items())
    body += f"lit_config.load_config(config, {str(REPO / 'tools/lit/lit.cfg.py')!r})\n"
    return body


def tools(args):
    bin_dir = args.out / "bin"
    bin_dir.mkdir(parents=True, exist_ok=True)
    opt = args.pin_build / "bin/opt" if args.mode == "pin" else Path(args.llrm_opt)
    for name, target in (
        ("opt", opt),
        ("FileCheck", args.pin_build / "bin/FileCheck"),
        ("not", args.pin_build / "bin/not"),
        ("count", args.pin_build / "bin/count"),
    ):
        link = bin_dir / name
        link.unlink(missing_ok=True)
        link.symlink_to(target.resolve())
    return bin_dir


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--mode", choices=("llrm", "pin"), default="llrm")
    parser.add_argument("--out", type=Path, default=Path("lit-out"))
    parser.add_argument("--llrm-opt", default=str(REPO / "target/debug/llrm-opt"))
    parser.add_argument("--pin-src", type=Path, default=PIN_SRC)
    parser.add_argument("--pin-build", type=Path, default=PIN_BUILD)
    parser.add_argument("--host-triple", default="x86_64-unknown-linux-gnu")
    args, lit_args = parser.parse_known_args()
    args.out = args.out.resolve()
    (args.out / "exec").mkdir(parents=True, exist_ok=True)
    bin_dir = tools(args)
    config = args.out / "lit.site.cfg.py"
    config.write_text(site(args, bin_dir))
    sys.path.insert(0, str(args.pin_src / "llvm/utils/lit"))
    from lit.main import main as lit_main

    sys.argv = [sys.argv[0], *lit_args]

    test_root = (args.pin_src / "llvm/test").resolve()
    lit_main({"config_map": {os.path.normcase(str(test_root / "lit.cfg.py")): str(config)}})


if __name__ == "__main__":
    main()
