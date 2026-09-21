"""Common HIR from the modern frontend to a fresh 16-bit OMF object."""

from pathlib import Path

from qbopt import hir
from qbopt import flow
from qbopt.backend import masm
from qbopt.backend import jumps
from qbopt.backend import lower
from qbopt.backend import phielim
from qbopt.backend import omfwrite
from qbopt.backend import prologue
from qbopt.backend import lower_int64
from qbopt.backend import cpu as targets
from qbopt.backend import frame as frames
from qbopt.frontend.qb import physicalize
from qbopt.objectfile.module import Space


def assembled(
    program: hir.Program,
    *,
    entry: str,
    cpu: str | targets.Profile = "386",
) -> masm.Module:
    """Lower one modern module to allocated machine form.

    The named entry is public for the runtime to call. Source procedures use
    one uniform far-call ABI for now; the runtime may live in another code
    segment, and parameter layout must not depend on which caller reached a
    procedure.
    """
    if len(program.modules) != 1:
        raise ValueError("native modern compilation currently accepts one module")
    module = program.modules[0]
    target = targets.profile(cpu)
    semantic = hir.lower(program)
    procedures: list[masm.Procedure] = []
    referenced: dict[str, str] = {}
    source_names = {function.name for function in module.functions}

    def object_name(name: str) -> str:
        # DOS C symbols carry one leading underscore. Runtime builtins name
        # their ABI symbols explicitly (``__print_text``) and already have it.
        return name if name.startswith("__") else f"_{name}"

    for function, lowered in zip(module.functions, semantic, strict=True):
        physical = physicalize(program, function, lowered)
        legalized = lower_int64.expanded(
            physical.lowered.body,
            physical.calls,
            physical.contracts,
            physical.hints,
        )
        body = flow.verified(
            lower.lowered(
                physical.lowered.name,
                legalized.body,
                legalized.calls,
                {},
                legalized.contracts,
                {},
                cpu=target,
                hints=legalized.hints,
                pointer_model=physical.pointer_model,
            ),
            "lower",
            in_ssa=True,
        )
        owned_frame = frames.of(body, legalized.calls)
        in_ssa = True
        for phase in flow.machine(flow._pinned(body), owned_frame, legalized.calls, cpu=target):
            if isinstance(phase, prologue.Prologue):
                continue
            if isinstance(phase, phielim.PhiElimination):
                in_ssa = False
            body = flow.checked(body, phase, in_ssa=in_ssa)

        reserve = -min(min(owned_frame.slots.values(), default=0), owned_frame.floor)
        callees: dict[int, masm.Callee] = {}
        for at, name in legalized.calls.items():
            inline = legalized.inline.get(at)
            if inline is not None:
                callees[at] = masm.Callee(name, False, inline)
                continue
            far = at in physical.far_calls
            linked_name = object_name(name) if name in source_names else name
            callees[at] = masm.Callee(linked_name, far)
            referenced[linked_name] = "far" if far else "near"

        is_entry = function.name == entry
        linked_name = object_name(function.name)
        procedure = masm.Procedure(linked_name, is_entry, True, body, reserve, callees)
        body = jumps.duplicated_returns(body, masm.return_overhead_bytes(procedure))
        procedures.append(masm.Procedure(linked_name, is_entry, True, body, reserve, callees))

    linked_entry = object_name(entry)
    if not any(procedure.name == linked_entry for procedure in procedures):
        raise ValueError(f"entry function {entry!r} does not exist")

    defined = {procedure.name for procedure in procedures}
    externs = tuple(sorted((name, distance) for name, distance in referenced.items() if name not in defined))
    names = {(Space.SEGMENT, item.id): f"{module.name}$D{item.id}" for item in module.data}
    data = (
        (
            "_DATA",
            tuple(
                part
                for item in module.data
                for part in (masm.Label(names[(Space.SEGMENT, item.id)]), bytes(item.bytes))
            ),
        ),
    )
    return masm.Module(
        code=f"{module.name.upper()}_TEXT",
        names=names,
        externs=externs,
        publics=(linked_entry,),
        data=data,
        procedures=tuple(procedures),
    )


def written(program: hir.Program, *, entry: str, source: str | Path) -> bytes:
    """Compile a modern program directly to an OMF object."""
    return omfwrite.written(assembled(program, entry=entry), Path(source).name)
