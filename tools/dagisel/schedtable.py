#!/usr/bin/env python3
"""Reads the pin's AArch64 scheduling models into Rust tables (`sched.rs`).

    schedtable.py --subtarget GenSubtarget.inc --instrinfo GenInstrInfo.inc --json target.json \
        --mcschedule MCSchedule.h --dir GENERATED_DIR --out sched.rs CPU...

The models are the ones `llvm-tblgen -gen-subtarget` resolves (`MCSchedModel`, `MCSchedClassDesc`,
the write/read tables, `resolveVariantSchedClassImpl`); each CPU is the model its `ProcessorModel`
names. One row per opcode the DAG tables emit (`GENERATED_DIR/instrinfo.rs`, named by `ids.rs`):
its scheduling class, which is the same number in every model. Nothing is defaulted: a predicate
or statement of a variant class that this script does not read, a table index past its table, a
resource that does not exist, or an opcode a complete model leaves without a class stops it with a
`SchedError`.
"""

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

INVALID_MICRO_OPS = (1 << 13) - 1
VARIANT_MICRO_OPS = INVALID_MICRO_OPS - 1

# `AArch64SchedPredicates.td` TIIPredicates a variant class may test: the pin's text -> our variant.
PREDICATES = {
    "AArch64_MC::hasShiftedReg(*MI)": "HasShiftedReg",
    "AArch64_MC::isScaledAddr(*MI)": "IsScaledAddr",
}

MODEL_FIELDS = [
    "IssueWidth", "MicroOpBufferSize", "LoopMicroOpBufferSize", "LoadLatency", "HighLatency",
    "MispredictPenalty", "PostRAScheduler", "CompleteModel", "EnableIntervals", "Processor ID",
    "ProcResources", "SchedClasses",
]  # fmt: skip
DEFAULT_OF = {
    "IssueWidth": "DefaultIssueWidth",
    "MicroOpBufferSize": "DefaultMicroOpBufferSize",
    "LoopMicroOpBufferSize": "DefaultLoopMicroOpBufferSize",
    "LoadLatency": "DefaultLoadLatency",
    "HighLatency": "DefaultHighLatency",
    "MispredictPenalty": "DefaultMispredictPenalty",
}


class SchedError(Exception):
    pass


@dataclass
class Resource:
    name: str
    num_units: int
    super_idx: int
    buffer_size: int
    sub_units: list


@dataclass
class ClassRow:
    name: str
    micro_ops: int
    begin_group: bool
    end_group: bool
    retire_ooo: bool
    read_advance_idx: int
    write_proc_res_idx: int
    write_latency_idx: int
    num_read_advance: int
    num_write_proc_res: int
    num_write_latency: int

    @property
    def invalid(self):
        return self.micro_ops == INVALID_MICRO_OPS

    @property
    def variant(self):
        return self.micro_ops == VARIANT_MICRO_OPS


@dataclass
class Model:
    name: str
    issue_width: int
    micro_op_buffer_size: int
    loop_micro_op_buffer_size: int
    load_latency: int
    high_latency: int
    mispredict_penalty: int
    post_ra_scheduler: bool
    complete_model: bool
    cpuid: int
    resources: list = field(default_factory=list)
    classes: list = field(default_factory=list)


@dataclass
class Instr:
    number: int
    sched_class: int
    pseudo: bool


def parse_defaults(header):
    """`MCSchedModel::Default*` from MCSchedule.h."""
    found = {m.group(1): int(m.group(2)) for m in re.finditer(r"static constexpr unsigned (Default\w+) = (\d+);", header)}
    missing = sorted(set(DEFAULT_OF.values()) - set(found))
    if missing:
        raise SchedError(f"MCSchedule.h does not give {', '.join(missing)}")
    return found


def section(text, start, what):
    """The text from `start` to the `};` that closes it."""
    at = text.find(start)
    if at < 0:
        raise SchedError(f"no {what}")
    return text[at : text.index("\n};", at)]


def rows(text, count):
    pattern = re.compile(r"^\s*\{\s*" + r",\s*".join([r"(-?\d+)"] * count) + r"\s*\}", re.M)
    return [tuple(int(v) for v in m.groups()) for m in pattern.finditer(text)]


