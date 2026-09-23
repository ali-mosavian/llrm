"""Typed source semantics shared by source frontends.

This is intentionally not an AST and not another optimizer.  Frontends finish
name and type resolution before constructing it; :mod:`qbopt.hir.lower` turns
it directly into the existing MIR.
"""

from enum import StrEnum
from collections.abc import Iterable
from dataclasses import dataclass

SCHEMA_VERSION = 1


class RuntimeProfile(StrEnum):
    QB45 = "qb45"
    PDS71 = "pds71"
    VBDOS = "vbdos"
    FREESTANDING = "freestanding"


class Dialect(StrEnum):
    QBASIC11 = "qbasic11"
    QB45 = "qb45"
    PDS71 = "pds71"
    VBDOS = "vbdos"
    MODERN = "modern"


class TargetProfile(StrEnum):
    I386_REAL_MODE = "i386-real-mode"


class ArrayOrder(StrEnum):
    COLUMN_MAJOR = "column-major"
    ROW_MAJOR = "row-major"


class FloatMode(StrEnum):
    INLINE = "inline"
    ALTERNATE = "alternate"


class TypeKind(StrEnum):
    VOID = "void"
    BOOLEAN = "boolean"
    INTEGER = "integer"
    FLOAT = "float"
    ARRAY = "array"
    POINTER = "pointer"
    OPAQUE = "opaque"


class AddressKind(StrEnum):
    NONE = "none"
    NEAR = "near"
    FAR = "far"
    HUGE = "huge"
    CODE = "code"
    # A 16-bit protected/real-mode segment selector, without an offset.
    SEGMENT = "segment"


class FloatEvaluation(StrEnum):
    NONE = "none"
    BINARY32 = "binary32"
    BINARY64 = "binary64"
    EXTENDED80 = "extended80"


class StackCleanup(StrEnum):
    CALLER = "caller"
    CALLEE = "callee"


class CallDistance(StrEnum):
    NEAR = "near"
    FAR = "far"


@dataclass(frozen=True, slots=True)
class Type:
    id: int
    name: str
    kind: TypeKind
    width: int
    signed: bool | None = None
    evaluation: FloatEvaluation = FloatEvaluation.NONE
    element: int | None = None
    rank: int = 0
    bounds: tuple[tuple[int, int], ...] = ()
    address: AddressKind = AddressKind.NONE


class Storage(StrEnum):
    LOCAL = "local"
    PARAMETER = "parameter"
    STATIC = "static"
    MODULE = "module"
    COMMON = "common"
    EXTERNAL = "external"


class DataLinkage(StrEnum):
    INTERNAL = "internal"
    EXTERNAL = "external"


class FunctionLinkage(StrEnum):
    INTERNAL = "internal"
    EXTERNAL = "external"


@dataclass(frozen=True, slots=True)
class Place:
    id: int
    name: str
    type: int
    storage: Storage
    offset: int
    symbol: int = 0
    extent: int | None = None
    address: AddressKind = AddressKind.NEAR
    volatile: bool = False


def frame_pieces(places: Iterable[Place]) -> dict[int, tuple[tuple[int, int, tuple[object, ...]], ...]]:
    """Each frame place's bytes as the objects holding them: `(low, high, identity)` frame spans.

    A place overlapping no other is one object. Where places overlap, the frame splits at
    every place's edge and each piece is one object: a region filled at once is the union of
    the places it holds, which stay apart.
    """
    frame = sorted(
        (one for one in places if one.storage in (Storage.LOCAL, Storage.PARAMETER)),
        key=lambda one: (one.offset, one.id),
    )
    pieces: dict[int, tuple[tuple[int, int, tuple[object, ...]], ...]] = {}
    at = 0
    while at < len(frame):
        end = frame[at].offset + (frame[at].extent or 0)
        group = at + 1
        while group < len(frame) and frame[group].offset < end:
            end = max(end, frame[group].offset + (frame[group].extent or 0))
            group += 1
        members = frame[at:group]
        edges = sorted({edge for one in members for edge in (one.offset, one.offset + (one.extent or 0))})
        spans = []
        for low, high in zip(edges, edges[1:]):
            owner = min(
                (one for one in members if one.offset <= low and high <= one.offset + (one.extent or 0)),
                key=lambda one: (one.extent or 0, one.id),
            )
            exact = owner.offset == low and owner.offset + (owner.extent or 0) == high
            identity = (owner.storage, owner.id) if exact else (owner.storage, owner.id, low - owner.offset)
            spans.append((low, high, identity))
        for one in members:
            pieces[one.id] = tuple(
                span for span in spans if one.offset <= span[0] and span[1] <= one.offset + (one.extent or 0)
            )
        at = group
    return pieces


