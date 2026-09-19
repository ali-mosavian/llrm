"""Deterministic, strict JSON wire format for HIR producers."""

import json
from enum import Enum
from typing import cast
from types import UnionType
from typing import get_args
from typing import get_origin
from dataclasses import fields
from typing import get_type_hints

from qbopt.hir import model
from qbopt.hir.verify import verify
from qbopt.hir.verify import InvalidHIR

_TAGS = {
    "array_element": model.ArrayElement,
    "constant": model.Constant,
    "indirect": model.IndirectPlace,
    "place": model.PlaceRef,
    "projection": model.ProjectedPlace,
    "value": model.ValueRef,
}


type JSON = None | bool | int | float | str | list["JSON"] | dict[str, "JSON"]


def _plain(value: object) -> JSON:
    if isinstance(value, Enum):
        return value.value
    if isinstance(value, tuple):
        return [_plain(one) for one in value]
    if hasattr(value, "__dataclass_fields__"):
        # Reflection is confined to the wire boundary. ty cannot express the
        # runtime dataclass protocol without importing its private stub type.
        members = fields(value)  # ty: ignore[invalid-argument-type]
        out: dict[str, JSON] = {one.name: _plain(getattr(value, one.name)) for one in members}
        if isinstance(value, model.ArrayElement):
            out["tag"] = "array_element"
        elif isinstance(value, model.Constant):
            out["tag"] = "constant"
        elif isinstance(value, model.PlaceRef):
            out["tag"] = "place"
        elif isinstance(value, model.ProjectedPlace):
            out["tag"] = "projection"
        elif isinstance(value, model.IndirectPlace):
            out["tag"] = "indirect"
        elif isinstance(value, model.ValueRef):
            out["tag"] = "value"
        return out
    return cast("JSON", value)


def encode(program: model.Program, *, indent: int | None = None) -> str:
    verify(program)
    return json.dumps(_plain(program), indent=indent, separators=None if indent else (",", ":"), sort_keys=True) + "\n"


def _make(type_: object, value: object, where: str) -> object:
    origin = get_origin(type_)
    if type_ is model.Operand:
        if not isinstance(value, dict) or (tag := value.get("tag")) not in _TAGS:
            raise InvalidHIR(f"{where}: unknown operand tag")
        return _record(_TAGS[tag], value, where, tagged=True)
    if origin is tuple:
        if not isinstance(value, list):
            raise InvalidHIR(f"{where}: expected array")
        args = get_args(type_)
        subtype = args[0]
        return tuple(_make(subtype, one, f"{where}[]") for one in value)
    if origin is UnionType:
        choices = get_args(type_)
        if isinstance(value, dict) and (tag := value.get("tag")) in _TAGS and _TAGS[tag] in choices:
            return _record(_TAGS[tag], value, where, tagged=True)
        for choice in choices:
            if choice is type(None) and value is None:
                return None
            try:
                return _make(choice, value, where)
            except (InvalidHIR, TypeError, ValueError):
                pass
        raise InvalidHIR(f"{where}: value does not match {type_}")
    if isinstance(type_, type) and issubclass(type_, Enum):
        try:
            return type_(value)
        except ValueError as error:
            raise InvalidHIR(f"{where}: unknown {type_.__name__} {value!r}") from error
    if hasattr(type_, "__dataclass_fields__"):
        return _record(cast(type, type_), value, where)
    if type_ is int and (not isinstance(value, int) or isinstance(value, bool)):
        raise InvalidHIR(f"{where}: expected integer")
    if type_ is float and (not isinstance(value, (int, float)) or isinstance(value, bool)):
        raise InvalidHIR(f"{where}: expected number")
    if type_ is str and not isinstance(value, str):
        raise InvalidHIR(f"{where}: expected string")
    if type_ is bool and not isinstance(value, bool):
        raise InvalidHIR(f"{where}: expected boolean")
    return value


def _record(type_: type, value: object, where: str, *, tagged: bool = False) -> object:
    if not isinstance(value, dict):
        raise InvalidHIR(f"{where}: expected object")
    hints = get_type_hints(type_)
    allowed = set(hints) | ({"tag"} if tagged else set())
    unknown = set(value) - allowed
    if unknown:
        raise InvalidHIR(f"{where}: unknown fields {sorted(unknown)}")
    members = fields(type_)  # ty: ignore[invalid-argument-type]
    required = {one.name for one in members if one.default is one.default_factory}
    missing = required - set(value)
    if missing:
        raise InvalidHIR(f"{where}: missing fields {sorted(missing)}")
    args = {name: _make(hint, value[name], f"{where}.{name}") for name, hint in hints.items() if name in value}
    return type_(**args)


def decode(text: str) -> model.Program:
    try:
        raw = json.loads(text)
    except json.JSONDecodeError as error:
        raise InvalidHIR(f"invalid HIR JSON: {error}") from error
    program = cast(model.Program, _record(model.Program, raw, "program"))
    verify(program)
    return program
