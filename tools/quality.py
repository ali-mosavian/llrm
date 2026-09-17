"""Measure emitted C functions without manufacturing a performance target.

The hard rule is that a target is external evidence, not the candidate with a
smaller number written beside it.  This tool therefore reports complete raw
metrics with ``target_status=missing`` until an audited target names its
evidence and a different normalized assembly hash.

    uv run python tools/quality.py fixtures/c/halve.cgs --cpu P5
    uv run python tools/quality.py bench/c/nbody.c --cpu all --references --dump build/quality
"""

import re
import json
import shutil
import hashlib
import argparse
import subprocess
from pathlib import Path

from iced_x86 import Decoder
from iced_x86 import Formatter
from iced_x86 import FormatterSyntax

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import masm
from qbopt.cycles import cycles
from qbopt.analysis import loops
from qbopt.backend import allocate
from qbopt.backend import omfwrite
from qbopt.backend import cpu as targets
from qbopt.cfront import compile as cfront

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_TARGETS = ROOT / "bench" / "c" / "targets.json"
FORMAT = Formatter(FormatterSyntax.MASM)
REFERENCE_COMPILERS = ("clang", "i686-elf-gcc")
# Open Watcom accepts these memory-model and calling-convention qualifiers as
# keywords. GCC and Clang do not; erase only their spelling so the reference
# remains a structural 32-bit comparison rather than failing before codegen.
REFERENCE_QUALIFIERS = ("near", "far", "huge", "cdecl", "pascal")
STRUCTURAL_METRICS = ("instructions", "loads", "stores", "branches", "calls", "address_calculations")

_FUNCTION_TYPE = re.compile(r'^\.type\s+"?([^",]+)"?\s*,\s*[@%]function$')


def _normalized_hash(instructions: tuple[str, ...]) -> str:
    """A stable identity for a reference function's instruction stream."""
    return hashlib.sha256(("\n".join(instructions) + "\n").encode()).hexdigest()


def _reference_memory(mnemonic: str, operands: str) -> tuple[int, int]:
    """Conservative load/store counts for normalized Intel-syntax assembly.

    These are structural comparison metrics, not a dependence model.  A
    read/modify/write memory destination counts once on each side, while LEA
    is recorded separately as address calculation rather than as a load.
    """
    if "[" not in operands or mnemonic == "lea":
        return 0, 0
    parts = [one.strip() for one in operands.split(",")]
    first_memory = bool(parts and "[" in parts[0])
    later_memory = any("[" in one for one in parts[1:])
    if not first_memory:
        return int(later_memory), 0
    pure_store = mnemonic.startswith(("mov", "fst", "fist", "fnst", "stmx")) or mnemonic == "pop"
    read_only = mnemonic.startswith(
        ("cmp", "test", "fld", "fild", "fadd", "fsub", "fmul", "fdiv", "fiadd", "fisub", "fimul", "fidiv")
    ) or mnemonic in {"push", "call", "jmp"}
    loads = int(later_memory or read_only or not pure_store)
    stores = int(not read_only)
    return loads, stores


def _reference_functions(assembly: str) -> list[dict]:
    """Measure each explicitly delimited function in GCC/Clang assembly."""
    declared: set[str] = set()
    current: str | None = None
    functions: list[dict] = []
    instructions: list[str] = []

    def finish() -> None:
        nonlocal current, instructions
        if current is None:
            return
        loads = stores = 0
        for line in instructions:
            mnemonic, _, operands = line.partition(" ")
            read, written = _reference_memory(mnemonic, operands)
            loads += read
            stores += written
        functions.append(
            {
                "name": current,
                "instructions": len(instructions),
                "loads": loads,
                "stores": stores,
                "branches": sum(line.split(None, 1)[0].startswith("j") for line in instructions),
                "calls": sum(line.split(None, 1)[0] == "call" for line in instructions),
                "address_calculations": sum(line.split(None, 1)[0] == "lea" for line in instructions),
                "normalized_sha256": _normalized_hash(tuple(instructions)),
            }
        )
        current, instructions = None, []

    for raw in assembly.splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        if found := _FUNCTION_TYPE.match(line):
            declared.add(found.group(1))
            continue
        if line.endswith(":") and line[:-1].strip('"') in declared:
            finish()
            current = line[:-1].strip('"')
            continue
        if current is None:
            continue
        if line.startswith(".size"):
            finish()
            continue
        if line.startswith(".") or line.endswith(":"):
            continue
        instructions.append(" ".join(line.lower().split()))
    finish()
    return functions


