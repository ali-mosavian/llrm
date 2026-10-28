#!/usr/bin/env python3
"""Generate crates/ir/llrm-mir/src/libfunc/attrs.rs from the pin's BuildLibCalls.cpp.

    tools/gen/gen_libattrs.py [--check]      write the table, or fail if the checked-in one differs

The switch of `inferNonMandatoryLibFuncAttrs` becomes `infer`, a match on `LibFunc` that calls the
`Setters` the pin's `setX(F, ...)` helpers are. The pin is LLVM_SRC (default ~/scratch/llvm-src) at
the commit PIN. With --check and no source tree it passes.
"""
import os
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gen_libfunc  # noqa: E402

PIN = gen_libfunc.PIN
ROOT = gen_libfunc.ROOT
OUT = ROOT / "crates/ir/llrm-mir/src/libfunc/attrs.rs"
SRC = Path(os.environ.get("LLVM_SRC", Path.home() / "scratch/llvm-src")) / "llvm/lib/Transforms/Utils/BuildLibCalls.cpp"

# The `setX` helpers of the switch -> `Setters` methods. A helper that takes an argument number has
# the number as its argument.
METHODS = {
    "setDoesNotAccessMemory": "does_not_access_memory",
    "setIsCold": "is_cold",
    "setNoReturn": "no_return",
    "setOnlyReadsMemory": "only_reads_memory",
    "setOnlyWritesMemory": "only_writes_memory",
    "setOnlyAccessesArgMemory": "only_accesses_arg_memory",
    "setOnlyAccessesInaccessibleMemOrArgMem": "only_accesses_inaccessible_mem_or_arg_mem",
    "setOnlyAccessesInaccessibleMemOrErrnoMem": "only_accesses_inaccessible_mem_or_errno_mem",
    "setOnlyAccessesInaccessibleMemOrArgMemOrErrnoMem": "only_accesses_inaccessible_mem_or_arg_mem_or_errno_mem",
    "setOnlyWritesErrnoMemory": "only_writes_errno_memory",
    "setOnlyWritesArgMemOrErrnoMem": "only_writes_arg_mem_or_errno_mem",
    "setDoesNotThrow": "does_not_throw",
    "setDoesNotCallback": "does_not_callback",
    "setRetDoesNotAlias": "ret_does_not_alias",
    "setDoesNotCapture": "does_not_capture",
    "setDoesNotAlias": "does_not_alias",
    "setDoesNotSync": "does_not_sync",
    "setRetNoUndef": "ret_no_undef",
    "setArgsNoUndef": "args_no_undef",
    "setArgNoUndef": "arg_no_undef",
    "setRetAndArgsNoUndef": "ret_and_args_no_undef",
    "setReturnedArg": "returned_arg",
    "setDoesNotFreeMemory": "does_not_free_memory",
    "setWillReturn": "will_return",
    "setAlignedAllocParam": "aligned_alloc_param",
    "setAllocatedPointerParam": "allocated_pointer_param",
}
# Without and with an argument number: the overloads of one name.
WITH_ARG = {"only_reads_memory", "only_writes_memory"}
KINDS = {"Alloc": "ALLOC", "Realloc": "REALLOC", "Free": "FREE", "Uninitialized": "UNINITIALIZED", "Zeroed": "ZEROED", "Aligned": "ALIGNED"}


def variants() -> dict[str, str]:
    """The LibFunc variant of each TargetLibraryInfo.td def, as gen_libfunc names them."""
    text = re.sub(r"/\*.*?\*/", "", re.sub(r"//[^\n]*", "", gen_libfunc.SRC.read_text()), flags=re.S)
    defs = gen_libfunc.DEF.findall(text)
    made, names = [], {}
    for name, string, *_ in defs:
        variant = gen_libfunc.camel(name)
        if variant in made:
            variant = gen_libfunc.camel(re.sub(r"^_+", lambda m: "under_" * len(m.group()), string))
        made.append(variant)
        names[name] = variant
    return names


def switch_body() -> str:
    text = SRC.read_text()
    start = text.index("switch (TheLibFunc) {", text.index("bool llvm::inferNonMandatoryLibFuncAttrs(Function &F"))
    end = text.index("  default:", start)
    body = text[start + len("switch (TheLibFunc) {"):end]
    return re.sub(r"//[^\n]*", "", body)