def parse_models(sub, defaults, only=None):
    """The `MCSchedModel`s of the file by name (all, or those in `only`), with their resources and class rows."""
    models = {}
    body = section(sub, "extern const llvm::MCSchedModel AArch64SchedModels[] = {", "AArch64SchedModels")
    for m in re.finditer(r"\n\{ // (\w+)\n(.*?)\n  \},", body + "\n  },", re.S):
        name, fields = m.groups()
        if only is not None and name not in only:
            continue
        values = []
        for line in fields.splitlines():
            line = line.strip()
            if not line:
                continue
            value, _, label = line.partition(",")
            comment = line.split("//", 1)[1].strip() if "//" in line else ""
            values.append((value.strip(), comment))
        if len(values) < len(MODEL_FIELDS):
            continue  # NoSchedModel and its kin carry no tables
        raw = {}
        for (value, comment), label in zip(values, MODEL_FIELDS):
            if comment and label != "Processor ID" and comment.split()[0] != label:
                raise SchedError(f"{name}: expected {label}, found {comment}")
            if value.startswith("MCSchedModel::"):
                value = defaults[value.removeprefix("MCSchedModel::")]
            raw[label] = value
        model = Model(
            name=name,
            issue_width=int(raw["IssueWidth"]),
            micro_op_buffer_size=int(raw["MicroOpBufferSize"]),
            loop_micro_op_buffer_size=int(raw["LoopMicroOpBufferSize"]),
            load_latency=int(raw["LoadLatency"]),
            high_latency=int(raw["HighLatency"]),
            mispredict_penalty=int(raw["MispredictPenalty"]),
            post_ra_scheduler=raw["PostRAScheduler"] == "true",
            complete_model=raw["CompleteModel"] == "true",
            cpuid=int(raw["Processor ID"]),
        )
        if raw["ProcResources"] != f"{name}ProcResources" or raw["SchedClasses"] != f"{name}SchedClasses":
            raise SchedError(f"{name}: tables named {raw['ProcResources']} and {raw['SchedClasses']}")
        model.resources = parse_resources(sub, name)
        model.classes = parse_classes(sub, name)
        models[name] = model
    return models


def parse_resources(sub, name):
    subunits_text = section(sub, f"static const unsigned {name}ProcResourceSubUnits[] = {{", f"{name} sub-units")
    subunits = [int(v) for line in subunits_text.splitlines()[1:] for v in re.findall(r"-?\d+", line.split("//")[0])]
    text = section(sub, f"static const llvm::MCProcResourceDesc {name}ProcResources[] = {{", f"{name} resources")
    out = []
    for m in re.finditer(r'\{"(\w+)",\s*(\d+),\s*(\d+),\s*(-?\d+),\s*(0|nullptr|\w+ \+ (\d+))\}', text):
        unit, units, sup, buf, ptr, begin = m.groups()
        listed = subunits[int(begin) : int(begin) + int(units)] if begin else []
        if begin and (len(listed) != int(units) or any(s >= len(out) + 64 for s in listed)):
            raise SchedError(f"{name}: sub-units of {unit} run past their list")
        out.append(Resource(unit, int(units), int(sup), int(buf), listed))
    if not out or out[0].name != "InvalidUnit":
        raise SchedError(f"{name}: resources do not start at InvalidUnit")
    for index, resource in enumerate(out):
        if resource.super_idx >= len(out) or any(s >= len(out) for s in resource.sub_units):
            raise SchedError(f"{name}: resource {index} names a resource past the table")
    return out


def parse_classes(sub, name):
    text = section(sub, f"static const llvm::MCSchedClassDesc {name}SchedClasses[] = {{", f"{name} classes")
    pattern = re.compile(
        r"\{DBGFIELD\((?:/\*([^*]+)\*/ )?\d+\)\s+(\d+), (true|false), (true|false), (true|false),"
        + r"\s*(\d+),\s*(\d+),\s*(\d+),\s*(\d+),\s*(\d+),\s*(\d+)\}"
    )
    out = []
    for m in pattern.finditer(text):
        label, micro, bg, eg, ret, *idx = m.groups()
        out.append(
            ClassRow(label or "", int(micro), bg == "true", eg == "true", ret == "true", *[int(i) for i in idx])
        )
    if len(re.findall(r"DBGFIELD", text)) != len(out):
        raise SchedError(f"{name}: a class row is in a form this script does not read")
    return out