@dataclass(frozen=True, slots=True)
class Value:
    id: int
    type: int


@dataclass(frozen=True, slots=True)
class ValueRef:
    value: int


@dataclass(frozen=True, slots=True)
class Constant:
    type: int
    value: int | float


@dataclass(frozen=True, slots=True)
class PlaceRef:
    place: int


@dataclass(frozen=True, slots=True)
class ArrayElement:
    place: int
    indices: tuple[ValueRef | Constant, ...]


@dataclass(frozen=True, slots=True)
class ProjectedPlace:
    place: int
    indices: tuple[ValueRef | Constant, ...]
    offset: int
    type: int


@dataclass(frozen=True, slots=True)
class IndirectPlace:
    base: int
    offset: int
    type: int
    volatile: bool = False
    inbounds: bool = False  # the language promises the access stays inside one object


class DescriptorField(StrEnum):
    LENGTH = "length"
    CAPACITY = "capacity"


@dataclass(frozen=True, slots=True)
class DescriptorPlace:
    base: int
    field: DescriptorField
    type: int


type Operand = ValueRef | Constant | PlaceRef | ArrayElement | ProjectedPlace | IndirectPlace | DescriptorPlace


class Op(StrEnum):
    COPY = "copy"
    LOAD = "load"
    STORE = "store"
    ADDRESS = "address"
    PTR_OFFSET = "ptr_offset"
    POINTER_SEGMENT = "pointer_segment"
    POINTER_OFFSET = "pointer_offset"
    CONCAT = "concat"
    CONVERT = "convert"
    # A float to an integer, rounded toward zero; CONVERT rounds as the environment does.
    TRUNCATE = "truncate"
    SIGN_EXTEND = "sign_extend"
    ZERO_EXTEND = "zero_extend"
    ADD = "add"
    SUB = "sub"
    MUL = "mul"
    # Fixed-point scaling stays semantic through MIR.  Expanding fixed i32
    # here into generic i64 arithmetic loses that both inputs are narrow and
    # makes the target legalize a 32x32 product as an arbitrary 64x64 one.
    FIXED_MUL = "fixed_mul"
    FIXED_DIV = "fixed_div"
    DIV = "div"
    REM = "rem"
    DIVMOD = "divmod"
    UDIV = "udiv"
    UREM = "urem"
    UDIVMOD = "udivmod"
    AND = "and"
    OR = "or"
    XOR = "xor"
    SHL = "shl"
    SHR = "shr"
    SAR = "sar"
    NEG = "neg"
    NOT = "not"
    EQ = "eq"
    NE = "ne"
    LT = "lt"
    LE = "le"
    GT = "gt"
    GE = "ge"
    BELOW = "below"
    BELOW_EQ = "beloweq"
    ABOVE = "above"
    ABOVE_EQ = "aboveeq"
    STRING_EQ = "string_eq"
    STRING_NE = "string_ne"
    STRING_LT = "string_lt"
    STRING_LE = "string_le"
    STRING_GT = "string_gt"
    STRING_GE = "string_ge"
    FADD = "fadd"
    FSUB = "fsub"
    FMUL = "fmul"
    FDIV = "fdiv"
    FNEG = "fneg"
    FABS = "fabs"
    FSQRT = "fsqrt"
    FSIN = "fsin"
    FCOS = "fcos"
    FATAN = "fatan"
    FLOG2 = "flog2"
    FEXP2 = "fexp2"
    # An I/O port: port_in reads a byte from operands[0]; port_out writes
    # operands[1], a byte, to operands[0]. Both are observable and ordered.
    PORT_IN = "port_in"
    PORT_OUT = "port_out"
    CALL = "call"