class UntrustedTarget(ValueError):
    """A denominator lacks independent, auditable evidence."""


class InvalidMeasurement(ValueError):
    """The emitted byte extent cannot be measured completely."""


def _version(command: str) -> str | None:
    path = shutil.which(command)
    if path is None:
        return None
    done = subprocess.run([path, "--version"], capture_output=True, text=True)
    return done.stdout.splitlines()[0] if done.returncode == 0 and done.stdout else None


def _revision() -> str | None:
    done = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True)
    return done.stdout.strip() if done.returncode == 0 else None


def _image(module: masm.Module, procedure: masm.Procedure, number: int) -> tuple[bytes, dict[str, int]]:
    """The exact selected bytes and labels, with relocations normalized to zero."""
    encoded = []
    for item in masm.listing(procedure, number):
        encoded.extend(omfwrite._items(item, module.names, number))
    labels = omfwrite._relaxed(encoded)
    out = bytearray()
    for item in encoded:
        match item:
            case masm.Label():
                continue
            case omfwrite.Piece(code=code):
                out.extend(code)
            case omfwrite.Jump(name=name, label=label, long=long):
                out.extend(omfwrite._jump(name, labels[label], len(out), long).code)
            case omfwrite.Near():
                out.extend(b"\xe8\x00\x00")
    return bytes(out), labels


def _blob(module: masm.Module, procedure: masm.Procedure, number: int) -> bytes:
    """The exact selected bytes, with relocation fields normalized to zero."""
    return _image(module, procedure, number)[0]


def _rows(code: bytes) -> list[tuple[str, str, str]]:
    if not code:
        raise InvalidMeasurement("function emitted no bytes")
    rows = []
    covered = 0
    for instruction in Decoder(16, code):
        if instruction.is_invalid:
            raise InvalidMeasurement(f"invalid instruction at byte {instruction.ip}")
        text = FORMAT.format(instruction).lower()
        mnemonic, _, operands = text.partition(" ")
        raw = code[instruction.ip : instruction.ip + instruction.len].hex()
        rows.append((raw, mnemonic, operands))
        covered += instruction.len
    if covered != len(code):
        raise InvalidMeasurement(f"decoded {covered} of {len(code)} function bytes")
    return rows


def _peak_live(procedure: masm.Procedure) -> int:
    _live_in, live_out = allocate.live(procedure.body)
    peak = 0
    for block in procedure.body.blocks:
        alive = set(live_out[block.at])
        peak = max(peak, len(alive))
        for instruction in reversed(block.insns):
            alive.difference_update(instruction.defines)
            alive.update(instruction.uses)
            peak = max(peak, len(alive))
    return peak


def _memory(rows: list[tuple[str, str, str]]) -> tuple[int, int]:
    loads = stores = 0
    for _raw, mnemonic, operands in rows:
        read, written = _reference_memory(mnemonic, operands)
        loads += read
        stores += written
    return loads, stores


def _transitions(body: lir.LirBody) -> dict[int, dict[int, float]]:
    """Estimated successor probabilities from CFG shape alone.

    Ordinary alternatives divide evenly.  A branch that either remains in a
    natural loop or exits it assigns nine tenths to continuing: the same
    profile-free ten-iteration convention used by spill weighting.
    """
    known = {block.at for block in body.blocks}
    natural = loops.loops(body.blocks, body.entry)
    out: dict[int, dict[int, float]] = {}
    for block in body.blocks:
        successors = tuple(one for one in block.succ if one in known)
        if not successors:
            out[block.at] = {}
            continue
        split = None
        for loop in natural:
            if block.at not in loop.body:
                continue
            inside = tuple(one for one in successors if one in loop.body)
            outside = tuple(one for one in successors if one not in loop.body)
            if inside and outside:
                split = {
                    **{one: 0.9 / len(inside) for one in inside},
                    **{one: 0.1 / len(outside) for one in outside},
                }
                break
        out[block.at] = split or {one: 1.0 / len(successors) for one in successors}
    return out


