#!/usr/bin/env python3
"""Write crates/ir/llrm-mir/src/intrinsic_attrs.rs: the attributes LLVM gives each intrinsic.

    python3 tools/intrinsics/gen.py           rewrite the file: the intrinsics MIR enumerates and
                                              ValueTracking reads by name
    python3 tools/intrinsics/gen.py --stdout  print it
    python3 tools/intrinsics/gen.py --all F   write every intrinsic to F (the oracle's input; not committed)

Reads Intrinsics.td through `llvm-tblgen -dump-json` and follows CodeGenIntrinsics.cpp
(`setProperty`, `setDefaultProperties`) and IntrinsicEmitter.cpp (`getEffectiveME`, the function and
argument attribute sets). A property it does not know stops the run.

LLVM_SRC (default ~/scratch/llvm-src) is a checkout at COMMIT; LLVM_BUILD (default
~/scratch/llvm-build) holds bin/llvm-tblgen.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import textwrap
from collections import Counter
from pathlib import Path

COMMIT = "d1106deb71cc"
ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "crates/ir/llrm-mir/src/intrinsic_attrs.rs"

# ModRefInfo bits, and IRMemLocation in enum order, as MIR's `memory(...)` names them.
REF, MOD = 1, 2
LOCATIONS = ["argmem", "inaccessiblemem", "errnomem", "other", "target_mem0", "target_mem1"]
LOCATION_OF = {"ArgMem": 0, "InaccessibleMem": 1, "TargetMem0": 4, "TargetMem1": 5}
ACCESS = {0: "none", 1: "read", 2: "write", 3: "readwrite"}

# Property -> function attribute, in IntrinsicEmitter's order.
FLAGS = {
    "IntrNoReturn": "noreturn",
    "IntrNoCallback": "nocallback",
    "IntrNoSync": "nosync",
    "IntrNoFree": "nofree",
    "IntrWillReturn": "willreturn",
    "IntrCold": "cold",
    "IntrNoDuplicate": "noduplicate",
    "IntrNoMerge": "nomerge",
    "IntrConvergent": "convergent",
    "IntrSpeculatable": "speculatable",
    "IntrStrictFP": "strictfp",
    "IntrNoCreateUndefOrPoison": "nocreateundeforpoison",
}
# What MIR's `Attribute` cannot say: dropped, and counted in the header.
UNREPRESENTABLE = {"noduplicate", "strictfp", "nocreateundeforpoison", "nofreeobj"}
# Properties that are no attribute: nothing to emit.
NOT_ATTRIBUTES = {"Commutative", "IntrTriviallyScalarizable", "ArgInfo", "RangeSet", "IntrHasSideEffects"}
ARGUMENT_FLAGS = {
    "NoCapture": "nocapture",  # `captures(none)`
    "NoAlias": "noalias",
    "NoUndef": "noundef",
    "NonNull": "nonnull",
    "Returned": "returned",
    "ReadOnly": "readonly",
    "WriteOnly": "writeonly",
    "ReadNone": "readnone",
    "ImmArg": "immarg",
    "NoFreeObj": "nofreeobj",
}
ARGUMENT_INTS = {"Align": ("align", "Align"), "Dereferenceable": ("dereferenceable", "Bytes")}


def tblgen() -> dict:
    src = Path(os.environ.get("LLVM_SRC", Path.home() / "scratch/llvm-src"))
    build = Path(os.environ.get("LLVM_BUILD", Path.home() / "scratch/llvm-build"))
    head = subprocess.run(["git", "-C", src, "rev-parse", "HEAD"], capture_output=True, text=True, check=True)
    if not head.stdout.startswith(COMMIT):
        sys.exit(f"gen: {src} is at {head.stdout.strip()}, not {COMMIT}")
    llvm = src / "llvm"
    out = subprocess.run(
        [build / "bin/llvm-tblgen", "-dump-json", "include/llvm/IR/Intrinsics.td", "-I", "include", "-I", "include/llvm/IR"],
        cwd=llvm,
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(out.stdout)


def memory(properties: list[tuple[str, dict]], side_effects: bool) -> int | None:
    """The `ME` of `setProperty`, per location; `getEffectiveME` drops `none` under side effects."""
    me = [3] * 6
    for name, record in properties:
        if name == "IntrNoMem":
            me = [0] * 6
        elif name == "IntrReadMem":
            me = [one & REF for one in me]
        elif name == "IntrWriteMem":
            me = [one & MOD for one in me]
        elif name == "IntrArgMemOnly":
            me = [one & 3 if at == 0 else 0 for at, one in enumerate(me)]
        elif name == "IntrInaccessibleMemOnly":
            me = [one & 3 if at == 1 else 0 for at, one in enumerate(me)]
        elif name == "IntrInaccessibleMemOrArgMemOnly":
            me = [one & 3 if at in (0, 1) else 0 for at, one in enumerate(me)]
        elif "IntrRead" in record["!superclasses"] or "IntrWrite" in record["!superclasses"]:
            base = MOD if "IntrRead" in record["!superclasses"] else REF
            mask = [base] * 6
            for location in record["MemLoc"]:
                mask[LOCATION_OF[location["def"]]] = 3
            me = [one & other for one, other in zip(me, mask)]
    if side_effects and all(one == 0 for one in me):
        return None
    return None if all(one == 3 for one in me) else me


def render_memory(me: list[int]) -> list[tuple[str | None, str]]:
    """`Attribute::getAsString`: `other` is the default, then each location that differs."""
    other = me[3]
    entries: list[tuple[str | None, str]] = []
    if other != 0 or all(one == other for one in me):
        entries.append((None, ACCESS[other]))
    for at in (0, 1, 2):
        if me[at] != other:
            entries.append((LOCATIONS[at], ACCESS[me[at]]))
    if me[4] == me[5]:
        if me[4] != other:
            entries.append(("target_mem", ACCESS[me[4]]))
    else:
        entries += [(LOCATIONS[at], ACCESS[me[at]]) for at in (4, 5) if me[at] != other]
    return entries


def intrinsic(d: dict, name: str, record: dict, defaults: list[str]):
    explicit = [(p["def"], d[p["def"]]) for p in record["IntrProperties"]]
    properties = list(explicit)
    if not record["DisableDefaultAttributes"]:
        properties += [(one, d[one]) for one in defaults]
    flags: set[str] = set()
    throws = False
    no_return = False
    arguments: dict[int, dict] = {}
    for prop, rec in properties:
        supers = rec["!superclasses"]
        kind = prop if not prop.startswith("anonymous_") else next(
            (s for s in supers if s in ARGUMENT_FLAGS or s in ARGUMENT_INTS or s in NOT_ATTRIBUTES
             or s in ("Range", "IntrRead", "IntrWrite")), None)
        if prop.startswith("anonymous_") and kind is None:
            sys.exit(f"gen: {name}: unknown property {prop} of {supers}")
        if kind in FLAGS:
            no_return |= kind == "IntrNoReturn"
            if kind != "IntrWillReturn" or not no_return:
                flags.add(FLAGS[kind])
        elif kind == "Throws":
            throws = True
        elif kind in ARGUMENT_FLAGS:
            arguments.setdefault(rec["ArgNo"], {})[ARGUMENT_FLAGS[kind]] = None
        elif kind in ARGUMENT_INTS:
            key, field = ARGUMENT_INTS[kind]
            arguments.setdefault(rec["ArgNo"], {})[key] = rec[field]
        elif kind == "Range":
            arguments.setdefault(rec["ArgNo"], {})["range"] = (rec["Lower"], rec["Upper"])
        elif kind in ("IntrNoMem", "IntrReadMem", "IntrWriteMem", "IntrArgMemOnly", "IntrInaccessibleMemOnly",
                      "IntrInaccessibleMemOrArgMemOnly", "IntrRead", "IntrWrite") or kind in NOT_ATTRIBUTES:
            pass
        else:
            sys.exit(f"gen: {name}: unknown property {prop}")
    if not throws:
        flags.add("nounwind")
    side_effects = any(p == "IntrHasSideEffects" for p, _ in properties)
    me = memory([(p, r) for p, r in properties], side_effects)
    left = set(flags & UNREPRESENTABLE)
    flags -= UNREPRESENTABLE
    for attrs in arguments.values():
        for one in [key for key in attrs if key in UNREPRESENTABLE]:
            left.add(one)
            del attrs[one]
    function = tuple(sorted(flags))
    memory_text = tuple(render_memory(me)) if me is not None else ()
    args = tuple(
        (at, tuple(sorted(attrs.items())))
        for at, attrs in sorted(arguments.items())
        if attrs
    )
    return (function, memory_text, args), left


def name_of(record: dict) -> str:
    explicit = record["LLVMName"]
    return explicit or "llvm." + record["!name"][4:].replace("_", ".")


def attr_text(key: str, value) -> str:
    if value is None:
        return f'F("{key}")'
    if key == "range":
        return f"R({value[0]}, {value[1]})"
    return f'I("{key}", {value})'


def mir_names() -> set[str]:
    """The `llvm.*` intrinsics `intrinsics.rs` enumerates."""
    text = (ROOT / "crates/ir/llrm-mir/src/intrinsics.rs").read_text()
    return set(re.findall(r'"(llvm\.[a-z][a-z0-9_.]*)"', text))


# The intrinsics ValueTracking reads by name: `IntrinsicId` of
# crates/ir/llrm-mir/src/valuetracking/pattern.rs, less the `llvm.` prefix.
VALUETRACKING_NAMES = """
aarch64.irg aarch64.tagp abs acos amdgcn.make.buffer.rsrc amdgcn.mbcnt.hi amdgcn.mbcnt.lo asin assume atan atan2
bitreverse bswap canonicalize ceil clmul cos cosh ctlz ctpop cttz exp exp10 exp2 experimental.get.vector.length
experimental.guard floor frexp fshl fshr get.active.lane.mask is.fpclass launder.invariant.group llrint log log10 log2
lrint matrix.column.major.load matrix.column.major.store maximum maximumnum maxnum minimum minimumnum minnum modf
nearbyint pdep pext pow powi ptrmask riscv.vsetvli riscv.vsetvlimax rint round roundeven sadd.sat sadd.with.overflow
scmp sin sincos sincospi sinh smax smin smul.fix smul.fix.sat smul.with.overflow smulh sshl.sat ssub.sat
ssub.with.overflow tan tanh threadlocal.address trunc uadd.sat uadd.with.overflow ucmp umax umin umul.fix
umul.fix.sat umul.with.overflow umulh ushl.sat usub.sat usub.with.overflow vscale x86.sse42.crc32.64.64
""".split()


# MIR states these itself: its markers still take the size operand, which Intrinsics.td has dropped, so its
# attribute positions are not MIR's.
MIR_OWN = {"llvm.lifetime.start", "llvm.lifetime.end"}


def emitted() -> set[str]:
    return mir_names() - MIR_OWN | {"llvm." + one for one in VALUETRACKING_NAMES}


def build(d: dict, keep: set[str] | None = None):
    """The intrinsics in `keep` (all, if none): each name -> its attribute set's index and whether it is
    overloaded; the sets; and, per attribute left out, its count in the whole table and the kept names it hits."""
    defaults = [
        k
        for k, v in d.items()
        if isinstance(v, dict)
        and "IntrinsicProperty" in v.get("!superclasses", [])
        and not k.startswith("anonymous_")
        and v.get("IsDefault")
    ]
    every: dict[str, tuple] = {}
    for record in d.values():
        if not isinstance(record, dict) or "Intrinsic" not in record.get("!superclasses", []):
            continue
        name = name_of(record)
        if name in every:
            sys.exit(f"gen: {name} is defined twice")
        overloaded = bool(d[record["TypeInfo"]["def"]]["isOverloaded"])
        every[name] = (*intrinsic(d, name, record, defaults), overloaded)
    if keep is not None and keep - every.keys():
        sys.exit(f"gen: not in Intrinsics.td: {sorted(keep - every.keys())}")
    deviations: dict[str, tuple[int, list[str]]] = {}
    for kind in sorted(UNREPRESENTABLE):
        hit = sorted(name for name, (_, left, _) in every.items() if kind in left)
        deviations[kind] = (len(hit), [name for name in hit if keep is None or name in keep])
    sets: dict[tuple, int] = {}
    names: dict[str, tuple[int, bool]] = {}
    for name, (one, _, overloaded) in sorted(every.items()):
        if keep is None or name in keep:
            names[name] = (sets.setdefault(one, len(sets)), overloaded)
    return names, list(sets), deviations, len(every)


def llvm_text(key: str, value) -> str:
    """`Attribute::getAsString`, ranges at i32."""
    if key == "range":
        low, high = ((one + (1 << 31)) % (1 << 32) - (1 << 31) for one in value)
        return f"range(i32 {low}, {high})"
    if value is None:
        return "captures(none)" if key == "nocapture" else key
    return f"align {value}" if key == "align" else f"{key}({value})"


def llvm_lines(names: dict[str, tuple[int, bool]], sets: list[tuple]) -> set[str]:
    """What `attrdump.cpp` prints for the same intrinsics: `name index attribute`."""

    lines = set()
    for name, (at, _) in names.items():
        function, memory_text, args = sets[at]
        for flag in function:
            lines.add(f"{name} f {flag}")
        if memory_text:
            parts = [access if loc is None else f"{loc}: {access}" for loc, access in memory_text]
            lines.add(f"{name} f memory({', '.join(parts)})")
        for index, attrs in args:
            where = "r" if index == 0 else str(index)
            for key, value in attrs:
                lines.add(f"{name} {where} {llvm_text(key, value)}")
    return lines


def generate(everything: bool = False) -> str:
    names, sets, deviations, total = build(tblgen(), None if everything else emitted())
    lines = [
        "// @generated by tools/intrinsics/gen.py from Intrinsics.td; do not edit.",
        f"// LLVM {COMMIT}: {len(names)} of {total} intrinsics, {len(sets)} attribute sets.",
        "// `nocapture` is LLVM's `captures(none)`. Arguments are by attribute",
        "// index: 0 is the return, n the nth parameter.",
        "//",
        "// DEVIATE: attributes MIR's `Attribute` cannot say are left out:",
    ]
    for number, (kind, (count, hit)) in enumerate(deviations.items(), 1):
        who = f"{len(hit)} of those here" if everything else (", ".join(hit) or "none of those here")
        text = f"{number}. {kind}: {count} of {total} in the whole table; affects {who}."
        lines += textwrap.wrap(text, 78, initial_indent="//   ", subsequent_indent="//      ")
    lines += [
        "",
        "#[allow(dead_code)] // `Int` is only in the whole table",
        "#[derive(Clone, Copy, Debug)]",
        "pub enum Attr {",
        "    Flag(&'static str),",
        "    Int(&'static str, u64),",
        "    /// `[lower, upper)`, of the argument's own width.",
        "    Range(i64, i64),",
        "}",
        "#[allow(unused_imports)]",
        "use Attr::{Flag as F, Int as I, Range as R};",
        "",
        "pub struct Set {",
        "    pub function: &'static [&'static str],",
        "    pub memory: &'static [(Option<&'static str>, &'static str)],",
        "    pub arguments: &'static [(u8, &'static [Attr])],",
        "}",
        "",
        f"pub static SETS: [Set; {len(sets)}] = [",
    ]
    for function, memory_text, args in sets:
        function_text = ", ".join(f'"{one}"' for one in function)
        memory_out = ", ".join(
            '({}, "{}")'.format("None" if loc is None else 'Some("%s")' % loc, acc) for loc, acc in memory_text
        )
        args_out = ", ".join(f"({at}, &[{', '.join(attr_text(k, v) for k, v in attrs)}])" for at, attrs in args)
        lines.append(f"    Set {{ function: &[{function_text}], memory: &[{memory_out}], arguments: &[{args_out}] }},")
    lines += ["];", "", "/// Sorted by name: the set, and whether a type suffix may follow the name.", f"pub static NAMES: [(&str, u16, bool); {len(names)}] = ["]
    lines += [f'    ("{name}", {names[name][0]}, {str(names[name][1]).lower()}),' for name in sorted(names)]
    lines += ["];", ""]
    return "\n".join(lines)


if __name__ == "__main__":
    if "--all" in sys.argv:
        Path(sys.argv[sys.argv.index("--all") + 1]).write_text(generate(True))
    elif "--stdout" in sys.argv:
        sys.stdout.write(generate())
    else:
        OUT.write_text(generate())
