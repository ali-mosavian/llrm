from pathlib import Path
from typing import Literal

import pytest

import corpus
from qbopt.objectfile import omf

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures" / "omf"

OBJECTS = sorted(p.name for p in FIXTURES.glob("*.obj"))

# the four configurations whose long-operator code differs. jumptable.obj is a
# different program -- an ON GOTO and a SELECT CASE -- and calls no operator.
OPERATOR_OBJECTS = ["pds-g2.obj", "qb45.obj", "vbdos-g2.obj", "vbdos-g3.obj"]

FULL_MARKERS = frozenset({"corpus", "e2e", "full", "slow"})
FULL_OBJECT_FIXTURES = frozenset({"mapped_obj", "obj", "operator_obj"})

# Tier 1 is intentionally an allow-list: a new test is Tier 2 until somebody
# decides it belongs in the bounded inner loop. These modules cover the object
# format, both frontends, MIR, core integer transforms, lowering, allocation,
# and final emission without exhaustive fixture expansion.
FAST_MODULES = frozenset(
    {
        "test_arithmetic_immediates.py",
        "test_constant_carry.py",
        "test_lower_conditions.py",
        "test_mir_alias.py",
        "test_omfwrite.py",
        "test_parcopy.py",
        "test_phi_widths.py",
        "test_sccp.py",
        "test_test_tiers.py",
    }
)


def pytest_addoption(parser: pytest.Parser) -> None:
    parser.addoption(
        "--full",
        action="store_true",
        help="run Tier 2 corpus, toolchain, emulator, and measurement tests as well as the fast Tier 1 suite",
    )


def pytest_ignore_collect(collection_path: Path, config: pytest.Config) -> bool | None:
    """Do not even import Tier 2 modules during the bounded fast run."""
    if config.getoption("--full"):
        return None
    if collection_path.parent.name == "tests" and collection_path.name.startswith("test_"):
        return collection_path.name not in FAST_MODULES
    return None


def tier_of(item: pytest.Item) -> Literal["fast", "full"]:
    """Classify tests from the bounded module manifest and cost markers.

    The three object fixtures expand one test into every committed OMF object.
    A direct Path parameter pointing at a fixture object is the other spelling
    used by exhaustive parametrizations. Both belong to the full tier even if
    a new test forgets to add the marker explicitly.
    """
    if FULL_MARKERS.intersection(marker.name for marker in item.iter_markers()):
        return "full"
    if item.path.name not in FAST_MODULES:
        return "full"
    if FULL_OBJECT_FIXTURES.intersection(getattr(item, "fixturenames", ())):
        return "full"
    callspec = getattr(item, "callspec", None)
    if callspec is not None and any(
        isinstance(value, Path) and value.suffix.lower() == ".obj" and "fixtures" in value.parts
        for value in callspec.params.values()
    ):
        return "full"
    return "fast"


def pytest_collection_modifyitems(config: pytest.Config, items: list[pytest.Item]) -> None:
    """Keep the default development loop below three seconds.

    ``--full`` only changes selection; both tiers are still marked in a full
    run, which keeps ``-m fast`` and ``-m full`` useful for diagnosis.
    """
    full = []
    for item in items:
        tier = tier_of(item)
        item.add_marker(getattr(pytest.mark, tier))
        if tier == "full":
            full.append(item)
    if config.getoption("--full"):
        return
    if full:
        config.hook.pytest_deselected(items=full)
        items[:] = [item for item in items if item not in full]


@pytest.fixture
def fixtures() -> Path:
    return FIXTURES


@pytest.fixture(params=OBJECTS)
def obj(request: pytest.FixtureRequest) -> Path:
    return FIXTURES / request.param


MAPPABLE = [name for name in OBJECTS if corpus.mappable(FIXTURES / name)]


@pytest.fixture(params=MAPPABLE)
def mapped_obj(request: pytest.FixtureRequest) -> Path:
    return FIXTURES / request.param


@pytest.fixture(params=OPERATOR_OBJECTS)
def operator_obj(request: pytest.FixtureRequest) -> Path:
    return FIXTURES / request.param


@pytest.fixture
def jumptable() -> list[omf.Record]:
    return omf.read(FIXTURES / "jumptable.obj")
