"""Small executable reference semantics for verified common HIR.

This is an oracle for frontend and future target-backend tests, not a target
runtime or an alternative optimization path.  It executes the typed operations
the frontend committed to at the HIR boundary and makes no machine decisions.
"""

import math
import struct
from dataclasses import dataclass

from qbopt.hir import model
from qbopt.hir.verify import verify


class ExecutionError(ValueError):
    """Verified HIR uses an operation the reference executor cannot run."""


@dataclass(frozen=True, slots=True)
class Result:
    output: str
    value: int | float | None


@dataclass(frozen=True, slots=True)
class _Address:
    memory: bytearray
    offset: int


@dataclass(frozen=True, slots=True)
class _Location:
    memory: bytearray
    offset: int
    type: model.Type


type _Scalar = int | float | _Address


def _number(value: _Scalar) -> int | float:
    if isinstance(value, _Address):
        raise ExecutionError("address used as a numeric value")
    return value


def _whole(value: _Scalar) -> int:
    return int(_number(value))


def _integer(value: int, type_: model.Type) -> int:
    bits = type_.width * 8
    masked = value & ((1 << bits) - 1)
    if type_.signed and masked & (1 << (bits - 1)):
        return masked - (1 << bits)
    return masked


def _normalized(value: _Scalar, type_: model.Type) -> _Scalar:
    if type_.kind is model.TypeKind.POINTER:
        if not isinstance(value, _Address):
            raise ExecutionError(f"{type_.name}: expected an address")
        return value
    if type_.kind is model.TypeKind.FLOAT:
        number = float(_number(value))
        if type_.width == 4:
            return struct.unpack("<f", struct.pack("<f", number))[0]
        return number
    if type_.kind in (model.TypeKind.INTEGER, model.TypeKind.BOOLEAN):
        return _integer(_whole(value), type_)
    raise ExecutionError(f"{type_.name}: aggregate values are not first-class")


def _trunc_div(left: int, right: int) -> int:
    if not right:
        raise ExecutionError("integer division by zero")
    magnitude = abs(left) // abs(right)
    return -magnitude if (left < 0) != (right < 0) else magnitude


def _fixed_text(raw: int, fraction: int) -> str:
    negative = raw < 0
    magnitude = abs(raw)
    whole, remainder = divmod(magnitude, 1 << fraction)
    digits = str(remainder * 5**fraction).rjust(fraction, "0").rstrip("0") if remainder else "0"
    return f"{'-' if negative else ''}{whole}.{digits}"