def statement(stmt: str, names: dict[str, str]) -> str:
    """One `Changed |= setX(F, ...);` as a Rust call."""
    stmt = " ".join(stmt.split()).replace("( ", "(")
    m = re.fullmatch(r"Changed \|= (\w+)\((.*)\)", stmt)
    assert m, stmt
    helper, args = m.group(1), m.group(2)
    assert args.startswith("F"), stmt
    args = args[1:].lstrip(", ").strip() if args != "F" else ""
    if helper == "setAllocSize":
        first, second = [a.strip() for a in args.split(",")]
        return f"set.alloc_size({first}, {'None' if second == 'std::nullopt' else f'Some({second})'});"
    if helper == "setAllocFamily":
        tern = re.fullmatch(r"TheLibFunc == LibFunc_(\w+) \? (\"\w+\") : (\"\w+\")", args)
        if tern:
            return f"set.alloc_family(if lib == LibFunc::{names[tern.group(1)]} {{ {tern.group(2)} }} else {{ {tern.group(3)} }});"
        return f"set.alloc_family({args});"
    if helper == "setAllocKind":
        kinds = " | ".join(f"AllocKind::{KINDS[k]}" for k in re.findall(r"AllocFnKind::(\w+)", args))
        return f"set.alloc_kind({kinds});"
    method = METHODS[helper]
    if method in WITH_ARG and args:
        return f"set.{method}_arg({args});"
    return f"set.{method}({args});"


def blocks(names: dict[str, str]):
    """The switch as (labels, statements, falls through) in source order."""
    out, labels, stmts = [], [], []
    pending = re.sub(r"\[\[fallthrough\]\];", "FALLTHROUGH;", switch_body())
    for raw in (part.strip() for part in pending.split(";")):
        # A statement run may start with case labels.
        while True:
            m = re.match(r"case LibFunc_(\w+):\s*", raw)
            if not m:
                break
            if stmts:
                out.append((labels, stmts, False))
                labels, stmts = [], []
            labels.append(names[m.group(1)])
            raw = raw[m.end():]
        if not raw:
            continue
        if raw == "break":
            out.append((labels, stmts, False))
            labels, stmts = [], []
        elif raw == "FALLTHROUGH":
            out.append((labels, stmts, True))
            labels, stmts = [], []
        elif raw == "return Changed":
            # `__cxa_throw` returns before the `nofree` of the end.
            stmts.append("return false;")
        else:
            stmts.append(statement(raw, names))
    assert not labels and not stmts
    return out


def render() -> str:
    names = variants()
    todo = blocks(names)
    lines = [
        f"// @generated by tools/gen/gen_libattrs.py from BuildLibCalls.cpp at {PIN}; do not edit.",
        "",
        "use super::LibFunc;",
        "use super::infer::{AllocKind, Setters};",
        "",
        "/// The attributes `inferNonMandatoryLibFuncAttrs` gives the library function `lib`, one call of `set` per",
        "/// `setX(F, ...)` of its case in the pin. False where the pin returns there, before the step that follows the",
        "/// switch.",
        "pub fn infer(",
        "    lib: LibFunc,",
        "    set: &mut impl Setters,",
        ") -> bool {",
        "    match lib {",
    ]
    # A block that falls through runs the statements of the next block too, labels apart.
    for at, (labels, stmts, falls) in enumerate(todo):
        body, i = list(stmts), at
        while todo[i][2]:
            i += 1
            body += todo[i][1]
        arms = "\n        | ".join(f"LibFunc::{label}" for label in labels)
        lines.append(f"        {arms} => {{")
        lines += [f"            {one}" for one in body]
        lines.append("        }")
    lines += ["        _ => {}", "    }", "    true", "}"]
    text = "\n".join(lines) + "\n"
    # The tree is formatted by tools/fmt.sh: the table is what it makes of this.
    done = subprocess.run([str(ROOT / "tools/fmt.sh"), "--stdin", str(OUT)], input=text, capture_output=True, text=True)
    if done.returncode != 0:
        raise SystemExit(f"gen_libattrs: tools/fmt.sh failed: {done.stderr}")
    return done.stdout


def main() -> int:
    if not SRC.is_file() or not gen_libfunc.SRC.is_file():
        print(f"gen_libattrs: no {SRC}: skipped", file=sys.stderr)
        return 0 if "--check" in sys.argv else 1
    text = gen_libfunc.formatted(render(), OUT)
    if "--check" in sys.argv:
        if OUT.read_text() != text:
            print("gen_libattrs: crates/ir/llrm-mir/src/libfunc/attrs.rs is stale: run tools/gen/gen_libattrs.py", file=sys.stderr)
            return 1
        return 0
    OUT.write_text(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
