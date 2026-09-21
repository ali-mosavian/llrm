"""Whole-module call memory effects for source-neutral HIR lowering."""

from dataclasses import replace
from collections.abc import Callable

from qbopt.hir import model
from qbopt.model import mir
from qbopt.analysis import alias
from qbopt.hir.lower import Lowered


def annotated(
    module: model.Module,
    functions: tuple[model.Function, ...],
    semantic: tuple[Lowered, ...],
    *,
    object_name: Callable[[str], str] = lambda name: name,
) -> tuple[Lowered, ...]:
    """Instantiate every defined callee's parameter effects at each call.

    HIR call operands stay in source-parameter order.  That makes this the
    common boundary where all source frontends can compute mod/ref effects,
    before ABI physicalization chooses stack order or register locations.
    """
    types = {one.id: one for one in module.types}
    callables = {one.id: one for one in module.callables}
    procedures: dict[str, alias.Procedure] = {}
    lowered_by_name: dict[str, Lowered] = {}
    for function, lowered in zip(functions, semantic, strict=True):
        body = alias.annotated(lowered.body)
        calls: dict[int, str] = {}
        arguments: dict[int, tuple[object, ...]] = {}
        abi_sites = {site.instruction for site in function.calls}
        instructions = {
            instruction.id: instruction
            for block in function.blocks
            for instruction in block.instructions
            if instruction.id in abi_sites
        }
        call_ops = {
            operation.id: operation
            for block in body.blocks
            for operation in block.ops
            if operation.kind is mir.Kind.CALL
        }
        value_types = {one.id: types[one.type] for one in function.values}
        for site in function.calls:
            operation = call_ops.get(site.instruction)
            instruction = instructions.get(site.instruction)
            if operation is None or instruction is None:
                raise ValueError(f"{function.name}: call {site.instruction} did not survive HIR lowering")
            target = callables[site.callee].name if site.callee is not None else operation.name
            calls[operation.at] = object_name(target)
            arguments[operation.at] = tuple(
                (lowered.values[operand.value], 0)
                if isinstance(operand, model.ValueRef) and value_types[operand.value].kind is model.TypeKind.POINTER
                else None
                for operand in instruction.operands
            )
        name = object_name(function.name)
        procedure = alias.Procedure(body, calls, arguments)
        procedures[name] = procedure
        lowered_by_name[name] = replace(lowered, body=body)
    summaries = alias.summaries(procedures)
    return tuple(
        replace(
            lowered_by_name[object_name(function.name)],
            body=alias.calls_annotated(procedures[object_name(function.name)], summaries),
        )
        for function in functions
    )