class _Machine:
    def __init__(self, program: model.Program, limit: int) -> None:
        verify(program)
        if len(program.modules) != 1:
            raise ExecutionError("the reference executor accepts one HIR module")
        self.program = program
        self.module = program.modules[0]
        self.types = {one.id: one for one in self.module.types}
        self.functions = {one.name: one for one in self.module.functions}
        self.data = {one.id: bytearray(one.bytes) for one in self.module.data}
        self.output: list[str] = []
        self.remaining = limit

    def invoke(self, name: str, arguments: tuple[int | float, ...]) -> int | float | None:
        function = self.functions.get(name)
        if function is None:
            raise ExecutionError(f"unknown entry function {name!r}")
        if len(arguments) != len(function.parameters):
            raise ExecutionError(f"{name}: expected {len(function.parameters)} arguments, received {len(arguments)}")
        values: dict[int, _Scalar] = {}
        value_types = {one.id: self.types[one.type] for one in function.values}
        for value, argument in zip(function.parameters, arguments, strict=True):
            values[value] = _normalized(argument, value_types[value])
        locals_ = {
            place.id: bytearray(place.extent if place.extent is not None else self.types[place.type].width)
            for place in function.places
            if place.storage is not model.Storage.MODULE
        }
        places = {one.id: one for one in function.places}
        blocks = {one.id: one for one in function.blocks}

        def operand_type(operand: model.Operand) -> model.Type:
            if isinstance(operand, model.Constant):
                return self.types[operand.type]
            if isinstance(operand, model.ValueRef):
                return value_types[operand.value]
            return location(operand).type

        def scalar(operand: model.Operand) -> _Scalar:
            if isinstance(operand, model.Constant):
                return _normalized(operand.value, self.types[operand.type])
            if isinstance(operand, model.ValueRef):
                try:
                    return values[operand.value]
                except KeyError as error:
                    raise ExecutionError(f"{name}: value {operand.value} used before definition") from error
            return load(location(operand))

        def index_value(operand: model.ValueRef | model.Constant) -> int:
            value = scalar(operand)
            if not isinstance(value, int):
                raise ExecutionError("array index is not an integer")
            return value

        def location(operand: model.Operand) -> _Location:
            if isinstance(operand, model.IndirectPlace):
                address = values.get(operand.base)
                if not isinstance(address, _Address):
                    raise ExecutionError("indirect place has no address value")
                return _Location(address.memory, address.offset + operand.offset, self.types[operand.type])
            if not isinstance(operand, (model.PlaceRef, model.ArrayElement, model.ProjectedPlace)):
                raise ExecutionError(f"{type(operand).__name__} is not a place")
            place = places[operand.place]
            if place.storage is model.Storage.MODULE:
                try:
                    memory = self.data[place.symbol]
                except KeyError as error:
                    raise ExecutionError(f"{place.name}: unknown module object {place.symbol}") from error
                offset = place.offset
            else:
                memory = locals_[place.id]
                offset = 0
            if isinstance(operand, model.PlaceRef):
                return _Location(memory, offset, self.types[place.type])

            array = self.types[place.type]
            if isinstance(operand, model.ProjectedPlace) and array.kind is not model.TypeKind.ARRAY:
                if operand.indices:
                    raise ExecutionError(f"{place.name}: non-array projection has indices")
                type_ = self.types[operand.type]
                if operand.offset + type_.width > array.width:
                    raise ExecutionError(f"{place.name}: projection exceeds its place")
                return _Location(memory, offset + operand.offset, type_)
            if array.kind is not model.TypeKind.ARRAY or array.element is None:
                raise ExecutionError(f"{place.name}: indexed place is not an array")
            indices = tuple(index_value(one) for one in operand.indices)
            if len(indices) != len(array.bounds):
                raise ExecutionError(f"{place.name}: index rank mismatch")
            extents = tuple(high - low + 1 for low, high in array.bounds)
            adjusted = tuple(index - low for index, (low, _high) in zip(indices, array.bounds, strict=True))
            if any(index < 0 or index >= extent for index, extent in zip(adjusted, extents, strict=True)):
                raise ExecutionError(f"{place.name}: array index out of bounds")
            linear = 0
            if self.program.array_order is model.ArrayOrder.ROW_MAJOR:
                for index, extent in zip(adjusted, extents, strict=True):
                    linear = linear * extent + index
            else:
                stride = 1
                for index, extent in zip(adjusted, extents, strict=True):
                    linear += index * stride
                    stride *= extent
            element = self.types[array.element]
            extra = operand.offset if isinstance(operand, model.ProjectedPlace) else 0
            type_ = self.types[operand.type] if isinstance(operand, model.ProjectedPlace) else element
            return _Location(memory, offset + linear * element.width + extra, type_)

        def load(where: _Location) -> _Scalar:
            data = bytes(where.memory[where.offset : where.offset + where.type.width])
            if len(data) != where.type.width:
                raise ExecutionError("load falls outside its storage object")
            if where.type.kind is model.TypeKind.FLOAT:
                return struct.unpack("<f" if where.type.width == 4 else "<d", data)[0]
            if where.type.kind in (model.TypeKind.INTEGER, model.TypeKind.BOOLEAN):
                return int.from_bytes(data, "little", signed=bool(where.type.signed))
            raise ExecutionError(f"cannot load first-class {where.type.name}")

        def store(where: _Location, value: _Scalar) -> None:
            value = _normalized(value, where.type)
            if where.type.kind is model.TypeKind.FLOAT:
                data = struct.pack("<f" if where.type.width == 4 else "<d", value)
            elif isinstance(value, int):
                data = (value & ((1 << (where.type.width * 8)) - 1)).to_bytes(where.type.width, "little")
            else:
                raise ExecutionError("storing pointers is not implemented by the reference executor")
            after = where.offset + len(data)
            if where.offset < 0 or after > len(where.memory):
                raise ExecutionError("store falls outside its storage object")
            where.memory[where.offset : after] = data

        def define(instruction: model.Instruction, results: tuple[_Scalar, ...]) -> None:
            if len(results) != len(instruction.results):
                raise ExecutionError(f"{instruction.op}: result count mismatch")
            for value, result in zip(instruction.results, results, strict=True):
                values[value] = _normalized(result, value_types[value])

        def unsigned(value: int, type_: model.Type) -> int:
            return value & ((1 << (type_.width * 8)) - 1)

        def execute(instruction: model.Instruction) -> None:
            op = instruction.op
            if op is model.Op.LOAD:
                define(instruction, (load(location(instruction.operands[0])),))
                return
            if op is model.Op.STORE:
                store(location(instruction.operands[0]), scalar(instruction.operands[1]))
                return
            if op is model.Op.ADDRESS:
                where = location(instruction.operands[0])
                define(instruction, (_Address(where.memory, where.offset),))
                return
            args = tuple(scalar(one) for one in instruction.operands)
            if op is model.Op.PTR_OFFSET:
                address, displacement = args
                if not isinstance(address, _Address) or not isinstance(displacement, int):
                    raise ExecutionError("pointer offset requires an address and an integer")
                define(instruction, (_Address(address.memory, address.offset + displacement),))
                return
            if op is model.Op.CALL:
                returned = self._call(instruction.callee, args)
                if instruction.results:
                    if returned is None:
                        raise ExecutionError(f"{instruction.callee}: call did not return a value")
                    define(instruction, (returned,))
                else:
                    define(instruction, ())
                return
            if op in (model.Op.COPY, model.Op.CONVERT, model.Op.SIGN_EXTEND, model.Op.ZERO_EXTEND):
                define(instruction, (args[0],))
                return
            if op in (model.Op.FNEG, model.Op.NEG):
                define(instruction, (-_number(args[0]),))
                return
            if op is model.Op.NOT:
                type_ = operand_type(instruction.operands[0])
                result = int(not args[0]) if type_.kind is model.TypeKind.BOOLEAN else ~_whole(args[0])
                define(instruction, (result,))
                return
            if op in (model.Op.FIXED_MUL, model.Op.FIXED_DIV):
                left, right, fraction = map(_whole, args)
                result = (left * right) >> fraction if op is model.Op.FIXED_MUL else _trunc_div(left << fraction, right)
                define(instruction, (result,))
                return
            binary = {
                model.Op.ADD: lambda a, b: a + b,
                model.Op.SUB: lambda a, b: a - b,
                model.Op.MUL: lambda a, b: a * b,
                model.Op.AND: lambda a, b: _whole(a) & _whole(b),
                model.Op.OR: lambda a, b: _whole(a) | _whole(b),
                model.Op.XOR: lambda a, b: _whole(a) ^ _whole(b),
                model.Op.SHL: lambda a, b: _whole(a) << _whole(b),
                model.Op.SAR: lambda a, b: _whole(a) >> _whole(b),
                model.Op.FADD: lambda a, b: a + b,
                model.Op.FSUB: lambda a, b: a - b,
                model.Op.FMUL: lambda a, b: a * b,
                model.Op.FDIV: lambda a, b: a / b,
            }
            if op in binary:
                define(instruction, (binary[op](args[0], args[1]),))
                return
            if op is model.Op.SHR:
                left_type = operand_type(instruction.operands[0])
                define(instruction, (unsigned(_whole(args[0]), left_type) >> _whole(args[1]),))
                return
            if op in (model.Op.DIV, model.Op.REM, model.Op.DIVMOD):
                left, right = _whole(args[0]), _whole(args[1])
                quotient = _trunc_div(left, right)
                remainder = left - quotient * right
                results = (
                    (quotient, remainder) if op is model.Op.DIVMOD else (quotient if op is model.Op.DIV else remainder,)
                )
                define(instruction, results)
                return
            if op in (model.Op.UDIV, model.Op.UREM, model.Op.UDIVMOD):
                left_type = operand_type(instruction.operands[0])
                right_type = operand_type(instruction.operands[1])
                left = unsigned(_whole(args[0]), left_type)
                right = unsigned(_whole(args[1]), right_type)
                if not right:
                    raise ExecutionError("integer division by zero")
                quotient, remainder = divmod(left, right)
                results = (
                    (quotient, remainder)
                    if op is model.Op.UDIVMOD
                    else (quotient if op is model.Op.UDIV else remainder,)
                )
                define(instruction, results)
                return
            comparisons = {
                model.Op.EQ: lambda a, b: a == b,
                model.Op.NE: lambda a, b: a != b,
                model.Op.LT: lambda a, b: a < b,
                model.Op.LE: lambda a, b: a <= b,
                model.Op.GT: lambda a, b: a > b,
                model.Op.GE: lambda a, b: a >= b,
            }
            if op in comparisons:
                define(instruction, (int(comparisons[op](args[0], args[1])),))
                return
            unsigned_comparisons = {
                model.Op.BELOW: lambda a, b: a < b,
                model.Op.BELOW_EQ: lambda a, b: a <= b,
                model.Op.ABOVE: lambda a, b: a > b,
                model.Op.ABOVE_EQ: lambda a, b: a >= b,
            }
            if op in unsigned_comparisons:
                left_type = operand_type(instruction.operands[0])
                right_type = operand_type(instruction.operands[1])
                left = unsigned(_whole(args[0]), left_type)
                right = unsigned(_whole(args[1]), right_type)
                define(instruction, (int(unsigned_comparisons[op](left, right)),))
                return
            unary_float = {
                model.Op.FABS: abs,
                model.Op.FSQRT: math.sqrt,
                model.Op.FSIN: math.sin,
                model.Op.FCOS: math.cos,
                model.Op.FATAN: math.atan,
                model.Op.FLOG2: math.log2,
                model.Op.FEXP2: lambda one: 2**one,
            }
            if op in unary_float:
                define(instruction, (unary_float[op](float(_number(args[0]))),))
                return
            raise ExecutionError(f"{name}: unsupported HIR operation {op.value}")

        current = function.entry
        while True:
            self.remaining -= 1
            if self.remaining < 0:
                raise ExecutionError("execution step limit exceeded")
            block = blocks[current]
            for instruction in block.instructions:
                execute(instruction)
            terminator = block.terminator
            if terminator.kind is model.TerminatorKind.JUMP:
                current = terminator.targets[0]
            elif terminator.kind is model.TerminatorKind.BRANCH:
                current = terminator.targets[0 if scalar(terminator.operands[0]) else 1]
            elif terminator.kind is model.TerminatorKind.SWITCH:
                key = _whole(scalar(terminator.operands[0]))
                current = dict(terminator.cases).get(key, terminator.targets[0])
            elif terminator.kind is model.TerminatorKind.RETURN:
                return _number(scalar(terminator.operands[0])) if terminator.operands else None
            else:
                raise ExecutionError(f"{name}: reached unreachable control flow")

    def _call(self, name: str | None, arguments: tuple[_Scalar, ...]) -> _Scalar | None:
        if name is None:
            raise ExecutionError("call has no resolved callee")
        if name in self.functions:
            return self.invoke(name, tuple(_number(one) for one in arguments))
        if name == "_pn":
            self.output.append("\n")
        elif name == "_pt":
            [address] = arguments
            if not isinstance(address, _Address):
                raise ExecutionError("_pt requires an address")
            try:
                end = address.memory.index(0, address.offset)
            except ValueError as error:
                raise ExecutionError("_pt received a non-NUL-terminated string") from error
            self.output.append(bytes(address.memory[address.offset : end]).decode("cp437"))
        elif name in ("_pf2", "_pf4"):
            raw, fraction = arguments
            self.output.append(_fixed_text(_whole(raw), _whole(fraction)))
        elif name == "_pb":
            self.output.append("true" if arguments[0] else "false")
        elif name == "_pc":
            self.output.append(bytes([_whole(arguments[0])]).decode("cp437"))
        elif name in ("_pi1", "_pu1", "_pi2", "_pu2", "_pi4", "_pu4"):
            self.output.append(str(_whole(arguments[0])))
        elif name in ("_pr4", "_pr8"):
            self.output.append(str(float(_number(arguments[0]))))
        else:
            raise ExecutionError(f"no reference implementation for external {name!r}")
        return None


def run(
    program: model.Program,
    entry: str,
    arguments: tuple[int | float, ...] = (),
    *,
    step_limit: int = 10_000_000,
) -> Result:
    """Execute one function and return its captured output and result value."""
    machine = _Machine(program, step_limit)
    value = machine.invoke(entry, arguments)
    return Result("".join(machine.output), value)