def _frequencies(body: lir.LirBody) -> dict[int, float] | None:
    """Profile-free expected executions of each block per function entry."""
    if loops.irreducible(body.blocks, body.entry):
        return None
    transitions = _transitions(body)
    nodes = tuple(transitions)
    index = {at: position for position, at in enumerate(nodes)}
    # f = entry + P^T f, solved directly. Iterating this equation made the
    # four nested loops in C nbody need thousands of rounds even though the
    # finite solution is small and well-conditioned.
    matrix = [[float(row == column) for column in range(len(nodes))] for row in range(len(nodes))]
    right = [0.0] * len(nodes)
    if body.entry in index:
        right[index[body.entry]] = 1.0
    for source, successors in transitions.items():
        column = index[source]
        for destination, probability in successors.items():
            matrix[index[destination]][column] -= probability

    for column in range(len(nodes)):
        pivot = max(range(column, len(nodes)), key=lambda row: abs(matrix[row][column]))
        if abs(matrix[pivot][column]) < 1e-12:
            return None
        matrix[column], matrix[pivot] = matrix[pivot], matrix[column]
        right[column], right[pivot] = right[pivot], right[column]
        scale = matrix[column][column]
        matrix[column] = [value / scale for value in matrix[column]]
        right[column] /= scale
        for row in range(len(nodes)):
            if row == column or abs(matrix[row][column]) < 1e-15:
                continue
            scale = matrix[row][column]
            matrix[row] = [value - scale * pivoted for value, pivoted in zip(matrix[row], matrix[column], strict=True)]
            right[row] -= scale * right[column]
    if any(value < -1e-9 for value in right):
        return None
    return {at: round(max(0.0, right[index[at]]), 9) for at in nodes}


def _block_instruction_counts(
    module: masm.Module, procedure: masm.Procedure, number: int
) -> tuple[int, dict[int, int], list[tuple[str, str, str]]]:
    """Exact emitted instruction counts split at the final block labels."""
    code, labels = _image(module, procedure, number)
    rows = _rows(code)
    starts = []
    for block in procedure.body.blocks:
        name = masm.label(number, block.at)
        if name not in labels:
            raise InvalidMeasurement(f"emitted function has no label for block {block.at:#x}")
        starts.append((block.at, labels[name]))
    prologue_end = starts[0][1] if starts else len(code)
    prologue = len(_rows(code[:prologue_end])) if prologue_end else 0
    counts = {}
    for index, (at, start) in enumerate(starts):
        end = starts[index + 1][1] if index + 1 < len(starts) else len(code)
        counts[at] = len(_rows(code[start:end])) if end > start else 0
    if prologue + sum(counts.values()) != len(rows):
        raise InvalidMeasurement("block instruction extents do not cover the emitted function exactly once")
    return prologue, counts, rows


def _dynamic_operations(module: masm.Module, procedure: masm.Procedure, number: int) -> tuple[float | None, str]:
    """Estimate executed instructions only when every visible cost is bounded."""
    prologue, counts, rows = _block_instruction_counts(module, procedure, number)
    if any(mnemonic in {"call", "int", "into"} for _raw, mnemonic, _operands in rows):
        return None, "unmeasured: call or interrupt hides executed work"
    if any(mnemonic.startswith("rep") for _raw, mnemonic, _operands in rows):
        return None, "unmeasured: repeated instruction has no audited count"
    frequencies = _frequencies(procedure.body)
    if frequencies is None:
        return None, "unmeasured: control flow has no finite profile-free estimate"
    estimate = round(float(prologue) + sum(counts[at] * frequencies.get(at, 0.0) for at in counts), 6)
    return estimate, "estimated: CFG branches and ten iterations per natural loop"


def _cost_report(
    rows: list[tuple[str, str, str]], target: targets.Profile
) -> tuple[float | None, str, tuple[str, ...]]:
    """Return the ranking and make every missing cost assumption visible."""
    kinds = [cycles.classify(mnemonic, operands, raw) for raw, mnemonic, operands in rows]
    missing = {
        f"unknown:{mnemonic}" if kind == "unknown" else kind
        for (_raw, mnemonic, _operands), kind in zip(rows, kinds, strict=True)
        if kind == "unknown" or not target.prices(kind)
    }
    if missing:
        forms = tuple(sorted(missing))
        return None, f"unpriced: {', '.join(forms)}", forms
    if target.name == "386":
        return float(sum(target.cost(kind) for kind in kinds)), "priced", ()
    scored, _detail = cycles.score(rows)
    return float(scored[0][targets.names().index(target.name) - 1]), "priced", ()


def _cost(rows: list[tuple[str, str, str]], target: targets.Profile) -> float | None:
    return _cost_report(rows, target)[0]


