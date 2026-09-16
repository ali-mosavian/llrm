"""Established source-language memory summaries for C library functions.

These are semantic contracts, not ABI contracts: register clobbers and stack
cleanup remain the backend's concern.  Object names are normalized by removing
the target's one C decoration underscore, so a user function actually named
``_strlen`` is not mistaken for the standard function.
"""

from collections.abc import Iterable

from qbopt.model import memory
from qbopt.analysis import alias


def _readonly(*parameters: int) -> alias.Summary:
    return alias.Summary(
        reads=frozenset(memory.Slice(memory.Object(memory.Kind.PARAMETER, parameter)) for parameter in parameters)
    )


# GCC marks strlen pure and constrains its use set to argument zero. LLVM adds
# readonly, argmemonly and nocapture(0). Open Watcom's implementation advances
# a local pointer while reading ``*p`` and performs no store.
_SOURCE = {
    "strlen": _readonly(0),
}


def summaries(names: Iterable[str]) -> dict[str, alias.Summary]:
    """Known summaries under the object names used at these call sites."""
    result = {}
    for name in names:
        source_name = name[1:] if name.startswith("_") else name
        if source_name in _SOURCE:
            result[name] = _SOURCE[source_name]
    return result
