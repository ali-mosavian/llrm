"""Exhaustive fresh-writer checks over every committed object."""

from pathlib import Path

import pytest

from qbopt import wholeseg
from qbopt.backend import omfwrite


def test_every_fresh_object_maps_every_code_offset_it_names(obj: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Every record and fixup offset in an accepted object has a new home."""
    unmapped: list[int] = []
    real = omfwrite._mapped

    def watch(offset: int, kept: int, moved: dict[int, int]) -> int | None:
        out = real(offset, kept, moved)
        if out is None:
            unmapped.append(offset)
        return out

    monkeypatch.setattr(omfwrite, "_mapped", watch)
    got = wholeseg.emitted(obj.read_bytes())
    if got.outcome is wholeseg.Emission.LIR:
        assert not unmapped, f"{obj.stem}: emitted while {len(unmapped)} offsets did not map"