def parse_instr_classes(text):
    """`opcode name -> Instr` from `-gen-instr-info`: the pin's number, the class number and whether it is a pseudo."""
    pattern = re.compile(
        r"^\s*\{ (\d+),\s*\d+,\s*\d+,\s*\d+,\s*(\d+),\s*\d+,\s*\d+,\s*\w+OpInfoBase \+ \d+,\s*\d+,\s*(.*?),\s*0x[0-9a-fA-F]+ULL \},\s*// (\w+)$",
        re.M,
    )
    return {
        m.group(4): Instr(int(m.group(1)), int(m.group(2)), "MCID::Pseudo" in m.group(3))
        for m in pattern.finditer(text)
    }


def parse_variants(sub, model):
    """`{class: [(predicate or None, class)]}` of the model's variant classes, in the pin's order."""
    start = sub.index("unsigned resolveVariantSchedClassImpl(unsigned SchedClass,")
    body = sub[start : sub.index("\n}\n", start)]
    out = {}
    parts = re.split(r"\n  case (\d+): // [^\n]*\n", body)
    for at in range(1, len(parts), 2):
        class_id, block = int(parts[at]), "\n" + parts[at + 1]
        found = re.search(rf"\n    if \(CPUID == {model.cpuid}\) \{{[^\n]*\n(.*?)\n    \}}\n", block, re.S)
        if not found:
            continue
        arms, rest = [], found.group(1)
        pattern = re.compile(r"(?:\s*if \((.*?)\)\s*\n)?\s*return (\d+);[^\n]*", re.S)
        pos = 0
        while pos < len(rest):
            m = pattern.match(rest, pos)
            if not m:
                raise SchedError(f"{model.name}: variant class {class_id} has a statement this script does not read")
            predicate = m.group(1)
            if predicate is not None:
                predicate = re.sub(r"\s+", " ", predicate).strip()
            arms.append((predicate, int(m.group(2))))
            pos = m.end()
        out[class_id] = arms
    return out


def invalid_kinds(model, opcode_class, unsupported):
    """`{class: 'Unsupported' | 'Unmodeled'}` of the invalid classes the opcodes use: the model's own
    `Unsupported` write, or a gap. A class whose opcodes disagree stops the script."""
    seen = {}
    for opcode, instr in opcode_class.items():
        if model.classes[instr.sched_class].invalid and not instr.pseudo:
            seen.setdefault(instr.sched_class, set()).add(opcode in unsupported)
    out = {}
    for class_id, answers in seen.items():
        if len(answers) != 1:
            raise SchedError(f"{model.name}: class {class_id} is unsupported for some of its opcodes and not for others")
        out[class_id] = "Unsupported" if answers == {True} else "Unmodeled"
    return out


def unsupported_opcodes(data, model_names, opcodes):
    """`{model: {opcode}}`: the opcodes with a write the model's `WriteRes` marks `Unsupported`."""
    writes = {name: set() for name in model_names}
    for record in data.values():
        if not isinstance(record, dict) or "WriteRes" not in record.get("!superclasses", []):
            continue
        model = record.get("SchedModel", {}).get("def")
        if model in writes and record.get("Unsupported"):
            write = record["WriteType"]
            writes[model].add(write["def"] if isinstance(write, dict) else write)
    out = {}
    for name in model_names:
        out[name] = {
            opcode
            for opcode in opcodes
            if any(w.get("def") in writes[name] for w in data.get(opcode, {}).get("SchedRW") or [])
        }
    return out


def rust_name(name):
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", name).upper()


def reachable(model, wanted, variants):
    """The class ids `wanted` opcodes reach in `model`, variants followed, each checked."""
    seen, order = set(), []
    pending = list(wanted)
    while pending:
        class_id = pending.pop()
        if class_id in seen:
            continue
        if class_id >= len(model.classes):
            raise SchedError(f"{model.name}: class {class_id} is past the table")
        seen.add(class_id)
        order.append(class_id)
        if model.classes[class_id].variant:
            if class_id not in variants:
                raise SchedError(f"{model.name}: variant class {class_id} has no arms for this model")
            for predicate, target in variants[class_id]:
                if predicate is not None and predicate not in PREDICATES:
                    raise SchedError(f"{model.name}: class {class_id} tests a predicate this script does not read: {predicate}")
                if model.classes[target].variant:
                    raise SchedError(f"{model.name}: variant class {class_id} leads to variant class {target}")
                pending.append(target)
    return sorted(order)