def function_report(module: masm.Module, procedure: masm.Procedure, number: int, cpu: str) -> dict:
    target = targets.profile(cpu)
    code = _blob(module, procedure, number)
    rows = _rows(code)
    loads, stores = _memory(rows)
    instructions = tuple(one for block in procedure.body.blocks for one in block.insns)
    dynamic_operations, dynamic_status = _dynamic_operations(module, procedure, number)
    weighted_cost, weighted_status, unpriced_forms = _cost_report(rows, target)
    return {
        "name": procedure.name,
        "bytes": len(code),
        "instructions": len(rows),
        "weighted_cost": weighted_cost,
        "weighted_status": weighted_status,
        "unpriced_forms": unpriced_forms,
        "dynamic_operations": dynamic_operations,
        "dynamic_status": dynamic_status,
        "loads": loads,
        "stores": stores,
        "branches": sum(mnemonic.startswith("j") for _raw, mnemonic, _operands in rows),
        "calls": sum(mnemonic == "call" for _raw, mnemonic, _operands in rows),
        "address_calculations": sum(mnemonic == "lea" for _raw, mnemonic, _operands in rows),
        "peak_live_values": _peak_live(procedure),
        "spill_reloads": sum(one.spill_reload for one in instructions),
        "spill_stores": sum(one.spill_store for one in instructions),
        "rematerializations": sum(one.rematerialized for one in instructions),
        "assembly_sha256": hashlib.sha256(code).hexdigest(),
        "target_status": "missing",
        "target": None,
        "ratio": None,
    }


def apply_target(function: dict, target: dict) -> dict:
    if not target.get("audited"):
        raise UntrustedTarget("target is not marked audited")
    if not isinstance(target.get("evidence"), str) or not target["evidence"].strip():
        raise UntrustedTarget("target has no evidence")
    if target.get("assembly_sha256") == function.get("assembly_sha256"):
        raise UntrustedTarget("target is the candidate's own assembly")
    metrics = target.get("metrics")
    if not isinstance(metrics, dict) or not metrics:
        raise UntrustedTarget("target has no metrics")
    ratios = []
    for name, denominator in metrics.items():
        numerator = function.get(name)
        if not isinstance(numerator, (int, float)) or not isinstance(denominator, (int, float)) or denominator <= 0:
            raise UntrustedTarget(f"target metric {name} is not a positive comparable number")
        ratios.append(numerator / denominator)
    return {
        **function,
        "target_status": "audited",
        "target": target,
        "ratio": max(ratios),
    }


def module_report(
    module: masm.Module,
    source: Path,
    cpu: str,
    target_data: dict | None = None,
    stages: dict[str, list[dict]] | None = None,
) -> dict:
    target = targets.profile(cpu)
    functions = []
    registered = (target_data or {}).get("targets", {})
    for number, procedure in enumerate(module.procedures):
        measured = function_report(module, procedure, number, target.name)
        key = f"{source.stem}.{procedure.name}.{target.name}"
        if key in registered:
            measured = apply_target(measured, registered[key])
        measured["stages"] = (stages or {}).get(procedure.name, [])
        functions.append(measured)
    return {
        "schema": 1,
        "revision": _revision(),
        "cpu": target.name,
        "source": str(source),
        "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
        "functions": functions,
    }


def _reference(source: Path, compiler: str, dump: Path) -> dict:
    path = shutil.which(compiler)
    if path is None:
        return {"compiler": compiler, "status": "unavailable"}
    clang = "clang" in Path(path).name
    if not clang:
        machine = subprocess.run([path, "-dumpmachine"], capture_output=True, text=True)
        target = machine.stdout.strip()
        if machine.returncode or not target.startswith(("i386", "i486", "i586", "i686")):
            return {
                "compiler": compiler,
                "version": _version(compiler),
                "status": "failed",
                "assembly": None,
                "diagnostic": f"not an i386-family compiler: {target or 'unknown target'}",
            }
    flags = [
        "-O3",
        "-march=i386",
        "-ffreestanding",
        "-fno-pic",
        "-fno-pie",
        "-fno-stack-protector",
        "-fno-asynchronous-unwind-tables",
        # qbopt's default floating contract observes the current rounding
        # mode, traps, and destination precision.  Without these, GCC kept
        # nbody's declared doubles as extended x87 values and the report
        # called the resulting absence of binary64 stores a codegen win.
        # That is a different program, not a target for this one.
        "-frounding-math",
        "-ftrapping-math",
        "-fexcess-precision=standard",
        "-ffp-contract=off",
        "-S",
        "-masm=intel",
        *(f"-D{qualifier}=" for qualifier in REFERENCE_QUALIFIERS),
    ]
    if clang:
        flags += ["--target=i386-unknown-linux-gnu", "-mno-sse", "-mno-sse2", "-fno-vectorize", "-fno-slp-vectorize"]
    else:
        flags += ["-m32", "-mno-sse", "-mno-sse2", "-fno-tree-vectorize"]
    done = subprocess.run([path, *flags, str(source), "-o", str(dump)], capture_output=True, text=True)
    generated = done.returncode == 0
    assembly = dump.read_text() if generated else ""
    return {
        "source": str(source),
        "compiler": compiler,
        "version": _version(compiler),
        "status": "generated" if generated else "failed",
        "assembly": str(dump) if generated else None,
        "flags": flags,
        "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
        "functions": _reference_functions(assembly) if generated else [],
        "diagnostic": done.stderr if done.returncode else "",
    }


