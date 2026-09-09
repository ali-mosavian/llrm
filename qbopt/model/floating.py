"""Machine-independent floating formats and observable evaluation rules."""

from dataclasses import dataclass
from enum import StrEnum


class Format(StrEnum):
    BINARY32 = "binary32"
    BINARY64 = "binary64"
    EXTENDED80 = "extended80"
    SIGNED16 = "signed16"
    SIGNED32 = "signed32"
    SIGNED64 = "signed64"


class Precision(StrEnum):
    EXACT = "exact"
    DESTINATION = "destination"
    DYNAMIC = "dynamic"


class Rounding(StrEnum):
    NONE = "none"
    DYNAMIC = "dynamic"


class Exceptions(StrEnum):
    STRICT = "strict"


@dataclass(frozen=True, slots=True)
class Semantics:
    """A storage conversion is distinct from arithmetic evaluation precision.

    Dynamic properties read the floating environment. Strict exceptions
    remain observable even when the numeric conversion itself is exact.
    None on an operation means unknown semantics, never permission to assume
    nearest rounding, no traps, or an unrounded store.
    """

    inputs: tuple[Format, ...]
    result: Format
    precision: Precision
    rounding: Rounding
    exceptions: Exceptions = Exceptions.STRICT