def tables(sub):
    wpr = rows(section(sub, "extern const llvm::MCWriteProcResEntry AArch64WriteProcResTable[] = {", "write-proc-res table"), 3)
    wl = rows(section(sub, "extern const llvm::MCWriteLatencyEntry AArch64WriteLatencyTable[] = {", "write-latency table"), 2)
    ra = rows(section(sub, "extern const llvm::MCReadAdvanceEntry AArch64ReadAdvanceTable[] = {", "read-advance table"), 3)
    return wpr, wl, ra


class Pool:
    """The entries every class slices, each distinct slice once and shared across models."""

    def __init__(self, what):
        self.what = what
        self.items = []
        self.starts = {}

    def span(self, entries):
        """`(at, len)` of `entries` in the pool, added when new."""
        entries = tuple(entries)
        if not entries:
            return 0, 0
        if len(entries) > 255:
            raise SchedError(f"a class has {len(entries)} {self.what} entries; the table's span holds 255")
        if entries not in self.starts:
            if len(self.items) + len(entries) > 65535:
                raise SchedError(f"the {self.what} pool is past 65535 entries")
            self.starts[entries] = len(self.items)
            self.items.extend(entries)
        return self.starts[entries], len(entries)

    def render(self, name, rust_type):
        body = "".join(f"    {item},\n" for item in self.items)
        return f"static {name}: [{rust_type}; {len(self.items)}] = [\n{body}];\n"


class Pools:
    def __init__(self):
        self.proc = Pool("WriteProcRes")
        self.latency = Pool("WriteLatency")
        self.advance = Pool("ReadAdvance")
        self.arms = Pool("VariantArm")
        self.rows = Pool("ClassRow")


FLAGS = (("begin_group", 1), ("end_group", 2), ("retire_ooo", 4))


def render_class(model, class_id, wpr, wl, ra, pools):
    row = model.classes[class_id]
    for what, begin, count, table in (
        ("WriteProcRes", row.write_proc_res_idx, row.num_write_proc_res, wpr),
        ("WriteLatency", row.write_latency_idx, row.num_write_latency, wl),
        ("ReadAdvance", row.read_advance_idx, row.num_read_advance, ra),
    ):
        if begin + count > len(table):
            raise SchedError(f"{model.name}: class {class_id} runs past the {what} table")
    resources = len(model.resources)
    proc = []
    for resource, release, acquire in wpr[row.write_proc_res_idx : row.write_proc_res_idx + row.num_write_proc_res]:
        if not 0 < resource < resources:
            raise SchedError(f"{model.name}: class {class_id} uses resource {resource}, which does not exist")
        proc.append(f"WriteProcRes {{ resource: {resource}, release_at_cycle: {release}, acquire_at_cycle: {acquire} }}")
    lat = [
        f"WriteLatency {{ write_idx: {write}, cycles: {cycles} }}"
        for cycles, write in wl[row.write_latency_idx : row.write_latency_idx + row.num_write_latency]
    ]
    adv = [
        f"ReadAdvance {{ use_idx: {use}, write_idx: {write}, cycles: {cycles} }}"
        for use, write, cycles in ra[row.read_advance_idx : row.read_advance_idx + row.num_read_advance]
    ]
    flags = sum(bit for field, bit in FLAGS if getattr(row, field))
    (proc_at, proc_len), (lat_at, lat_len), (adv_at, adv_len) = (
        pools.proc.span(proc),
        pools.latency.span(lat),
        pools.advance.span(adv),
    )
    text = (
        f"ClassRow {{ num_micro_ops: {row.micro_ops}, flags: {flags}, "
        f"proc_at: {proc_at}, proc_len: {proc_len}, lat_at: {lat_at}, lat_len: {lat_len}, "
        f"adv_at: {adv_at}, adv_len: {adv_len} }}"
    )
    return f"SchedClass::Resolved({pools.rows.span([text])[0]})"