@dataclass(frozen=True, slots=True)
class Instruction:
    id: int
    op: Op
    results: tuple[int, ...] = ()
    operands: tuple[Operand, ...] = ()
    callee: str | None = None
    pure: bool = False


class TerminatorKind(StrEnum):
    JUMP = "jump"
    BRANCH = "branch"
    SWITCH = "switch"
    RETURN = "return"
    UNREACHABLE = "unreachable"


@dataclass(frozen=True, slots=True)
class Terminator:
    kind: TerminatorKind
    operands: tuple[Operand, ...] = ()
    targets: tuple[int, ...] = ()
    cases: tuple[tuple[int, int], ...] = ()


@dataclass(frozen=True, slots=True)
class Block:
    id: int
    instructions: tuple[Instruction, ...]
    terminator: Terminator
    # The frontend expects this block never to run; see mir.MirBlock.cold.
    cold: bool = False


@dataclass(frozen=True, slots=True)
class CallAbi:
    instruction: int
    order: tuple[int, ...]
    cleanup: StackCleanup
    distance: CallDistance
    callee: int | None = None


@dataclass(frozen=True, slots=True)
class Callable:
    """One resolved language procedure symbol; calls refer to its stable id."""

    id: int
    name: str
    result_type: int | None
    parameter_types: tuple[int, ...]
    by_value: tuple[bool, ...]
    segmented: tuple[bool, ...]
    arrays: tuple[bool, ...]
    defined: bool


@dataclass(frozen=True, slots=True)
class ProcedureAbi:
    cleanup: StackCleanup
    distance: CallDistance
    parameter_bytes: int


@dataclass(frozen=True, slots=True)
class Function:
    id: int
    name: str
    result_type: int
    values: tuple[Value, ...]
    places: tuple[Place, ...]
    blocks: tuple[Block, ...]
    entry: int
    parameters: tuple[int, ...] = ()
    abi: ProcedureAbi | None = None
    calls: tuple[CallAbi, ...] = ()
    error_handler: int | None = None
    error_handler_local: bool = False
    external_entries: tuple[int, ...] = ()
    linkage: FunctionLinkage = FunctionLinkage.EXTERNAL


@dataclass(frozen=True, slots=True)
class DataRelocation:
    at: int
    target: int
    addend: int
    address: AddressKind


@dataclass(frozen=True, slots=True)
class DataObject:
    id: int
    name: str
    bytes: tuple[int, ...]
    readonly: bool = False
    relocations: tuple[DataRelocation, ...] = ()
    linkage: DataLinkage = DataLinkage.INTERNAL
    # Placement class, independently of mutability. Near objects participate
    # in DGROUP; far/huge objects live in a separately addressed segment.
    address: AddressKind = AddressKind.NEAR


@dataclass(frozen=True, slots=True)
class Module:
    id: int
    name: str
    types: tuple[Type, ...]
    functions: tuple[Function, ...]
    data: tuple[DataObject, ...] = ()
    callables: tuple[Callable, ...] = ()


@dataclass(frozen=True, slots=True)
class Program:
    dialect: Dialect
    runtime: RuntimeProfile
    modules: tuple[Module, ...]
    schema: int = SCHEMA_VERSION
    target: TargetProfile = TargetProfile.I386_REAL_MODE
    array_order: ArrayOrder = ArrayOrder.COLUMN_MAJOR
    float_mode: FloatMode = FloatMode.INLINE