def _comparisons(reports: list[dict], references: list[dict]) -> list[dict]:
    """Join qbopt and reference functions by source and C symbol identity."""
    out = []
    for report in reports:
        for reference in references:
            if reference.get("source") != report["source"] or reference.get("status", "generated") != "generated":
                continue
            theirs = {one["name"].lstrip("_"): one for one in reference.get("functions", ())}
            for function in report["functions"]:
                name = function["name"].lstrip("_")
                other = theirs.get(name)
                if other is None:
                    continue
                ratios = {
                    metric: function[metric] / other[metric] if other[metric] else None for metric in STRUCTURAL_METRICS
                }
                attribution = {
                    metric: _gap_attribution(function.get("stages", []), metric, other[metric], function[metric])
                    for metric in STRUCTURAL_METRICS
                }
                out.append(
                    {
                        "source": report["source"],
                        "cpu": report["cpu"],
                        "function": name,
                        "compiler": reference["compiler"],
                        "reference_assembly": reference.get("assembly"),
                        "qbopt": {metric: function[metric] for metric in STRUCTURAL_METRICS},
                        "reference": {metric: other[metric] for metric in STRUCTURAL_METRICS},
                        "ratios": ratios,
                        "gap_attribution": attribution,
                        "first_excess_stage": {
                            metric: result["stage"] if result["status"] == "attributed" else None
                            for metric, result in attribution.items()
                        },
                    }
                )
    return out


def _first_excess_stage(stages: list[dict], metric: str, reference: int) -> str | None:
    """First stage after which a structural excess remains through emission."""
    relevant = [one for one in stages if metric in one]
    if not relevant or relevant[-1][metric] <= reference:
        return None
    for index, stage in enumerate(relevant):
        if all(one[metric] > reference for one in relevant[index:]):
            return stage["stage"]
    return None


def _gap_attribution(stages: list[dict], metric: str, reference: int, emitted: int) -> dict:
    """Attribute only when the last stage and emitted-byte instruments agree."""
    if emitted <= reference:
        return {"status": "no_excess", "stage": None}
    relevant = [one for one in stages if one.get("form") == "lir" and metric in one]
    if not relevant:
        return {"status": "unmeasured", "stage": None}
    last = relevant[-1][metric]
    if last != emitted:
        return {
            "status": "unmapped",
            "stage": None,
            "last_stage": last,
            "emitted": emitted,
        }
    return {"status": "attributed", "stage": _first_excess_stage(relevant, metric, reference)}