def render_model(model, wanted, sub, wpr, wl, ra, kinds, pools, pool_names):
    variants = parse_variants(sub, model)
    ids = reachable(model, wanted, variants)
    lines = []
    for class_id in ids:
        row = model.classes[class_id]
        if row.invalid:
            body = f"SchedClass::{kinds.get(class_id, 'Unmodeled')}"
        elif row.variant:
            arms = [
                f"VariantArm {{ pred: {'None' if p is None else 'Some(SchedPred::' + PREDICATES[p] + ')'}, class: {t} }}"
                for p, t in variants[class_id]
            ]
            at, length = pools.arms.span(arms)
            body = f"SchedClass::Variant {{ at: {at}, len: {length} }}"
        else:
            body = render_class(model, class_id, wpr, wl, ra, pools)
        lines.append(f"        ({class_id}, {body}),")
    resources = []
    for r in model.resources:
        subs = ", ".join(str(s) for s in r.sub_units)
        resources.append(
            f"        ProcResource {{ name: {json.dumps(r.name)}, num_units: {r.num_units}, super_idx: {r.super_idx}, "
            f"buffer_size: {r.buffer_size}, sub_units: &[{subs}] }},"
        )
    flag = lambda v: str(v).lower()
    return (
        f"pub static {rust_name(model.name)}: SchedModel = SchedModel {{\n"
        f"    name: {json.dumps(model.name)},\n"
        f"    issue_width: {model.issue_width},\n"
        f"    micro_op_buffer_size: {model.micro_op_buffer_size},\n"
        f"    loop_micro_op_buffer_size: {model.loop_micro_op_buffer_size},\n"
        f"    load_latency: {model.load_latency},\n"
        f"    high_latency: {model.high_latency},\n"
        f"    mispredict_penalty: {model.mispredict_penalty},\n"
        f"    post_ra_scheduler: {flag(model.post_ra_scheduler)},\n"
        f"    complete_model: {flag(model.complete_model)},\n"
        f"    opcode_class: &OPCODE_CLASS,\n"
        f"    write_proc_res: &{pool_names[0]},\n"
        f"    write_latency: &{pool_names[1]},\n"
        f"    read_advance: &{pool_names[2]},\n"
        f"    variant_arms: &{pool_names[3]},\n"
        f"    class_rows: &{pool_names[4]},\n"
        f"    resources: &[\n" + "\n".join(resources) + "\n    ],\n"
        f"    classes: &[\n" + "\n".join(lines) + "\n    ],\n"
        f"}};\n"
    )


def render(models, cpu_models, instrs, sub, numbers, unsupported=None, regen="tools/dagisel/regen-target.sh"):
    """The text of `sched.rs`: `numbers` {opcode name: opcode number}, `cpu_models` {cpu: model name},
    `unsupported` {model name: opcodes whose writes the model marks Unsupported}."""
    unsupported = unsupported or {}
    wpr, wl, ra = tables(sub)
    chosen = sorted(set(cpu_models.values()))
    for name in chosen:
        if name not in models:
            raise SchedError(f"the pin has no model {name}")
    opcode_class = {}
    for opcode in numbers:
        if opcode not in instrs:
            raise SchedError(f"{opcode} is in no -gen-instr-info row")
        opcode_class[opcode] = instrs[opcode]
    for name in chosen:
        model = models[name]
        if not model.complete_model:
            continue
        for opcode, instr in opcode_class.items():
            if not instr.pseudo and model.classes[instr.sched_class].invalid and opcode not in unsupported.get(name, ()):
                raise SchedError(f"{name} is a complete model and gives {opcode} no class")
    out = [
        "//! The pin's scheduling models of the target's CPUs, and the class of each opcode the DAG tables emit.\n",
        "//!\n",
        f"//! Generated by `{regen}`; do not edit.\n\n",
        "use crate::sched::*;\n\n",
        "/// The model the pin's `ProcessorModel` of this CPU names.\n",
        "pub fn model(cpu: &str) -> Option<&'static SchedModel> {\n    match cpu {\n",
    ]
    for cpu in sorted(cpu_models):
        out.append(f"        {json.dumps(cpu)} => Some(&{rust_name(cpu_models[cpu])}),\n")
    out.append("        _ => None,\n    }\n}\n\n")
    out.append("/// `(opcode, scheduling class)`, by opcode; the class number is the same in every model.\n")
    out.append(f"static OPCODE_CLASS: [(u16, u16); {len(numbers)}] = [\n")
    for name in sorted(numbers, key=numbers.get):
        out.append(f"    ({numbers[name]}, {instrs[name].sched_class}),\n")
    out.append("];\n\n")
    pools = Pools()
    pool_names = ("WRITE_PROC_RES", "WRITE_LATENCY", "READ_ADVANCE", "VARIANT_ARMS", "CLASS_ROWS")
    rendered = []
    for name in chosen:
        wanted = sorted({instr.sched_class for instr in opcode_class.values()})
        kinds = invalid_kinds(models[name], opcode_class, unsupported.get(name, ()))
        rendered.append(render_model(models[name], wanted, sub, wpr, wl, ra, kinds, pools, pool_names))
    out.append("/// The pin's `MCWriteProcResEntry`, `MCWriteLatencyEntry`, `MCReadAdvanceEntry`, variant arms and distinct class rows the classes below slice;\n")
    out.append("/// a slice the models share is stored once.\n")
    for pool, name, rust_type in zip((pools.proc, pools.latency, pools.advance, pools.arms, pools.rows), pool_names, ("WriteProcRes", "WriteLatency", "ReadAdvance", "VariantArm", "ClassRow")):
        out.append(pool.render(name, rust_type))
        out.append("\n")
    for text in rendered:
        out.append(text)
        out.append("\n")
    return "".join(out).rstrip("\n") + "\n"


