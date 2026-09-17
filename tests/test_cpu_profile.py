from pathlib import Path
from dataclasses import FrozenInstanceError

import pytest

from qbopt import flow
from qbopt.backend import cpu
from qbopt.backend import lower
from qbopt.backend import allocate
from qbopt.cfront import compile as cfront

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures" / "c"


def test_every_public_cpu_name_has_one_immutable_profile() -> None:
    assert cpu.names() == ("386", "486", "P5", "P6", "K5", "K6", "K7", "Core")
    assert tuple(cpu.profile(name).name for name in cpu.names()) == cpu.names()
    with pytest.raises(FrozenInstanceError):
        cpu.profile("P5").name = "386"


def test_unknown_cpu_is_rejected_at_the_shared_boundary() -> None:
    with pytest.raises(ValueError, match="unknown CPU target"):
        cpu.profile("pentium")


def test_machine_pipeline_gives_allocator_the_complete_cpu_profile() -> None:
    phases = flow.machine({}, cpu="P5")
    allocator = next(one for one in phases if isinstance(one, allocate.RegAlloc))
    assert allocator.cpu is cpu.profile("P5")


def test_c_frontend_threads_selected_cpu_to_every_procedure(monkeypatch: pytest.MonkeyPatch) -> None:
    observed = []
    real = lower.lowered

    def recording(*args, **kwargs):
        observed.append(kwargs.get("cpu", args[6] if len(args) > 6 else None))
        return real(*args, **kwargs)

    monkeypatch.setattr(lower, "lowered", recording)
    cfront.compiled((FIXTURES / "halve.cgs").read_text(), "halve", optimise=True, cpu="P5")
    assert observed and set(observed) == {cpu.profile("P5")}


def test_c_frontend_default_remains_386(monkeypatch: pytest.MonkeyPatch) -> None:
    observed = []
    real = lower.lowered

    def recording(*args, **kwargs):
        observed.append(kwargs.get("cpu", args[6] if len(args) > 6 else None))
        return real(*args, **kwargs)

    monkeypatch.setattr(lower, "lowered", recording)
    cfront.compiled((FIXTURES / "halve.cgs").read_text(), "halve", optimise=True)
    assert observed and set(observed) == {cpu.profile("386")}
