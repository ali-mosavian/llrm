"""Measure emitted C functions without manufacturing a performance target.

The hard rule is that a target is external evidence, not the candidate with a
smaller number written beside it.  This tool therefore reports complete raw
metrics with ``target_status=missing`` until an audited target names its
evidence and a different normalized assembly hash.

    uv run python tools/quality.py fixtures/c/halve.cgs --cpu P5
    uv run python tools/quality.py bench/c/nbody.c --cpu all --references --dump build/quality
"""

import json
import shutil
import hashlib
import argparse
import subprocess
from pathlib import Path

from iced_x86 import Decoder
from iced_x86 import Formatter
from iced_x86 import FormatterSyntax

from qbopt.backend import masm
from qbopt.cycles import cycles
from qbopt.backend import allocate
from qbopt.backend import omfwrite
from qbopt.backend import cpu as targets
from qbopt.cfront import compile as cfront

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_TARGETS = ROOT / "bench" / "c" / "targets.json"
FORMAT = Formatter(FormatterSyntax.MASM)
REFERENCE_COMPILERS = ("clang", "i686-elf-gcc")


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


def _blob(module: masm.Module, procedure: masm.Procedure, number: int) -> bytes:
    """The exact selected bytes, with relocation fields normalized to zero."""
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
    return bytes(out)


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
    for raw, mnemonic, operands in rows:
        kind = cycles.classify(mnemonic, operands, raw)
        if kind.endswith("_rm") or kind in {"push_m", "imul_m32", "idiv_m32"}:
            loads += 1
        if kind.endswith("_mr"):
            stores += 1
            if mnemonic not in {"mov", "fst", "fstp", "fist", "fistp"}:
                loads += 1
        if "[" in operands and mnemonic.startswith(("fld", "fiadd", "fisub", "fimul", "fidiv")):
            loads += 1
        if "[" in operands and mnemonic.startswith(("fst", "fist")):
            stores += 1
    return loads, stores


def _cost(rows: list[tuple[str, str, str]], target: targets.Profile) -> float | None:
    if target.name == "386":
        total = 0
        for raw, mnemonic, operands in rows:
            try:
                total += target.cost(cycles.classify(mnemonic, operands, raw))
            except KeyError:
                return None
        return float(total)
    scored, _detail = cycles.score(rows)
    return float(scored[0][targets.names().index(target.name) - 1])


def function_report(module: masm.Module, procedure: masm.Procedure, number: int, cpu: str) -> dict:
    target = targets.profile(cpu)
    code = _blob(module, procedure, number)
    rows = _rows(code)
    loads, stores = _memory(rows)
    instructions = tuple(one for block in procedure.body.blocks for one in block.insns)
    return {
        "name": procedure.name,
        "bytes": len(code),
        "instructions": len(rows),
        "weighted_cost": _cost(rows, target),
        "dynamic_operations": None,
        "dynamic_status": "unmeasured: no audited trip/profile weights",
        "loads": loads,
        "stores": stores,
        "branches": sum(mnemonic.startswith("j") for _raw, mnemonic, _operands in rows),
        "calls": sum(mnemonic == "call" for _raw, mnemonic, _operands in rows),
        "address_calculations": sum(mnemonic == "lea" for _raw, mnemonic, _operands in rows),
        "peak_live_values": _peak_live(procedure),
        "spill_reloads": sum(one.spill_reload for one in instructions),
        "spill_stores": sum(one.spill_store for one in instructions),
        "rematerializations": None,
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


def module_report(module: masm.Module, source: Path, cpu: str, target_data: dict | None = None) -> dict:
    target = targets.profile(cpu)
    functions = []
    registered = (target_data or {}).get("targets", {})
    for number, procedure in enumerate(module.procedures):
        measured = function_report(module, procedure, number, target.name)
        key = f"{source.stem}.{procedure.name}.{target.name}"
        if key in registered:
            measured = apply_target(measured, registered[key])
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
    flags = ["-O3", "-march=i386", "-ffreestanding", "-S", "-masm=intel"]
    if clang:
        flags += ["--target=i386-unknown-linux-gnu", "-mno-sse", "-mno-sse2", "-fno-vectorize", "-fno-slp-vectorize"]
    else:
        flags += ["-m32", "-mno-sse", "-mno-sse2", "-fno-tree-vectorize"]
    done = subprocess.run([path, *flags, str(source), "-o", str(dump)], capture_output=True, text=True)
    return {
        "compiler": compiler,
        "version": _version(compiler),
        "status": "generated" if done.returncode == 0 else "failed",
        "assembly": str(dump) if done.returncode == 0 else None,
        "diagnostic": done.stderr if done.returncode else "",
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
            module = cfront.assembled(text, source.stem, optimise=True, cpu=name)
            assembly = args.dump / f"{source.stem}-{name}.asm"
            assembly.write_text(masm.text(module))
            report = module_report(module, source, name, target_data)
            report["assembly"] = str(assembly)
            reports.append(report)
        if args.references and source.suffix == ".c":
            for compiler in REFERENCE_COMPILERS:
                references.append(_reference(source, compiler, args.dump / f"{source.stem}-{compiler}.s"))
    result = {
        "schema": 1,
        "revision": _revision(),
        "compilers": {name: _version(name) for name in REFERENCE_COMPILERS},
        "reports": reports,
        "references": references,
    }
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(result, indent=2) + "\n")
    for report in reports:
        for function in report["functions"]:
            cost = "--" if function["weighted_cost"] is None else f"{function['weighted_cost']:g}"
            ratio = "NO TARGET" if function["ratio"] is None else f"{function['ratio']:.2f}x"
            print(
                f"{report['cpu']:>4} {Path(report['source']).stem}.{function['name']:<24} "
                f"{function['bytes']:>5} bytes {function['instructions']:>4} ins {cost:>7} cost {ratio}"
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
