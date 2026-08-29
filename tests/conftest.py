from pathlib import Path

import pytest

from qbopt import omf
from qbopt import blocks
from qbopt import module

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures" / "omf"

OBJECTS = sorted(p.name for p in FIXTURES.glob("*.obj"))

# the four configurations whose long-operator code differs. jumptable.obj is a
# different program -- an ON GOTO and a SELECT CASE -- and calls no operator.
OPERATOR_OBJECTS = ["pds-g2.obj", "qb45.obj", "vbdos-g2.obj", "vbdos-g3.obj"]


@pytest.fixture
def fixtures() -> Path:
    return FIXTURES


@pytest.fixture(params=OBJECTS)
def obj(request: pytest.FixtureRequest) -> Path:
    return FIXTURES / request.param


def _mappable(name: str) -> bool:
    found = module.load(FIXTURES / name)
    return found is not None and not isinstance(blocks.code_map(found), str)


MAPPABLE = [name for name in OBJECTS if _mappable(name)]


@pytest.fixture(params=MAPPABLE)
def mapped_obj(request: pytest.FixtureRequest) -> Path:
    return FIXTURES / request.param


@pytest.fixture(params=OPERATOR_OBJECTS)
def operator_obj(request: pytest.FixtureRequest) -> Path:
    return FIXTURES / request.param


@pytest.fixture
def jumptable() -> list[omf.Record]:
    return omf.read(FIXTURES / "jumptable.obj")
