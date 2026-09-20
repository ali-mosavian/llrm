"""Typed source semantics shared by source frontends.

This is intentionally not an AST and not another optimizer.  Frontends finish
name and type resolution before constructing it; :mod:`qbopt.hir.lower` turns
it directly into the existing MIR.
"""

from enum import StrEnum
from dataclasses import dataclass

SCHEMA_VERSION = 1


class RuntimeProfile(StrEnum):
    QB45 = "qb45"
    PDS71 = "pds71"
    VBDOS = "vbdos"


class Dialect(StrEnum):
    QBASIC11 = "qbasic11"
    QB45 = "qb45"
    PDS71 = "pds71"
    VBDOS = "vbdos"


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


type Operand = ValueRef | Constant | PlaceRef | ArrayElement | ProjectedPlace | IndirectPlace


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
    SIGN_EXTEND = "sign_extend"
    ZERO_EXTEND = "zero_extend"
    ADD = "add"
    SUB = "sub"
    MUL = "mul"
    DIV = "div"
    REM = "rem"
    DIVMOD = "divmod"
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