def _stage_metrics(state: object) -> dict:
    """Comparable structural counts at one MIR or LIR boundary."""
    if isinstance(state, mir.MirBody):
        ops = [one for block in state.blocks for one in block.ops]
        return {
            "form": "mir",
            "operations": len(ops),
            "loads": sum(len(one.loads) for one in ops),
            "stores": sum(len(one.stores) for one in ops),
            "branches": sum(one.kind in (mir.Kind.JUMP, mir.Kind.BRANCH) for one in ops),
            "calls": sum(one.kind is mir.Kind.CALL for one in ops),
            "address_calculations": sum(one.kind is mir.Kind.ADDRESS for one in ops),
        }
    if not isinstance(state, lir.LirBody):
        return {}
    all_insns = [one for block in state.blocks for one in block.insns]
    insns = [one for one in all_insns if one.what is not None and one.what.op is not ir.Operation.NOTHING]
    loads = stores = branches = calls = addresses = 0
    for one in insns:
        what = one.what
        if what is None:
            continue
        memory_dests = tuple(operand for operand in what.dests if isinstance(operand, ir.Mem))
        memory_sources = tuple(operand for operand in what.sources if isinstance(operand, ir.Mem))
        stores += len(memory_dests)
        loads += len(memory_sources)
        if what.op not in (ir.Operation.MOVE, ir.Operation.FLOAT_STORE):
            loads += sum(operand not in memory_sources for operand in memory_dests)
        branches += what.op in (ir.Operation.JUMP, ir.Operation.BRANCH)
        calls += what.op is ir.Operation.CALL
        addresses += what.name.lower() == "lea"
    return {
        "form": "lir",
        "instructions": len(insns),
        "loads": loads,
        "stores": stores,
        "branches": branches,
        "calls": calls,
        "address_calculations": addresses,
        "spill_reloads": sum(one.spill_reload for one in all_insns),
        "spill_stores": sum(one.spill_store for one in all_insns),
        "rematerializations": sum(one.rematerialized for one in all_insns),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="quality", description=__doc__.splitlines()[0])
    parser.add_argument("sources", nargs="+", type=Path)
    parser.add_argument("--cpu", choices=("all", *targets.names()), default="386")
    parser.add_argument("--targets", type=Path, default=DEFAULT_TARGETS)
    parser.add_argument("--json", type=Path)
    parser.add_argument("--dump", type=Path, default=ROOT / "build" / "quality")
    parser.add_argument("--references", action="store_true")
    parser.add_argument("--gate", action="store_true", help="fail for missing targets or ratios above 1.10")
    args = parser.parse_args(argv)
    target_data = json.loads(args.targets.read_text()) if args.targets.is_file() else {"schema": 1, "targets": {}}
    cpus = targets.names() if args.cpu == "all" else (args.cpu,)
    reports = []
    args.dump.mkdir(parents=True, exist_ok=True)
    references = []
    for source in args.sources:
        text = source.read_text() if source.suffix == ".cgs" else cfront.recorded(source, [])
        for name in cpus:
            stages: dict[str, list[dict]] = {}

            def watch(stage: str, function: str, state: object, sink: dict[str, list[dict]] = stages) -> None:
                measured = _stage_metrics(state)
                if measured:
                    sink.setdefault(function, []).append({"stage": stage, **measured})

            stage_dump = args.dump / f"{source.stem}-{name}-stages"
            module = cfront.assembled(text, source.stem, optimise=True, cpu=name, dump=stage_dump, watch=watch)
            assembly = args.dump / f"{source.stem}-{name}.asm"
            assembly.write_text(masm.text(module))
            report = module_report(module, source, name, target_data, stages)
            report["assembly"] = str(assembly)
            report["stage_dump"] = str(stage_dump)
            reports.append(report)
        if args.references and source.suffix == ".c":
            for compiler in REFERENCE_COMPILERS:
                references.append(_reference(source, compiler, args.dump / f"{source.stem}-{compiler}.s"))
    comparisons = _comparisons(reports, references)
    result = {
        "schema": 1,
        "revision": _revision(),
        "compilers": {name: _version(name) for name in REFERENCE_COMPILERS},
        "reports": reports,
        "references": references,
        "structural_comparisons": comparisons,
    }
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(result, indent=2) + "\n")
    for report in reports:
        for function in report["functions"]:
            cost = (
                f"UNPRICED[{','.join(function['unpriced_forms'])}]"
                if function["weighted_cost"] is None
                else f"{function['weighted_cost']:g}"
            )
            ratio = "NO TARGET" if function["ratio"] is None else f"{function['ratio']:.2f}x"
            print(
                f"{report['cpu']:>4} {Path(report['source']).stem}.{function['name']:<24} "
                f"{function['bytes']:>5} bytes {function['instructions']:>4} ins {cost:>7} cost {ratio}"
            )
    for comparison in comparisons:
        ratio = comparison["ratios"]["instructions"]
        measured = "--" if ratio is None else f"{ratio:.2f}x"
        print(
            f" ref {comparison['cpu']:>4} {Path(comparison['source']).stem}.{comparison['function']:<24} "
            f"{comparison['compiler']:<14} {measured:>6} instructions"
        )
    if not args.gate:
        return 0
    return int(
        any(
            function["ratio"] is None or function["ratio"] > 1.10
            for report in reports
            for function in report["functions"]
        )
    )


if __name__ == "__main__":
    raise SystemExit(main())