def opcode_numbers(directory, instrs):
    """`{name: number}` of the instructions `instrinfo.rs` has rows for, named by `-gen-instr-info`.

    The generic opcodes (`COPY`, `REG_SEQUENCE`) are in no `ids.rs` opcode module, so the names come from the
    pin's rows; where `ids.rs` also names one, the two numbers must agree."""
    by_number = {instr.number: name for name, instr in instrs.items()}
    wanted = [int(m.group(1)) for m in re.finditer(r"^\s+\((\d+), InstrDesc ", (directory / "instrinfo.rs").read_text(), re.M)]
    missing = [n for n in wanted if n not in by_number]
    if missing:
        raise SchedError(f"instrinfo.rs has opcodes {missing[:5]} that -gen-instr-info does not")
    ids = (directory / "ids.rs").read_text()
    block = ids[ids.index("pub mod opcode {") :]
    for m in re.finditer(r"pub const (\w+): u16 = (\d+);", block[: block.index("\n}")]):
        if m.group(1) in instrs and instrs[m.group(1)].number != int(m.group(2)):
            raise SchedError(f"{m.group(1)} is {m.group(2)} in ids.rs and {instrs[m.group(1)].number} in -gen-instr-info")
    return {by_number[n]: n for n in wanted}


def processors(data, cpus):
    """`{cpu: model name}` from the pin's `ProcessorModel` and `ProcessorAlias` records."""
    named = {}
    aliases = {}
    for record in data.values():
        if not isinstance(record, dict):
            continue
        classes = record.get("!superclasses", [])
        if "Processor" in classes and "SchedModel" in record:
            named[record["Name"]] = record["SchedModel"]["def"]
        elif "ProcessorAlias" in classes:
            aliases[record["Name"]] = record["Alias"]
    out = {}
    for cpu in cpus:
        target = aliases.get(cpu, cpu)
        if target not in named:
            raise SchedError(f"the pin has no processor {cpu}")
        out[cpu] = named[target]
    return out


def main():
    parser = argparse.ArgumentParser()
    for flag in ("subtarget", "instrinfo", "json", "mcschedule", "dir", "out"):
        parser.add_argument(f"--{flag}", required=True)
    parser.add_argument("cpus", nargs="+")
    args = parser.parse_args()
    sub = Path(args.subtarget).read_text()
    defaults = parse_defaults(Path(args.mcschedule).read_text())
    data = json.loads(Path(args.json).read_text())
    cpu_models = processors(data, args.cpus)
    models = parse_models(sub, defaults, set(cpu_models.values()))
    instrs = parse_instr_classes(Path(args.instrinfo).read_text())
    numbers = opcode_numbers(Path(args.dir), instrs)
    unsupported = unsupported_opcodes(data, set(cpu_models.values()), numbers)
    Path(args.out).write_text(render(models, cpu_models, instrs, sub, numbers, unsupported))


if __name__ == "__main__":
    try:
        main()
    except SchedError as error:
        sys.exit(f"schedtable.py: {error}")
